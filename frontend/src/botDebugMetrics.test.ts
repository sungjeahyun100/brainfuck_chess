import assert from 'node:assert/strict'
import test from 'node:test'
import { ratioPercent, summarizeBotDebugTurns, type BotDebugTurn } from './botDebugMetrics.ts'
import type { BotTurnStats } from './types/game.ts'

function stats(overrides: Partial<BotTurnStats> = {}): BotTurnStats {
  return {
    score: 0,
    searched_nodes: 0,
    depth_reached: 0,
    completed_depth: 0,
    iterations_started: 0,
    iterations_completed: 0,
    qnodes: 0,
    beta_cutoffs: 0,
    tt_probes: 0,
    tt_hits: 0,
    tt_cutoffs: 0,
    tt_stores: 0,
    aspiration_searches: 0,
    aspiration_researches: 0,
    aspiration_fail_lows: 0,
    aspiration_fail_highs: 0,
    generated_legal_actions: 0,
    unique_canonical_actions: 0,
    beam_selected_actions: 0,
    mandatory_tactical_actions: 0,
    drop_actions_generated: 0,
    drop_actions_selected: 0,
    board_optional_actions_generated: 0,
    board_optional_actions_selected: 0,
    quiet_drop_actions_generated: 0,
    quiet_drop_actions_selected: 0,
    normal_nodes: 0,
    move_generation_nanos: 0,
    canonical_deduplication_nanos: 0,
    move_ordering_nanos: 0,
    root_generated_legal_actions: 0,
    root_unique_canonical_actions: 0,
    root_beam_selected_actions: 0,
    root_mandatory_tactical_actions: 0,
    root_drop_actions_generated: 0,
    root_drop_actions_selected: 0,
    root_board_optional_actions_generated: 0,
    root_board_optional_actions_selected: 0,
    root_quiet_drop_actions_generated: 0,
    root_quiet_drop_actions_selected: 0,
    elapsed_ms: 0,
    ...overrides,
  }
}

test('ratioPercent distinguishes unavailable ratios from zero percent', () => {
  assert.equal(ratioPercent(0, 0), null)
  assert.equal(ratioPercent(0, 5), 0)
  assert.equal(ratioPercent(2, 5), 40)
})

test('summarizeBotDebugTurns aggregates a bot test session', () => {
  const turns: BotDebugTurn[] = [
    { turnNumber: 1, action: 'move', stats: stats({ searched_nodes: 900, elapsed_ms: 100, completed_depth: 3, tt_probes: 10, tt_hits: 4 }) },
    { turnNumber: 2, action: 'drop', stats: stats({ searched_nodes: 2_100, elapsed_ms: 200, completed_depth: 4, tt_probes: 30, tt_hits: 16 }) },
  ]

  assert.deepEqual(summarizeBotDebugTurns(turns), {
    turns: 2,
    totalNodes: 3_000,
    averageElapsedMs: 150,
    maxCompletedDepth: 4,
    ttHitRate: 50,
    nodesPerSecond: 10_000,
  })
})
