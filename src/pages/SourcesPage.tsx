import { useEffect, useState } from "react";
import { api } from "../api/client";
import type { GameSource } from "../api/types";
import { useToast } from "../state/ToastContext";
import { formatDate } from "../utils/format";

export function SourcesPage() {
  const [sources, setSources] = useState<GameSource[]>([]);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [interval, setInterval_] = useState(60);
  const [submitting, setSubmitting] = useState(false);
  const [syncingId, setSyncingId] = useState<number | null>(null);
  const toast = useToast();

  async function load() {
    setSources(await api.listSources());
  }

  useEffect(() => {
    load();
  }, []);

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim() || !url.trim()) return;
    setSubmitting(true);
    try {
      await api.addSource(name.trim(), url.trim(), interval);
      toast.push(`Added source "${name.trim()}"`, "success");
      setName("");
      setUrl("");
      await load();
    } catch (e) {
      toast.push(String(e), "error");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleSync(id: number) {
    setSyncingId(id);
    try {
      const outcome = await api.syncSource(id);
      if (outcome.error) {
        toast.push(`Sync failed: ${outcome.error}`, "error");
      } else {
        toast.push(
          `Synced: ${outcome.games_upserted} games${outcome.warnings.length ? `, ${outcome.warnings.length} warnings` : ""}`,
          "success"
        );
      }
      await load();
    } finally {
      setSyncingId(null);
    }
  }

  async function handleToggle(source: GameSource) {
    await api.setSourceEnabled(source.id, !source.enabled);
    await load();
  }

  async function handleDelete(id: number) {
    await api.deleteSource(id);
    await load();
  }

  return (
    <div className="page-content">
      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Add a Catalogue Source</h2>
        </div>
        <form className="card-panel" onSubmit={handleAdd} style={{ maxWidth: 560 }}>
          <div className="form-field">
            <label className="form-label">Name</label>
            <input className="text-input" value={name} onChange={(e) => setName(e.target.value)} placeholder="My Game Catalogue" />
          </div>
          <div className="form-field">
            <label className="form-label">JSON URL</label>
            <input
              className="text-input"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/games.json"
            />
            <span className="form-hint">
              http://, https://, or file:// (use file://{"{repo}"}/example-data/games.json for the bundled sample catalogue).
            </span>
          </div>
          <div className="form-field">
            <label className="form-label">Update interval (minutes)</label>
            <input
              className="text-input"
              type="number"
              min={5}
              value={interval}
              onChange={(e) => setInterval_(Number(e.target.value))}
              style={{ maxWidth: 140 }}
            />
          </div>
          <button className="btn btn-primary" type="submit" disabled={submitting}>
            {submitting ? <span className="spinner" /> : "Add Source"}
          </button>
        </form>
      </section>

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Sources</h2>
          <span className="section-subtitle">{sources.length}</span>
        </div>
        {sources.length === 0 ? (
          <div className="empty-state">No sources configured yet.</div>
        ) : (
          sources.map((s) => (
            <div key={s.id} className="card-panel" style={{ marginBottom: 12 }}>
              <div className="row-between">
                <div>
                  <div style={{ fontWeight: 700 }}>{s.name}</div>
                  <div className="form-hint">{s.url}</div>
                  <div className="form-hint">
                    {s.last_updated_at ? `Last synced ${formatDate(s.last_updated_at)}` : "Never synced"}
                    {s.last_error && <span style={{ color: "var(--danger)" }}> · {s.last_error}</span>}
                  </div>
                </div>
                <div className="row">
                  <label className="switch">
                    <input type="checkbox" checked={s.enabled} onChange={() => handleToggle(s)} />
                    <span className="switch-track" />
                  </label>
                  <button className="btn btn-secondary btn-sm" onClick={() => handleSync(s.id)} disabled={syncingId === s.id}>
                    {syncingId === s.id ? <span className="spinner" /> : "Sync Now"}
                  </button>
                  <button className="btn btn-danger btn-sm" onClick={() => handleDelete(s.id)}>
                    Remove
                  </button>
                </div>
              </div>
            </div>
          ))
        )}
      </section>
    </div>
  );
}
