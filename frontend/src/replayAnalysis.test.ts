import assert from 'node:assert/strict'
import test from 'node:test'
import { analysisPosition, reconcileOptimisticNode } from './replayAnalysis.ts'
import type { AnalysisNode, AnalysisTree } from './types/gameRecord.ts'
import type { GameState, TurnAction } from './types/game.ts'

const action = (piece: string): TurnAction => ({
  type: 'drop', player_id: 'white', piece_id: piece, to: { file: 0, rank: 0 },
})
const state = { current_player: 'white' } as GameState
const node = (id: string, parent: string | null, pending = false): AnalysisNode => ({
  id, parent_node_id: parent, pending, action: action(id), state_after: state,
  state_hash: id, created_at_ms: 1,
})
const tree = (nodes: AnalysisNode[]): AnalysisTree => ({
  id: 'tree', game_id: 'game', name: 'Variation 1', base_ply: 10, version: 2,
  created_at_ms: 1, updated_at_ms: 1, nodes,
})

test('four optimistic plies retain their ordered action suffix', () => {
  const nodes = [node('local-a', null, true), node('local-b', 'local-a', true), node('local-c', 'local-b', true), node('local-d', 'local-c', true)]
  assert.deepEqual(analysisPosition({ ...tree(nodes), id: 'local-tree' }, nodes[3], 0).pending_actions, nodes.map(item => item.action))
})

test('acknowledging a parent reparents queued children without moving the cursor', () => {
  const parent = node('local-b', 'server-a', true)
  const child = node('local-c', 'local-b', true)
  const value = tree([node('server-a', null), parent, child])
  const replacement = reconcileOptimisticNode(value, parent.id, node('server-b', 'server-a'))
  assert.equal(replacement?.id, 'server-b')
  assert.equal(child.parent_node_id, 'server-b')
  assert.deepEqual(analysisPosition(value, child, 0), {
    base_ply: 10, tree_id: 'tree', node_id: 'server-b', pending_actions: [child.action],
  })
})

test('returning to a persisted parent creates a sibling branch position', () => {
  const a = node('a', null), b = node('b', 'a'), c = node('c', 'b'), d = node('local-d', 'b', true)
  const value = tree([a, b, c, d])
  assert.deepEqual(analysisPosition(value, d, 0), {
    base_ply: 10, tree_id: 'tree', node_id: 'b', pending_actions: [d.action],
  })
})
