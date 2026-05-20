import { invoke } from "@tauri-apps/api/core";
import type { TabBundle } from "@/types";

/** 영속화된 탭 그룹 로드. 첫 실행이거나 파일이 없으면 빈 bundle 반환. */
export async function listTabs(): Promise<TabBundle> {
  return invoke<TabBundle>("list_tabs");
}

/** 현재 탭 그룹 영속화. 디바운스는 호출자가 책임. */
export async function saveTabs(bundle: TabBundle): Promise<void> {
  await invoke<void>("save_tabs", { bundle });
}
