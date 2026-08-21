import { useEffect, useState } from "react";
import { api, onSourceSynced } from "../api/client";
import type { Game } from "../api/types";
import { GameGrid } from "../components/GameGrid";

export function LibraryPage() {
  const [games, setGames] = useState<Game[]>([]);
  const [genres, setGenres] = useState<string[]>([]);
  const [search, setSearch] = useState("");
  const [genre, setGenre] = useState("");
  const [sortBy, setSortBy] = useState<"title" | "release_date" | "size_bytes" | "added_at">("title");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");
  const [loading, setLoading] = useState(true);

  async function load() {
    setLoading(true);
    const [gamesResult, genreList] = await Promise.all([
      api.listGames({
        search: search || undefined,
        genre: genre || undefined,
        sort_by: sortBy,
        sort_dir: sortDir,
        limit: 500,
      }),
      api.listGenres(),
    ]);
    setGames(gamesResult);
    setGenres(genreList);
    setLoading(false);
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [search, genre, sortBy, sortDir]);

  useEffect(() => {
    const unlisten = onSourceSynced(() => load());
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="page-content">
      <div className="row" style={{ marginBottom: 20, flexWrap: "wrap", gap: 12 }}>
        <div className="search-input-wrap" style={{ maxWidth: 320 }}>
          <span className="search-input-icon">⌕</span>
          <input
            className="search-input"
            placeholder="Search the library…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <select className="select-input" value={genre} onChange={(e) => setGenre(e.target.value)}>
          <option value="">All genres</option>
          {genres.map((g) => (
            <option key={g} value={g}>
              {g}
            </option>
          ))}
        </select>
        <select className="select-input" value={sortBy} onChange={(e) => setSortBy(e.target.value as typeof sortBy)}>
          <option value="title">Sort: Title</option>
          <option value="release_date">Sort: Release date</option>
          <option value="size_bytes">Sort: Size</option>
          <option value="added_at">Sort: Recently added</option>
        </select>
        <select className="select-input" value={sortDir} onChange={(e) => setSortDir(e.target.value as typeof sortDir)}>
          <option value="asc">Ascending</option>
          <option value="desc">Descending</option>
        </select>
      </div>

      {loading ? (
        <div className="empty-state">Loading…</div>
      ) : (
        <GameGrid games={games} emptyMessage="No games match your filters." />
      )}
    </div>
  );
}
