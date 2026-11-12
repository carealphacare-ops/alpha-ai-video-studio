import { invoke } from "@tauri-apps/api/core";

export interface Project { id: string; name: string; kind: string; status: string; created_at: number; updated_at: number }
export interface ProviderState { id: string; enabled: boolean; has_key: boolean; model: string | null }
export interface UsageSummary { month_estimated: number; month_actual: number; count: number }
export interface Job { id: string; kind: string; status: string; progress: number; error: string | null; output_path: string | null; updated_at: number }
export interface ToolStatus { ffmpeg: string | null; ffprobe: string | null }

export const api = {
  listProjects: () => invoke<Project[]>("list_projects"),
  createProject: (name: string, kind: string) => invoke<string>("create_project", { name, kind }),
  renameProject: (id: string, name: string) => invoke<void>("rename_project", { id, name }),
  duplicateProject: (id: string) => invoke<string>("duplicate_project", { id }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),
  getSettings: () => invoke<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  listProviderStates: () => invoke<ProviderState[]>("list_provider_states"),
  setProviderEnabled: (id: string, enabled: boolean) => invoke<void>("set_provider_enabled", { id, enabled }),
  setProviderModel: (id: string, model: string) => invoke<void>("set_provider_model", { id, model }),
  saveApiKey: (id: string, key: string) => invoke<void>("save_api_key", { id, key }),
  removeApiKey: (id: string) => invoke<void>("remove_api_key", { id }),
  usageSummary: () => invoke<UsageSummary>("usage_summary"),
  listJobs: () => invoke<Job[]>("list_jobs"),
  listExports: () => invoke<[string, number][]>("list_exports"),
  toolStatus: () => invoke<ToolStatus>("tool_status"),
  probeMedia: (path: string) => invoke<string>("probe_media", { path }),
  startTrimExport: (input: string, output: string, start: number, end: number) =>
    invoke<string>("start_trim_export", { input, output, start, end }),
  cancelJob: (id: string) => invoke<void>("cancel_job", { id }),
};

export const errText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));
export const fmtTime = (s: number) => {
  const m = Math.floor(s / 60);
  return `${m}:${(s - m * 60).toFixed(1).padStart(4, "0")}`;
};
export const fmtDate = (unix: number) => new Date(unix * 1000).toLocaleString();
