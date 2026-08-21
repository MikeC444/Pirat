export interface GameSource {
  id: number;
  name: string;
  url: string;
  enabled: boolean;
  update_interval_minutes: number;
  last_updated_at: string | null;
  last_error: string | null;
}

export interface Game {
  id: number;
  source_id: number;
  external_id: string;
  title: string;
  description: string;
  cover_url: string | null;
  version: string | null;
  size_bytes: number | null;
  release_date: string | null;
  genres: string[];
  developer: string | null;
  publisher: string | null;
  download_url: string | null;
  checksum: string | null;
  checksum_algo: string | null;
  added_at: string;
  updated_at: string;
}

export type DownloadState = "queued" | "downloading" | "paused" | "completed" | "failed" | "cancelled";

export interface DownloadTask {
  id: number;
  game_id: number;
  state: DownloadState;
  progress_bytes: number;
  total_bytes: number;
  speed_bps: number;
  started_at: string | null;
  updated_at: string;
  error: string | null;
  dest_path: string;
}

export interface Installation {
  id: number;
  game_id: number;
  install_path: string;
  executable_path: string | null;
  installed_at: string;
  version_installed: string | null;
}

export interface AppSettings {
  install_dir: string;
  download_dir: string;
  max_concurrent_downloads: number;
  bandwidth_limit_kbps: number;
  theme: string;
  start_with_windows: boolean;
  auto_update_sources: boolean;
}

export interface GameQuery {
  search?: string;
  genre?: string;
  sort_by?: "title" | "release_date" | "size_bytes" | "added_at" | "updated_at";
  sort_dir?: "asc" | "desc";
  source_id?: number;
  limit?: number;
  offset?: number;
}

export interface SyncOutcome {
  games_upserted: number;
  warnings: string[];
  error: string | null;
}

export interface InstallOutcome {
  install_path: string;
  suggested_executable: string | null;
}

export interface DownloadProgressPayload {
  download_id: number;
  game_id: number;
  state: DownloadState;
  progress_bytes: number;
  total_bytes: number;
  speed_bps: number;
  eta_seconds: number | null;
  error: string | null;
}

export interface SourceSyncedPayload {
  source_id: number;
  games_upserted: number;
  warnings: string[];
  error: string | null;
}
