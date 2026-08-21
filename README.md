# Pirat Launcher

A desktop game-library launcher: fetches a game catalogue from a configurable
JSON URL, lets you browse/search/filter it, and manages downloads and
installs from lawful HTTP/HTTPS sources — all backed by a local SQLite
database. Built with **Tauri 2** (Rust backend) and **React + TypeScript**
(Vite frontend), targeting **Windows 10/11**.

## Architecture

```
src/                      React + TypeScript frontend (Vite)
  api/                     typed wrappers around Tauri invoke() + event listeners
  components/              Sidebar, GameCard/Grid, DownloadRow, CachedImage, ...
  pages/                   Home, Library, GameDetail, Downloads, Sources, Settings
  state/                   Toast notifications context
  styles/                  dark theme (CSS variables)

src-tauri/                 Rust backend (Tauri app)
  src/
    db/                     SQLite schema + one repository module per table
    sources/                GameSource fetch/parse/schedule + tolerant JSON parser
    downloads/               DownloadProvider trait + HttpDownloadProvider + DownloadManager
    install/                 checksum verification + zip-slip-safe extraction + launch
    images/                  disk-cached cover art downloader
    settings/                typed settings service
    logging/                 file + console logging (tracing)
    commands.rs              all #[tauri::command] entry points the frontend calls
    state.rs                 shared AppState (DB handle, HTTP client, DownloadManager)

example-data/games.json    bundled sample catalogue (7 fictional games) for offline testing
```

The frontend never touches the filesystem, network, or database directly —
every action goes through a typed command in `src/api/client.ts` calling into
Rust. `SourceManager`/`sources::sync_source` own fetching and parsing a
catalogue and writing it into SQLite, so the library keeps working from the
last successful sync even when a source is temporarily unreachable.
Downloads go through a `DownloadProvider` trait (`src-tauri/src/downloads/provider.rs`);
only `HttpDownloadProvider` (ordinary HTTP/HTTPS with Range-based resume) is
implemented, but new lawful providers can be added without touching the
download manager or UI.

## Features implemented

- **Catalogue system**: fetch a JSON catalogue from any `http://`, `https://`,
  or `file://` URL; tolerant parser that fills in sane defaults for missing
  optional fields and reports clear per-field errors for malformed ones
  without discarding the rest of the catalogue.
- **Sources**: add/enable/disable/remove sources, manual "Sync Now", and a
  background scheduler that re-syncs each enabled source on its configured
  interval.
- **Library**: search, genre filter, and sort (title / release date / size /
  recently added) backed by real SQL queries — not client-side filtering.
- **Game detail page**: cover art, description, version, size, release date,
  genres, developer/publisher, checksum status.
- **Downloads**: real HTTP/HTTPS downloads with live progress %, speed, ETA,
  pause/resume (via HTTP Range requests), cancel, and persisted
  queued/downloading/paused/completed/failed/cancelled state — survives app
  restarts.
- **Install pipeline**: SHA-256 checksum verification when a catalogue
  provides one, zip-slip-safe archive extraction (path traversal is
  rejected), placement into the configured install directory, and an
  installation record. Nothing downloaded is ever auto-executed — launching
  is a separate, explicit action.
- **Settings**: install/download directories (native folder picker), max
  concurrent downloads, bandwidth limit, theme, start-with-Windows
  (`tauri-plugin-autostart`), auto-update sources, clear image cache, reset
  settings — all persisted in SQLite and reloaded on next launch.
- **Image cache**: cover art is downloaded once per URL and cached to disk;
  later loads (including across restarts) reuse the cached file.
- **Offline resilience**: if a source is unreachable, the previously cached
  catalogue stays fully browsable and an error is recorded on the source
  instead of clearing data.

### Known limitations

- No "Popular" home section — there's no popularity signal in the schema
  (download counts, ratings, etc. aren't part of the catalogue format), so it
  was left out rather than faked with an arbitrary sort.
- Light theme is selectable in Settings but not yet styled (dark is the only
  fully designed theme).
- Bandwidth limiting is a simple per-chunk sleep-based throttle, not a
  precise token-bucket shaper.
- `tauri-plugin-autostart`'s actual registry effect only manifests on
  Windows; it was exercised at the API-call level in this Linux build
  environment, not against a real Windows registry.

## Running in development

Requires Node.js 18+ and a Rust toolchain (`rustup`).

```bash
npm install
npm run tauri dev
```

On Linux you'll also need the WebView/GTK dev packages Tauri uses to render
its window (already handled if you're on Windows, where WebView2 ships with
the OS):

```bash
sudo apt install libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev build-essential
```

To try the app without any external network dependency, go to **Sources**
and add `file:///absolute/path/to/example-data/games.json` as a source — it
loads the bundled 7-game sample catalogue.

### Running the backend's automated tests

```bash
cd src-tauri
cargo test
```

25 tests cover the database repositories, the tolerant JSON catalogue
parser, HTTP downloads (progress/resume), checksum verification, zip-slip
rejection, settings persistence, and a full end-to-end flow (add source →
sync → search/filter → download → install).

## Building the Windows installer

The project is configured to bundle an NSIS installer for Windows
(`src-tauri/tauri.conf.json` → `bundle.targets: ["nsis"]`).

**On a Windows machine** (recommended path):

```powershell
npm install
npm run tauri build
```

The installer is written to
`src-tauri/target/release/bundle/nsis/Pirat Launcher_<version>_x64-setup.exe`.

**Cross-compiling from Linux/macOS** (what this environment used to produce
the installer attached earlier in this conversation): install the MinGW-w64
toolchain and NSIS, add the Rust target, then build against it:

```bash
sudo apt install mingw-w64 nsis
rustup target add x86_64-pc-windows-gnu
npm install
npm run build
npx tauri build --target x86_64-pc-windows-gnu --bundles nsis
```

This produced a verified `PE32+` Windows executable and a working NSIS
installer directly from this Linux container — confirmed with `file` against
both `pirat-launcher.exe` and the `...-setup.exe`. Cross-compiled builds are
still marked experimental by Tauri, so the officially supported path for a
release build remains building on Windows itself or in Windows CI (e.g.
GitHub Actions `windows-latest`):

```yaml
runs-on: windows-latest
steps:
  - uses: actions/checkout@v4
  - uses: actions/setup-node@v4
    with: { node-version: 20 }
  - uses: dtolnay/rust-toolchain@stable
  - run: npm install
  - run: npm run tauri build
```

## Adding a new download source (lawful, HTTP/HTTPS only)

The download system is provider-based
(`src-tauri/src/downloads/provider.rs`). To add another lawful source type,
implement `DownloadProvider` (`supports`, `download`) and register it in
`DownloadManager::new` (`src-tauri/src/downloads/mod.rs`) — no other code
needs to change. This project intentionally implements only ordinary
HTTP/HTTPS downloads; it does not implement DRM circumvention, torrent/P2P
automation, or any other approach to unauthorized copyrighted-game
distribution.
