import type { BotTurnStats } from './types/game'

export interface BotDebugTurn {
  turnNumber: number
  action: string
  stats: BotTurnStats
}

export interface BotDebugSummary {
  turns: number
  totalNodes: number
  averageElapsedMs: number
  maxCompletedDepth: number
  ttHitRate: number | null
  nodesPerSecond: number | null
}

export function ratioPercent(numerator: number, denominator: number): number | null {
  return denominator > 0 ? (numerator / denominator) * 100 : null
}

export function summarizeBotDebugTurns(turns: BotDebugTurn[]): BotDebugSummary {
  const totalNodes = turns.reduce((sum, turn) => sum + turn.stats.searched_nodes, 0)
  const totalElapsedMs = turns.reduce((sum, turn) => sum + turn.stats.elapsed_ms, 0)
  const ttHits = turns.reduce((sum, turn) => sum + turn.stats.tt_hits, 0)
  const ttProbes = turns.reduce((sum, turn) => sum + turn.stats.tt_probes, 0)

  return {
    turns: turns.length,
    totalNodes,
    averageElapsedMs: turns.length > 0 ? totalElapsedMs / turns.length : 0,
    maxCompletedDepth: turns.reduce(
      (max, turn) => Math.max(max, turn.stats.completed_depth),
      0,
    ),
    ttHitRate: ratioPercent(ttHits, ttProbes),
    nodesPerSecond: totalElapsedMs > 0 ? (totalNodes * 1_000) / totalElapsedMs : null,
  }
}
