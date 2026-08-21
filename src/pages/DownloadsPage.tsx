import { useEffect, useState } from "react";
import { api, onDownloadProgress } from "../api/client";
import type { DownloadTask, Game } from "../api/types";
import { DownloadRow } from "../components/DownloadRow";
import { useToast } from "../state/ToastContext";

export function DownloadsPage() {
  const [downloads, setDownloads] = useState<DownloadTask[]>([]);
  const [games, setGames] = useState<Record<number, Game>>({});
  const [installingId, setInstallingId] = useState<number | null>(null);
  const toast = useToast();

  async function load() {
    const tasks = await api.listDownloads();
    setDownloads(tasks);
    const missingIds = [...new Set(tasks.map((t) => t.game_id))];
    const fetched = await Promise.all(missingIds.map((id) => api.getGame(id)));
    setGames((prev) => {
      const next = { ...prev };
      fetched.forEach((g) => {
        if (g) next[g.id] = g;
      });
      return next;
    });
  }

  useEffect(() => {
    load();
  }, []);

  useEffect(() => {
    const unlisten = onDownloadProgress((payload) => {
      setDownloads((prev) => {
        const idx = prev.findIndex((d) => d.id === payload.download_id);
        const updated: DownloadTask = {
          id: payload.download_id,
          game_id: payload.game_id,
          state: payload.state,
          progress_bytes: payload.progress_bytes,
          total_bytes: payload.total_bytes,
          speed_bps: payload.speed_bps,
          started_at: idx >= 0 ? prev[idx].started_at : null,
          updated_at: new Date().toISOString(),
          error: payload.error,
          dest_path: idx >= 0 ? prev[idx].dest_path : "",
        };
        if (idx >= 0) {
          const copy = [...prev];
          copy[idx] = updated;
          return copy;
        }
        return [updated, ...prev];
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  async function handleInstall(task: DownloadTask) {
    setInstallingId(task.id);
    try {
      const result = await api.installFromDownload(task.id);
      toast.push(`Installed ${games[task.game_id]?.title ?? "game"} to ${result.install_path}`, "success");
    } catch (e) {
      toast.push(String(e), "error");
    } finally {
      setInstallingId(null);
    }
  }

  const active = downloads.filter((d) => d.state === "downloading" || d.state === "queued" || d.state === "paused");
  const finished = downloads.filter((d) => d.state === "completed" || d.state === "failed" || d.state === "cancelled");

  return (
    <div className="page-content">
      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Active Downloads</h2>
          <span className="section-subtitle">{active.length}</span>
        </div>
        {active.length === 0 ? (
          <div className="empty-state">No active downloads. Start one from a game's detail page.</div>
        ) : (
          active.map((task) => (
            <DownloadRow
              key={task.id}
              task={task}
              game={games[task.game_id]}
              installing={installingId === task.id}
              onPause={() => api.pauseDownload(task.id).catch((e) => toast.push(String(e), "error"))}
              onResume={() =>
                api.resumeDownload(task.id).catch((e) => toast.push(String(e), "error"))
              }
              onCancel={() => api.cancelDownload(task.id).catch((e) => toast.push(String(e), "error"))}
              onInstall={() => handleInstall(task)}
            />
          ))
        )}
      </section>

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">History</h2>
          <span className="section-subtitle">{finished.length}</span>
        </div>
        {finished.length === 0 ? (
          <div className="empty-state">Completed, failed, and cancelled downloads will appear here.</div>
        ) : (
          finished.map((task) => (
            <DownloadRow
              key={task.id}
              task={task}
              game={games[task.game_id]}
              installing={installingId === task.id}
              onPause={() => {}}
              onResume={() => api.resumeDownload(task.id).catch((e) => toast.push(String(e), "error"))}
              onCancel={() => api.cancelDownload(task.id).catch((e) => toast.push(String(e), "error"))}
              onInstall={() => handleInstall(task)}
            />
          ))
        )}
      </section>
    </div>
  );
}
