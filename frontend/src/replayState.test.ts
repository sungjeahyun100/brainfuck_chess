import { LEGACY_RULES_VERSION, STANDARD_RULES_VERSION } from './gameRulesVersions.ts'
import assert from 'node:assert/strict'
import test from 'node:test'
import { reactive, readonly } from 'vue'
import { applyStateDelta, buildReplayFrames, buildReplayFramesResult } from './replayState.ts'
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
  const record = { ruleset_version: LEGACY_RULES_VERSION, initial_state: initial, actions: [
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

test('reconstructs readonly Vue proxy records and nested reactive delta values', () => {
  const initial = reactive({
    board: { size: 8, squares: { e3: 'p1', e5: null } },
    pieces: { p1: { current_square: { file: 4, rank: 2 } } },
    piece_definitions: {},
    history: [],
  }) as unknown as GameState
  const deltaValue = reactive({ file: 4, rank: 4 })
  const record = readonly({
    ruleset_version: LEGACY_RULES_VERSION,
    initial_state: initial,
    actions: [{ state_delta: [
      { op: 'set' as const, path: ['board', 'squares', 'e3'], value: null },
      { op: 'set' as const, path: ['board', 'squares', 'e5'], value: 'p1' },
      { op: 'set' as const, path: ['pieces', 'p1', 'current_square'], value: deltaValue },
    ] }],
  }) as unknown as GameRecord

  const frames = buildReplayFrames(record)
  assert.equal(frames.length, 2)
  assert.equal(frames[1].board.squares.e5, 'p1')
  assert.deepEqual(frames[1].pieces.p1.current_square, { file: 4, rank: 4 })
})

test('returns a safe failure result for malformed or forbidden replay data', () => {
  assert.deepEqual(buildReplayFramesResult({ actions: [] }), { ok: false, error: 'invalid_replay' })

  const malformed = {
    ruleset_version: LEGACY_RULES_VERSION,
    initial_state: { board: { size: 8, squares: {} }, pieces: {}, piece_definitions: {}, history: [] },
    initial_clock: {},
    players: { white: {}, black: {} },
    decks: {
      white: { deployments: [], pocket: [] },
      black: { deployments: [], pocket: [] },
    },
    actions: [{
      state_delta: [{ op: 'set', path: ['__proto__', 'polluted'], value: true }],
      clock: {}, action: {}, notation: { actor: {}, ability_events: [] },
    }],
  }
  assert.deepEqual(buildReplayFramesResult(malformed), { ok: false, error: 'invalid_replay' })
  assert.equal(({} as { polluted?: boolean }).polluted, undefined)
})

test('Standard delta replay preserves both hands and recorded Draw order without resampling', () => {
  const initial = { ruleset: 'standard', board: { size: 8, squares: {} }, pieces: { b4: { id: 'b4', type_id: 'knight', in_pocket: true }, w5: { id: 'w5', type_id: 'rook', in_pocket: true } }, players: {
    white: { deck: { hand_pieces: ['w1','w2','w3','w4'], pocket_pieces: ['w5'] } }, black: { deck: { hand_pieces: ['b1','b2','b3'], pocket_pieces: ['b4'] } },
  }, history: [] } as unknown as GameState
  const record = { ruleset_version: STANDARD_RULES_VERSION, initial_state: initial, initial_draws: [{ player_id: 'white', timing: 'turn_start', piece_ids: ['w4'] }], actions: [
    { draws: [{ player_id: 'black', timing: 'turn_start', piece_ids: ['b4'] }], state_delta: [
      { op: 'set', path: ['players','black','deck','hand_pieces'], value: ['b1','b2','b3','b4'] }, { op: 'set', path: ['players','black','deck','pocket_pieces'], value: [] }, { op: 'set', path: ['pieces','b4','in_pocket'], value: false },
    ] },
    { draws: [{ player_id: 'white', timing: 'turn_start', piece_ids: ['w5'] }], state_delta: [
      { op: 'set', path: ['players','white','deck','hand_pieces'], value: ['w1','w2','w3','w4','w5'] }, { op: 'set', path: ['players','white','deck','pocket_pieces'], value: [] }, { op: 'set', path: ['pieces','w5','in_pocket'], value: false },
    ] },
  ] } as unknown as GameRecord
  const frames = buildReplayFrames(record)
  assert.equal(frames[0].players.white.deck.hand_pieces!.length, 4)
  assert.equal(frames[0].players.black.deck.hand_pieces!.length, 3)
  assert.equal(frames[1].players.black.deck.hand_pieces!.length, 4)
  assert.deepEqual(frames[2].players.white.deck.hand_pieces, ['w1','w2','w3','w4','w5'])
  assert.equal(frames[1].pieces.b4.type_id, 'knight')
  assert.equal(frames[2].pieces.w5.type_id, 'rook')
  assert.deepEqual(buildReplayFrames(record), frames)
})

test('G7 versions are rejected before even attempting malformed delta replay', () => {
  for (const [ruleset, version, expected] of [
    ['standard', LEGACY_RULES_VERSION, 'unsupported_development_standard_record'],
    ['standard', 'future', 'unsupported_rules_version'],
    ['legacy', STANDARD_RULES_VERSION, 'unsupported_rules_version'],
  ] as const) {
    const record = { initial_state: { ruleset }, ruleset_version: version, actions: [{ state_delta: null }] } as unknown as GameRecord
    assert.throws(() => buildReplayFrames(record), new RegExp(expected))
  }
})
