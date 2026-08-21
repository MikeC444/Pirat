import type { Game } from "../api/types";
import { GameCard } from "./GameCard";

export function GameGrid({ games, emptyMessage, badge }: { games: Game[]; emptyMessage?: string; badge?: (g: Game) => string | undefined }) {
  if (games.length === 0) {
    return <div className="empty-state">{emptyMessage ?? "No games to show yet."}</div>;
  }
  return (
    <div className="game-grid">
      {games.map((g) => (
        <GameCard key={g.id} game={g} badge={badge?.(g)} />
      ))}
    </div>
  );
}
