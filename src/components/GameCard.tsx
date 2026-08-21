import { useNavigate } from "react-router-dom";
import type { Game } from "../api/types";
import { CachedImage } from "./CachedImage";
import { formatBytes } from "../utils/format";

export function GameCard({ game, badge }: { game: Game; badge?: string }) {
  const navigate = useNavigate();
  return (
    <div className="game-card" onClick={() => navigate(`/game/${game.id}`)}>
      <div style={{ position: "relative" }}>
        <CachedImage url={game.cover_url} alt={game.title} className="game-card-cover" />
        {badge && <span className="game-card-badge">{badge}</span>}
      </div>
      <div className="game-card-body">
        <div className="game-card-title">{game.title}</div>
        <div className="game-card-meta">
          {game.genres.slice(0, 2).join(", ") || "Uncategorized"} · {formatBytes(game.size_bytes)}
        </div>
      </div>
    </div>
  );
}
