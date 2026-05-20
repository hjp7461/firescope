import type { ProfileMode } from "./profile";

export type Session = {
  session_id: string;
  profile_id: string;
  profile_name: string;
  project_id: string;
  mode: ProfileMode;
  activated_at: string;
  expires_at?: string;
};

export type TokenRefreshed = {
  session_id: string;
  profile_id: string;
  expires_at: string;
};

export type TabRecord = {
  id: string;
  profile_id?: string;
  order: number;
};

export type TabBundle = {
  version: 1;
  tabs: TabRecord[];
  active_tab_id?: string;
};
