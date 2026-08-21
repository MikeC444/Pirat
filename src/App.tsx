import { HashRouter, Route, Routes } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { ToastProvider } from "./state/ToastContext";
import { HomePage } from "./pages/HomePage";
import { LibraryPage } from "./pages/LibraryPage";
import { GameDetailPage } from "./pages/GameDetailPage";
import { DownloadsPage } from "./pages/DownloadsPage";
import { SourcesPage } from "./pages/SourcesPage";
import { SettingsPage } from "./pages/SettingsPage";

export default function App() {
  return (
    <ToastProvider>
      <HashRouter>
        <div className="app-shell">
          <Sidebar />
          <div className="main-area">
            <Routes>
              <Route path="/" element={<HomePage />} />
              <Route path="/library" element={<LibraryPage />} />
              <Route path="/game/:id" element={<GameDetailPage />} />
              <Route path="/downloads" element={<DownloadsPage />} />
              <Route path="/sources" element={<SourcesPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </div>
        </div>
      </HashRouter>
    </ToastProvider>
  );
}
