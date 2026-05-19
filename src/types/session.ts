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
