import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api, onDownloadProgress } from "../api/client";
import type { DownloadTask, Game, Installation } from "../api/types";
import { CachedImage } from "../components/CachedImage";
import { useToast } from "../state/ToastContext";
import { formatBytes, formatDate, formatEta, formatSpeed } from "../utils/format";

export function GameDetailPage() {
  const { id } = useParams<{ id: string }>();
  const gameId = Number(id);
  const navigate = useNavigate();
  const toast = useToast();

  const [game, setGame] = useState<Game | null>(null);
  const [installation, setInstallation] = useState<Installation | null>(null);
  const [download, setDownload] = useState<DownloadTask | null>(null);
  const [busy, setBusy] = useState(false);

  async function load() {
    const [g, install, downloads] = await Promise.all([
      api.getGame(gameId),
      api.getInstallation(gameId),
      api.listDownloads(),
    ]);
    setGame(g);
    setInstallation(install);
    const relevant = downloads
      .filter((d) => d.game_id === gameId)
      .sort((a, b) => b.id - a.id)[0];
    setDownload(relevant ?? null);
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gameId]);

  useEffect(() => {
    const unlisten = onDownloadProgress((payload) => {
      if (payload.game_id !== gameId) return;
      setDownload((prev) => ({
        id: payload.download_id,
        game_id: payload.game_id,
        state: payload.state,
        progress_bytes: payload.progress_bytes,
        total_bytes: payload.total_bytes,
        speed_bps: payload.speed_bps,
        started_at: prev?.started_at ?? null,
        updated_at: new Date().toISOString(),
        error: payload.error,
        dest_path: prev?.dest_path ?? "",
      }));
      if (payload.state === "completed") {
        api.getInstallation(gameId).then(setInstallation);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [gameId]);

  if (!game) {
    return <div className="page-content empty-state">Loading…</div>;
  }

  async function handleDownload() {
    setBusy(true);
    try {
      await api.startDownload(gameId);
      toast.push(`Started downloading ${game!.title}`, "success");
      await load();
    } catch (e) {
      toast.push(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleInstall() {
    if (!download) return;
    setBusy(true);
    try {
      const result = await api.installFromDownload(download.id);
      toast.push(`Installed to ${result.install_path}`, "success");
      await load();
    } catch (e) {
      toast.push(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleLaunch() {
    setBusy(true);
    try {
      await api.launchGame(gameId);
    } catch (e) {
      toast.push(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  const pct = download && download.total_bytes > 0 ? Math.min(100, (download.progress_bytes / download.total_bytes) * 100) : 0;
  const eta = download && download.speed_bps > 0 && download.total_bytes > download.progress_bytes
    ? (download.total_bytes - download.progress_bytes) / download.speed_bps
    : null;

  return (
    <div className="page-content">
      <button className="btn btn-secondary btn-sm" style={{ marginBottom: 18 }} onClick={() => navigate(-1)}>
        ← Back
      </button>

      <div className="detail-hero">
        <CachedImage url={game.cover_url} alt={game.title} className="detail-cover" />
        <div className="detail-info">
          <h1 className="detail-title">{game.title}</h1>
          <div className="detail-sub">
            {game.developer ?? "Unknown developer"}
            {game.publisher && game.publisher !== game.developer ? ` · Published by ${game.publisher}` : ""}
          </div>

          <div className="chip-row">
            {game.genres.map((g) => (
              <span key={g} className="chip">
                {g}
              </span>
            ))}
          </div>

          <div className="meta-grid">
            <div>
              <div className="meta-item-label">Version</div>
              <div className="meta-item-value">{game.version ?? "Unknown"}</div>
            </div>
            <div>
              <div className="meta-item-label">Size</div>
              <div className="meta-item-value">{formatBytes(game.size_bytes)}</div>
            </div>
            <div>
              <div className="meta-item-label">Release Date</div>
              <div className="meta-item-value">{formatDate(game.release_date)}</div>
            </div>
            <div>
              <div className="meta-item-label">Checksum</div>
              <div className="meta-item-value">{game.checksum ? `${game.checksum_algo} verified` : "Not provided"}</div>
            </div>
          </div>

          <div style={{ marginTop: "auto", paddingTop: 12 }}>
            {installation ? (
              <div className="row">
                <button className="btn btn-primary" onClick={handleLaunch} disabled={busy || !installation.executable_path}>
                  ▶ Launch
                </button>
                <span className="form-hint">Installed at {installation.install_path}</span>
              </div>
            ) : download && download.state === "completed" ? (
              <button className="btn btn-primary" onClick={handleInstall} disabled={busy}>
                {busy ? <span className="spinner" /> : "Install"}
              </button>
            ) : download && (download.state === "downloading" || download.state === "paused" || download.state === "queued") ? (
              <div style={{ maxWidth: 360 }}>
                <div className="progress-track" style={{ marginBottom: 8 }}>
                  <div className={`progress-fill state-${download.state}`} style={{ width: `${pct}%` }} />
                </div>
                <div className="form-hint">
                  {formatBytes(download.progress_bytes)} / {formatBytes(download.total_bytes)}
                  {download.state === "downloading" && ` · ${formatSpeed(download.speed_bps)} · ETA ${formatEta(eta)}`}
                  {download.state === "paused" && " · Paused"}
                </div>
                <div className="row" style={{ marginTop: 8 }}>
                  {download.state === "downloading" ? (
                    <button className="btn btn-secondary btn-sm" onClick={() => api.pauseDownload(download.id)}>
                      Pause
                    </button>
                  ) : (
                    <button className="btn btn-secondary btn-sm" onClick={() => api.resumeDownload(download.id)}>
                      Resume
                    </button>
                  )}
                  <button className="btn btn-danger btn-sm" onClick={() => api.cancelDownload(download.id)}>
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <button className="btn btn-primary" onClick={handleDownload} disabled={busy || !game.download_url}>
                {busy ? <span className="spinner" /> : "⤓ Download"}
              </button>
            )}
            {!game.download_url && !installation && (
              <div className="form-hint" style={{ marginTop: 8 }}>
                This catalogue entry has no download source configured.
              </div>
            )}
          </div>
        </div>
      </div>

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">About</h2>
        </div>
        <p className="detail-description">{game.description || "No description provided."}</p>
      </section>
    </div>
  );
}
