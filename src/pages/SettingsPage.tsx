import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { enable as enableAutostart, disable as disableAutostart } from "@tauri-apps/plugin-autostart";
import { api } from "../api/client";
import type { AppSettings } from "../api/types";
import { useToast } from "../state/ToastContext";

export function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const toast = useToast();

  useEffect(() => {
    api.getSettings().then(setSettings);
  }, []);

  if (!settings) {
    return <div className="page-content empty-state">Loading settings…</div>;
  }

  async function update(patch: Partial<AppSettings>) {
    const next = { ...settings!, ...patch };
    setSettings(next);
    setSaving(true);
    try {
      await api.saveSettings(next);
    } catch (e) {
      toast.push(String(e), "error");
    } finally {
      setSaving(false);
    }
  }

  async function pickInstallDir() {
    const dir = await open({ directory: true, defaultPath: settings!.install_dir });
    if (typeof dir === "string") await update({ install_dir: dir });
  }

  async function pickDownloadDir() {
    const dir = await open({ directory: true, defaultPath: settings!.download_dir });
    if (typeof dir === "string") await update({ download_dir: dir });
  }

  async function toggleAutostart(checked: boolean) {
    try {
      if (checked) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
      await update({ start_with_windows: checked });
    } catch (e) {
      // Autostart isn't available in every dev environment (e.g. this Linux
      // container); still persist the preference so it takes effect on a
      // real Windows install.
      await update({ start_with_windows: checked });
      toast.push(`Autostart preference saved, but could not be applied here: ${e}`, "info");
    }
  }

  async function handleClearCache() {
    try {
      await api.clearImageCache();
      toast.push("Image cache cleared", "success");
    } catch (e) {
      toast.push(String(e), "error");
    }
  }

  async function handleReset() {
    await api.resetSettings();
    const fresh = await api.getSettings();
    setSettings(fresh);
    toast.push("Settings reset to defaults", "success");
  }

  return (
    <div className="page-content" style={{ maxWidth: 640 }}>
      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Storage</h2>
        </div>
        <div className="card-panel">
          <div className="form-field">
            <label className="form-label">Game installation directory</label>
            <div className="row">
              <input className="text-input" style={{ flex: 1 }} value={settings.install_dir} readOnly />
              <button className="btn btn-secondary btn-sm" onClick={pickInstallDir}>
                Browse…
              </button>
            </div>
          </div>
          <div className="form-field">
            <label className="form-label">Download directory</label>
            <div className="row">
              <input className="text-input" style={{ flex: 1 }} value={settings.download_dir} readOnly />
              <button className="btn btn-secondary btn-sm" onClick={pickDownloadDir}>
                Browse…
              </button>
            </div>
          </div>
        </div>
      </section>

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Downloads</h2>
        </div>
        <div className="card-panel">
          <div className="form-field">
            <label className="form-label">Maximum simultaneous downloads</label>
            <input
              className="text-input"
              type="number"
              min={1}
              max={10}
              style={{ maxWidth: 140 }}
              value={settings.max_concurrent_downloads}
              onChange={(e) => update({ max_concurrent_downloads: Number(e.target.value) })}
            />
          </div>
          <div className="form-field">
            <label className="form-label">Bandwidth limit (KB/s, 0 = unlimited)</label>
            <input
              className="text-input"
              type="number"
              min={0}
              style={{ maxWidth: 140 }}
              value={settings.bandwidth_limit_kbps}
              onChange={(e) => update({ bandwidth_limit_kbps: Number(e.target.value) })}
            />
          </div>
          <div className="row-between">
            <div>
              <label className="form-label">Automatically update sources</label>
              <div className="form-hint">Periodically re-fetch enabled catalogue sources in the background.</div>
            </div>
            <label className="switch">
              <input
                type="checkbox"
                checked={settings.auto_update_sources}
                onChange={(e) => update({ auto_update_sources: e.target.checked })}
              />
              <span className="switch-track" />
            </label>
          </div>
        </div>
      </section>

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Application</h2>
        </div>
        <div className="card-panel">
          <div className="form-field">
            <label className="form-label">Theme</label>
            <select
              className="select-input"
              style={{ maxWidth: 200 }}
              value={settings.theme}
              onChange={(e) => update({ theme: e.target.value })}
            >
              <option value="dark">Dark</option>
              <option value="light">Light (not yet styled)</option>
            </select>
          </div>
          <div className="row-between">
            <div>
              <label className="form-label">Start with Windows</label>
              <div className="form-hint">Launch Pirat Launcher automatically when you sign in.</div>
            </div>
            <label className="switch">
              <input type="checkbox" checked={settings.start_with_windows} onChange={(e) => toggleAutostart(e.target.checked)} />
              <span className="switch-track" />
            </label>
          </div>
        </div>
      </section>

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Maintenance</h2>
        </div>
        <div className="card-panel row" style={{ gap: 12 }}>
          <button className="btn btn-secondary" onClick={handleClearCache}>
            Clear Image Cache
          </button>
          <button className="btn btn-danger" onClick={handleReset}>
            Reset Settings
          </button>
          {saving && <span className="form-hint">Saving…</span>}
        </div>
      </section>
    </div>
  );
}
