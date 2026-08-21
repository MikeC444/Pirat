import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSettings,
  DownloadProgressPayload,
  DownloadTask,
  Game,
  GameQuery,
  GameSource,
  InstallOutcome,
  Installation,
  SourceSyncedPayload,
  SyncOutcome,
} from "./types";

// Every filesystem/network/DB operation goes through these typed wrappers —
// the React UI never touches Tauri's `invoke` directly, mirroring the
// front/back boundary the rest of the app is built around.

export const api = {
  // Sources
  listSources: () => invoke<GameSource[]>("list_sources"),
  addSource: (name: string, url: string, updateIntervalMinutes: number) =>
    invoke<GameSource>("add_source", { name, url, updateIntervalMinutes }),
  setSourceEnabled: (id: number, enabled: boolean) => invoke<void>("set_source_enabled", { id, enabled }),
  deleteSource: (id: number) => invoke<void>("delete_source", { id }),
  syncSource: (id: number) => invoke<SyncOutcome>("sync_source", { id }),
  syncAllSources: () => invoke<void>("sync_all_sources"),

  // Games
  listGames: (query: GameQuery) => invoke<Game[]>("list_games", { query }),
  getGame: (id: number) => invoke<Game | null>("get_game", { id }),
  listGenres: () => invoke<string[]>("list_genres"),
  recentlyAdded: (limit: number) => invoke<Game[]>("recently_added", { limit }),

  // Downloads
  listDownloads: () => invoke<DownloadTask[]>("list_downloads"),
  startDownload: (gameId: number) => invoke<number>("start_download", { gameId }),
  pauseDownload: (downloadId: number) => invoke<void>("pause_download", { downloadId }),
  resumeDownload: (downloadId: number) => invoke<void>("resume_download", { downloadId }),
  cancelDownload: (downloadId: number) => invoke<void>("cancel_download", { downloadId }),

  // Install
  installFromDownload: (downloadId: number) => invoke<InstallOutcome>("install_from_download", { downloadId }),
  listInstallations: () => invoke<Installation[]>("list_installations"),
  getInstallation: (gameId: number) => invoke<Installation | null>("get_installation", { gameId }),
  setInstallExecutable: (gameId: number, executablePath: string) =>
    invoke<void>("set_install_executable", { gameId, executablePath }),
  launchGame: (gameId: number) => invoke<void>("launch_game", { gameId }),

  // Settings
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (newSettings: AppSettings) => invoke<void>("save_settings", { newSettings }),
  resetSettings: () => invoke<void>("reset_settings"),
  clearImageCache: () => invoke<void>("clear_image_cache"),

  // Images
  fetchCoverImage: (url: string) => invoke<string>("fetch_cover_image", { url }),
};

export function onDownloadProgress(callback: (payload: DownloadProgressPayload) => void) {
  return listen<DownloadProgressPayload>("download://progress", (event) => callback(event.payload));
}

export function onSourceSynced(callback: (payload: SourceSyncedPayload) => void) {
  return listen<SourceSyncedPayload>("source://synced", (event) => callback(event.payload));
}
