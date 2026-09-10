import { LEGACY_RULES_VERSION, STANDARD_RULES_VERSION } from './gameRulesVersions.ts'
import assert from 'node:assert/strict'
import test from 'node:test'
import { decodeReplayCode, encodeReplayCode, MAX_REPLAY_CODE_LENGTH, MAX_REPLAY_JSON_BYTES } from './replayCodec.ts'
import type { GameRecord } from './types/gameRecord.ts'

const clock = {
  time_control: 'five_three' as const, mode: 'countdown' as const, initial_time_ms: 300_000, increment_ms: 3_000,
  active_color: 'white' as const, turn_started_at_ms: 1_000, server_now_ms: 1_000,
  white_remaining_ms: 300_000, black_remaining_ms: 300_000, white_elapsed_ms: 0, black_elapsed_ms: 0,
}
const state = {
  id: 'game-1', board: { size: 8, squares: {} }, pieces: {}, piece_definitions: {}, custom_piece_manifest: [],
  players: {}, current_player: 'white', turn_number: 1, phase: 'playing', history: [], result: null,
} as unknown as GameRecord['initial_state']
const record: GameRecord = {
  format_version: 2, game_id: 'game-1', display_name: 'white-black-2026-08-24-1732', ruleset_version: 'deck-chess-1', chessembly_version: 'chessembly-1',
  started_at_ms: 1_000, ended_at_ms: 2_000, result: { winner: 'white', reason: 'resignation' },
  players: { white: { public_id: 'white', nickname: 'White', side: 'white' }, black: { public_id: 'black', nickname: 'Black', side: 'black' } },
  time_control: 'five_three', initial_state: state, initial_clock: clock,
  decks: { white: { side: 'white', deck_name: 'White deck', deployments: [], pocket: [] }, black: { side: 'black', deck_name: 'Black deck', deployments: [], pocket: [] } },
  actions: [], final_clock: clock,
}

test('Replay Code gzip encode and decode round-trips the canonical GameRecord', async () => {
  const code = await encodeReplayCode(record)
  assert.match(code, /^DC-G2-/u)
  const decoded = await decodeReplayCode(code)
  assert.equal(decoded.ok, true)
  if (decoded.ok) assert.deepEqual(decoded.value, record)
})

test('Replay Code preserves a guest nickname without inventing a public id', async () => {
  const guestRecord = structuredClone(record)
  guestRecord.players.black = { public_id: null, nickname: 'Local Guest', side: 'black' }
  const decoded = await decodeReplayCode(await encodeReplayCode(guestRecord))
  assert.equal(decoded.ok, true)
  if (decoded.ok) assert.deepEqual(decoded.value.players.black, guestRecord.players.black)
})

test('Replay Code accepts the intentionally omitted empty custom piece manifest', async () => {
  const legacyRecord = structuredClone(record) as GameRecord
  delete legacyRecord.initial_state.custom_piece_manifest
  const decoded = await decodeReplayCode(await encodeReplayCode(legacyRecord))
  assert.equal(decoded.ok, true)
  if (decoded.ok) assert.equal(decoded.value.initial_state.custom_piece_manifest, undefined)
})

test('Replay Code rejects a present but malformed custom piece manifest', async () => {
  const malformed = structuredClone(record) as unknown as Record<string, unknown>
  ;(malformed.initial_state as Record<string, unknown>).custom_piece_manifest = { exposed_type_id: 'custom:bad:v1:bad' }
  assert.deepEqual(
    await decodeReplayCode(await encodeReplayCode(malformed as unknown as GameRecord)),
    { ok: false, error: 'invalid_schema' },
  )
})

test('Replay Code rejects prefix, version, truncation, malformed payload and excessive input', async () => {
  assert.deepEqual(await decodeReplayCode(''), { ok: false, error: 'empty' })
  assert.deepEqual(await decodeReplayCode('wrong'), { ok: false, error: 'invalid_format' })
  assert.deepEqual(await decodeReplayCode('DC-G1-AAAA'), { ok: false, error: 'unsupported_version' })
  const valid = await encodeReplayCode(record)
  assert.equal((await decodeReplayCode(valid.slice(0, -4))).ok, false)
  assert.deepEqual(await decodeReplayCode('DC-G2-%%%%'), { ok: false, error: 'invalid_payload' })
  assert.deepEqual(await decodeReplayCode('x'.repeat(MAX_REPLAY_CODE_LENGTH + 1)), { ok: false, error: 'too_large' })
})

