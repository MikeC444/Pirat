import { useEffect, useState } from "react";
import { api } from "../api/client";
import { onSourceSynced } from "../api/client";
import type { Game, Installation } from "../api/types";
import { GameGrid } from "../components/GameGrid";

export function HomePage() {
  const [recent, setRecent] = useState<Game[]>([]);
  const [allGames, setAllGames] = useState<Game[]>([]);
  const [installations, setInstallations] = useState<Installation[]>([]);
  const [loading, setLoading] = useState(true);

  async function load() {
    const [recentGames, catalogue, installed] = await Promise.all([
      api.recentlyAdded(10),
      api.listGames({ sort_by: "title", sort_dir: "asc", limit: 500 }),
      api.listInstallations(),
    ]);
    setRecent(recentGames);
    setAllGames(catalogue);
    setInstallations(installed);
    setLoading(false);
  }

  useEffect(() => {
    load();
    const unlisten = onSourceSynced(() => load());
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const installedGameIds = new Set(installations.map((i) => i.game_id));
  const installedGames = allGames.filter((g) => installedGameIds.has(g.id));

  if (loading) {
    return <div className="page-content empty-state">Loading your library…</div>;
  }

  return (
    <div className="page-content">
      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Recently Added</h2>
          <span className="section-subtitle">{recent.length} games</span>
        </div>
        <GameGrid games={recent} emptyMessage="Add a source in Sources to populate your catalogue." />
      </section>

      {installedGames.length > 0 && (
        <section className="section">
          <div className="section-header">
            <h2 className="section-title">Installed</h2>
            <span className="section-subtitle">{installedGames.length} games</span>
          </div>
          <GameGrid games={installedGames} badge={() => "Installed"} />
        </section>
      )}

      <section className="section">
        <div className="section-header">
          <h2 className="section-title">All Games</h2>
          <span className="section-subtitle">{allGames.length} games</span>
        </div>
        <GameGrid games={allGames.slice(0, 24)} />
      </section>
    </div>
  );
}
