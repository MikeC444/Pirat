import type { DownloadTask, Game } from "../api/types";
import { formatBytes, formatEta, formatSpeed } from "../utils/format";

interface Props {
  task: DownloadTask;
  game: Game | undefined;
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
  onInstall: () => void;
  installing: boolean;
}

export function DownloadRow({ task, game, onPause, onResume, onCancel, onInstall, installing }: Props) {
  const pct = task.total_bytes > 0 ? Math.min(100, (task.progress_bytes / task.total_bytes) * 100) : 0;
  const eta = task.speed_bps > 0 && task.total_bytes > task.progress_bytes ? (task.total_bytes - task.progress_bytes) / task.speed_bps : null;

  return (
    <div className="card-panel" style={{ marginBottom: 12 }}>
      <div className="row-between" style={{ marginBottom: 10 }}>
        <div>
          <div style={{ fontWeight: 700, fontSize: 14 }}>{game?.title ?? `Game #${task.game_id}`}</div>
          <div className="form-hint">
            {formatBytes(task.progress_bytes)} / {formatBytes(task.total_bytes)}
          </div>
        </div>
        <span className={`state-tag state-${task.state}`}>{task.state}</span>
      </div>

      <div className="progress-track">
        <div className={`progress-fill state-${task.state}`} style={{ width: `${pct}%` }} />
      </div>

      <div className="row-between" style={{ marginTop: 10 }}>
        <div className="form-hint">
          {task.state === "downloading" && `${formatSpeed(task.speed_bps)} · ETA ${formatEta(eta)}`}
          {task.error && <span style={{ color: "var(--danger)" }}>{task.error}</span>}
        </div>
        <div className="row">
          {task.state === "downloading" && (
            <button className="btn btn-secondary btn-sm" onClick={onPause}>
              Pause
            </button>
          )}
          {(task.state === "paused" || task.state === "failed") && (
            <button className="btn btn-secondary btn-sm" onClick={onResume}>
              Resume
            </button>
          )}
          {task.state === "completed" ? (
            <button className="btn btn-primary btn-sm" onClick={onInstall} disabled={installing}>
              {installing ? <span className="spinner" /> : "Install"}
            </button>
          ) : (
            task.state !== "cancelled" && (
              <button className="btn btn-danger btn-sm" onClick={onCancel}>
                Cancel
              </button>
            )
          )}
        </div>
      </div>
    </div>
  );
}
