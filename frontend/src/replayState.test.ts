import assert from 'node:assert/strict'
import test from 'node:test'
import { applyStateDelta, buildReplayFrames } from './replayState.ts'
import type { GameState } from './types/game.ts'
import type { GameRecord } from './types/gameRecord.ts'

test('applies set/remove replay effects without mutating the previous frame', () => {
  const state = { board: { size: 8, squares: { e3: 'p1', e5: null } }, pieces: { p1: { current_square: { file: 4, rank: 2 } } }, history: [{ action: {} }] } as unknown as GameState
  const next = applyStateDelta(state, [
    { op: 'set', path: ['board', 'squares', 'e3'], value: null },
    { op: 'set', path: ['board', 'squares', 'e5'], value: 'p1' },
    { op: 'set', path: ['pieces', 'p1', 'current_square'], value: { file: 4, rank: 4 } },
  ])
  assert.equal(state.board.squares.e3, 'p1'); assert.equal(next.board.squares.e5, 'p1'); assert.deepEqual(next.history, [])
})

test('reconstructs drop, ammo, air layer, forced landing and transform effects as state changes', () => {
  const state = { board: { size: 12, squares: {}, air_squares: { e11: 'b1' } }, pieces: { b1: { type_id: 'bomber', current_square: { file: 4, rank: 10 }, current_ammo: 2, layer: 'air', remaining_flight_turns: 1, state: {} }, p1: { type_id: 'paratrooper', in_pocket: true } }, players: { white: { deck: { pocket_pieces: ['p1'] } } }, history: [] } as unknown as GameState
  const next = applyStateDelta(state, [
    { op: 'set', path: ['board', 'air_squares', 'e11'], value: null }, { op: 'set', path: ['board', 'squares', 'k4'], value: 'b1' },
    { op: 'set', path: ['pieces', 'b1', 'current_square'], value: { file: 10, rank: 3 } }, { op: 'set', path: ['pieces', 'b1', 'layer'], value: 'ground' },
    { op: 'set', path: ['pieces', 'b1', 'remaining_flight_turns'], value: 0 }, { op: 'set', path: ['pieces', 'b1', 'current_ammo'], value: 1 },
    { op: 'set', path: ['pieces', 'b1', 'type_id'], value: 'veteran-bomber' }, { op: 'set', path: ['pieces', 'p1', 'in_pocket'], value: false },
    { op: 'set', path: ['players', 'white', 'deck', 'pocket_pieces'], value: [] },
  ])
  assert.equal(next.pieces.b1.layer, 'ground'); assert.equal(next.pieces.b1.current_ammo, 1); assert.equal(next.pieces.b1.type_id, 'veteran-bomber')
  assert.equal(next.board.squares.k4, 'b1'); assert.equal(next.pieces.p1.in_pocket, false); assert.deepEqual(next.players.white.deck.pocket_pieces, [])
})

test('builds one frame per same-player canonical action in exact recorded order', () => {
  const initial = { board: { size: 12, squares: {}, air_squares: { e11: 'b' } }, pieces: { b: { current_square: { file: 4, rank: 10 }, layer: 'air' } }, history: [] } as unknown as GameState
  const record = { initial_state: initial, actions: [
    { state_delta: [{ op: 'set', path: ['pieces', 'b', 'current_square'], value: { file: 4, rank: 3 } }] },
    { state_delta: [{ op: 'set', path: ['pieces', 'b', 'current_square'], value: { file: 10, rank: 3 } }] },
    { state_delta: [{ op: 'set', path: ['current_player'], value: 'white' }] },
  ] } as unknown as GameRecord
  const frames = buildReplayFrames(record)
  assert.equal(frames.length, 4)
  assert.deepEqual(frames[0].pieces.b.current_square, { file: 4, rank: 10 })
  assert.deepEqual(frames[1].pieces.b.current_square, { file: 4, rank: 3 })
  assert.deepEqual(frames[2].pieces.b.current_square, { file: 10, rank: 3 })
})
