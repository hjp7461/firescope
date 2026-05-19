import { create } from "zustand";

export type ViewKind = "table" | "tree" | "json" | "log";

type ViewState = {
  activeView: ViewKind;
  setView: (v: ViewKind) => void;
};

export const useViewStore = create<ViewState>((set) => ({
  activeView: "table",
  setView: (activeView) => set({ activeView }),
}));