test('Replay Code rejects payloads whose decompressed size exceeds the budget', async () => {
  const oversized = new TextEncoder().encode('x'.repeat(MAX_REPLAY_JSON_BYTES + 1))
  const compressed = new Uint8Array(await new Response(new Blob([oversized]).stream().pipeThrough(new CompressionStream('gzip'))).arrayBuffer())
  let binary = ''; for (const byte of compressed) binary += String.fromCharCode(byte)
  const payload = btoa(binary).replace(/\+/gu, '-').replace(/\//gu, '_').replace(/=+$/gu, '')
  assert.deepEqual(await decodeReplayCode(`DC-G2-${payload}`), { ok: false, error: 'invalid_payload' })
})

test('Replay Code rejects malformed action, square, deck and oversized delta arrays', async () => {
  const baseAction = {
    ply: 1, player_id: 'white', elapsed_ms: 10, clock,
    action: { type: 'ability', player_id: 'white', piece_id: 'p1', ability_id: 'fire', to: { file: 1, rank: 1 }, deployments: [] },
    notation: { turn_number: 1, move_number: 1, side: 'white', actor: { piece_id: 'p1', piece_type_id: 'tank', piece_name: 'tank', from: { file: 1, rank: 0 }, layer: 'ground', state: {} }, kind: 'ability', ability_id: 'fire', ability_name: '포격', from: { file: 1, rank: 0 }, to: { file: 1, rank: 1 }, target: { file: 1, rank: 1 }, ability_events: [{ ability_id: 'fire', ability_name: '포격', target: { file: 1, rank: 1 } }] },
    state_delta: [],
  }
  async function rejected(mutator: (value: Record<string, unknown>) => void) {
    const value = structuredClone({ ...record, actions: [baseAction] }) as unknown as Record<string, unknown>
    mutator(value)
    assert.deepEqual(await decodeReplayCode(await encodeReplayCode(value as unknown as GameRecord)), { ok: false, error: 'invalid_schema' })
  }
  await rejected(value => { ((value.actions as Array<Record<string, unknown>>)[0].action as Record<string, unknown>).type = 'teleport' })
  await rejected(value => { (((value.actions as Array<Record<string, unknown>>)[0].action as Record<string, unknown>).to as Record<string, unknown>).file = 99 })
  await rejected(value => { (value.decks as Record<string, Record<string, unknown>>).white.deployments = [{ piece_name: 'tank', square: { file: -1, rank: 0 } }] })
  await rejected(value => { (value.actions as Array<Record<string, unknown>>)[0].state_delta = Array.from({ length: 513 }, () => ({ op: 'set', path: ['turn_number'], value: 2 })) })
  await rejected(value => { (value.actions as Array<Record<string, unknown>>)[0].state_delta = [{ op: 'set', path: ['pieces', '__proto__', 'polluted'], value: true }] })
})

test('G1 Replay retains explicit rulesets, leaves legacy JSON unchanged and rejects unknown values', async () => {
  for (const ruleset of [undefined, 'legacy', 'standard'] as const) {
    const original = structuredClone(record)
    if (ruleset !== undefined) original.initial_state.ruleset = ruleset
    original.ruleset_version = ruleset === 'standard' ? STANDARD_RULES_VERSION : LEGACY_RULES_VERSION
    const decoded = await decodeReplayCode(await encodeReplayCode(original))
    assert.equal(decoded.ok, true)
    if (decoded.ok) assert.deepEqual(decoded.value, original)
  }
  for (const ruleset of ['future', null, 1]) {
    const malformed = { ...record, initial_state: { ...record.initial_state, ruleset } }
    assert.deepEqual(await decodeReplayCode(await encodeReplayCode(malformed as unknown as GameRecord)), { ok: false, error: 'invalid_schema' })
  }
})

test('Draw resolution metadata round-trips and malformed timing, players and instances are rejected', async () => {
  const standard = structuredClone(record)
  standard.initial_state.ruleset = 'standard'
  standard.ruleset_version = STANDARD_RULES_VERSION
  standard.initial_draws = [
    { player_id: 'white', timing: 'initial', piece_ids: ['w1','w2','w3'] },
    { player_id: 'black', timing: 'initial', piece_ids: ['b1','b2','b3'] },
    { player_id: 'white', timing: 'turn_start', piece_ids: ['w4'] },
  ]
  const decoded = await decodeReplayCode(await encodeReplayCode(standard))
  assert.equal(decoded.ok, true)
  if (decoded.ok) assert.deepEqual(decoded.value.initial_draws, standard.initial_draws)
  for (const draws of [null, {}, [{ player_id: 'unknown', timing: 'initial', piece_ids: [] }], [{ player_id: 'white', timing: 'future', piece_ids: [] }], [{ player_id: 'white', timing: 'initial', piece_ids: ['duplicate','duplicate'] }], [{ player_id: 'black', timing: 'turn_start', piece_ids: ['one','two'] }]]) {
    const bad = { ...standard, initial_draws: draws } as unknown as GameRecord
    assert.deepEqual(await decodeReplayCode(await encodeReplayCode(bad)), { ok: false, error: 'invalid_schema' })
  }
})

test('G5 canonical ExtraSummon preserves exact ordered sacrifices and rejects malformed IDs and squares', async () => {
  const value = structuredClone(record)
  value.initial_state.ruleset = 'standard'
  value.ruleset_version = STANDARD_RULES_VERSION
  value.actions = [{
    ply: 1, player_id: 'white', elapsed_ms: 0, clock_before_ms: null, clock_after_ms: null, clock, state_delta: [],
    action: { type: 'extra_summon', player_id: 'white', extra_piece_id: 'extra', sacrifice_piece_ids: ['q3','q1','q2'], target_square: { file: 4, rank: 0 } },
    notation: { turn_number: 1, move_number: 1, side: 'white', kind: 'extra_summon', actor: { piece_id: 'extra', piece_type_id: 'guhang', piece_name: '구행', layer: 'ground', state: {} }, to: { file: 4, rank: 0 }, ability_events: [] },
    draws: [{ player_id: 'black', timing: 'turn_start', piece_ids: ['drawn'] }],
  }]
  const decoded = await decodeReplayCode(await encodeReplayCode(value))
  assert.equal(decoded.ok, true)
  if (decoded.ok) assert.deepEqual(decoded.value.actions, value.actions)
  for (const patch of [
    { sacrifice_piece_ids: ['same','same'] }, { sacrifice_piece_ids: [''] }, { sacrifice_piece_ids: ['\u0000bad'] },
    { extra_piece_id: '' }, { extra_piece_id: null }, { target_square: { file: 1 } },
    { target_square: { file: 1.5, rank: 0 } }, { target_square: { file: -1, rank: 0 } },
  ]) {
    const bad = structuredClone(value)
    Object.assign(bad.actions[0].action, patch)
    assert.deepEqual(await decodeReplayCode(await encodeReplayCode(bad)), { ok: false, error: 'invalid_schema' })
  }
})

test('G7 replay code preserves semantic versions and explicitly rejects development, mismatched and future records', async () => {
  for (const [ruleset, version, expected] of [
    ['standard', LEGACY_RULES_VERSION, 'unsupported_development_standard_record'],
    ['standard', 'deck-chess-standard-2', 'unsupported_rules_version'],
    ['legacy', STANDARD_RULES_VERSION, 'unsupported_rules_version'],
    ['legacy', 'future', 'unsupported_rules_version'],
  ] as const) {
    const value = structuredClone(record)
    value.initial_state.ruleset = ruleset
    value.ruleset_version = version
    assert.deepEqual(await decodeReplayCode(await encodeReplayCode(value)), { ok: false, error: expected })
    assert.equal(value.ruleset_version, version)
  }
})
