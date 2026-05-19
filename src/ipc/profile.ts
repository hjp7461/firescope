// 프로파일 IPC 타입 안전 래퍼 (`docs/03-ipc-contract.md` §1).

import { call } from "./index";
import type {
  CreateProfileParams,
  CredentialInput,
  CredentialStatus,
  ExportResult,
  ImportResult,
  ProfileMeta,
  TestResult,
  UpdateProfileParams,
} from "@/types";

export const listProfiles = () => call<ProfileMeta[]>("list_profiles");

export const getProfile = (profileId: string) =>
  call<ProfileMeta | null>("get_profile", { profile_id: profileId });

// 타입 객체 인자는 { params }로 감싼다 (계약 v0.3).
export const createProfile = (params: CreateProfileParams) =>
  call<ProfileMeta>("create_profile", { params });

export const updateProfile = (params: UpdateProfileParams) =>
  call<ProfileMeta>("update_profile", { params });

export const deleteProfile = (profileId: string) =>
  call<void>("delete_profile", { profile_id: profileId });

export const duplicateProfile = (profileId: string, newName: string) =>
  call<ProfileMeta>("duplicate_profile", {
    profile_id: profileId,
    new_name: newName,
  });

export const setCredential = (
  profileId: string,
  credential: CredentialInput,
) =>
  call<CredentialStatus>("set_credential", {
    profile_id: profileId,
    credential,
  });

export const clearCredential = (profileId: string) =>
  call<CredentialStatus>("clear_credential", { profile_id: profileId });

export const testProfile = (profileId: string) =>
  call<TestResult>("test_profile", { profile_id: profileId });

export const exportProfiles = (path: string, profileIds?: string[]) =>
  call<ExportResult>("export_profiles", {
    path,
    profile_ids: profileIds ?? null,
  });

export const importProfiles = (path: string) =>
  call<ImportResult>("import_profiles", { path });
