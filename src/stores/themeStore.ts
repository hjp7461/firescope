import { create } from "zustand";

export type Theme = "system" | "light" | "dark";

const STORAGE_KEY = "firescope:theme";

function loadTheme(): Theme {
  if (typeof window === "undefined") return "system";
  const raw = window.localStorage.getItem(STORAGE_KEY);
  return raw === "light" || raw === "dark" ? raw : "system";
}

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

/** html에 `.dark` 클래스를 적용/해제. shadcn은 `.dark` 클래스 변형을 따른다. */
function applyTheme(theme: Theme) {
  if (typeof document === "undefined") return;
  const isDark = theme === "dark" || (theme === "system" && systemPrefersDark());
  document.documentElement.classList.toggle("dark", isDark);
}

type ThemeState = {
  theme: Theme;
  setTheme: (t: Theme) => void;
};

export const useThemeStore = create<ThemeState>((set) => ({
  theme: loadTheme(),
  setTheme: (theme) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, theme);
    }
    applyTheme(theme);
    set({ theme });
  },
}));

/** 앱 시작 시 1회 호출 — 저장된 테마 적용 + system 모드 변경 추적. */
export function initTheme(): () => void {
  const initial = useThemeStore.getState().theme;
  applyTheme(initial);
  if (typeof window === "undefined") return () => {};
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => {
    if (useThemeStore.getState().theme === "system") {
      applyTheme("system");
    }
  };
  mq.addEventListener("change", handler);
  return () => mq.removeEventListener("change", handler);
}
