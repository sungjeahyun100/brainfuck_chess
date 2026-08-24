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
  format_version: 1, game_id: 'game-1', display_name: 'white-black-2026-08-24-1732', ruleset_version: 'deck-chess-1', chessembly_version: 'chessembly-1',
  started_at_ms: 1_000, ended_at_ms: 2_000, result: { winner: 'white', reason: 'resignation' },
  players: { white: { public_id: 'white', nickname: 'White', side: 'white' }, black: { public_id: 'black', nickname: 'Black', side: 'black' } },
  time_control: 'five_three', initial_state: state, initial_clock: clock, piece_id_map: {}, actions: [], final_state: state, final_clock: clock,
}

test('Replay Code gzip encode and decode round-trips the canonical GameRecord', async () => {
  const code = await encodeReplayCode(record)
  assert.match(code, /^DC-G1-/u)
  const decoded = await decodeReplayCode(code)
  assert.equal(decoded.ok, true)
  if (decoded.ok) assert.deepEqual(decoded.value, record)
})

test('Replay Code rejects prefix, version, truncation, malformed payload and excessive input', async () => {
  assert.deepEqual(await decodeReplayCode(''), { ok: false, error: 'empty' })
  assert.deepEqual(await decodeReplayCode('wrong'), { ok: false, error: 'invalid_format' })
  assert.deepEqual(await decodeReplayCode('DC-G2-AAAA'), { ok: false, error: 'unsupported_version' })
  const valid = await encodeReplayCode(record)
  assert.equal((await decodeReplayCode(valid.slice(0, -4))).ok, false)
  assert.deepEqual(await decodeReplayCode('DC-G1-%%%%'), { ok: false, error: 'invalid_payload' })
  assert.deepEqual(await decodeReplayCode('x'.repeat(MAX_REPLAY_CODE_LENGTH + 1)), { ok: false, error: 'too_large' })
})

test('Replay Code rejects payloads whose decompressed size exceeds the budget', async () => {
  const oversized = new TextEncoder().encode('x'.repeat(MAX_REPLAY_JSON_BYTES + 1))
  const compressed = new Uint8Array(await new Response(new Blob([oversized]).stream().pipeThrough(new CompressionStream('gzip'))).arrayBuffer())
  let binary = ''; for (const byte of compressed) binary += String.fromCharCode(byte)
  const payload = btoa(binary).replace(/\+/gu, '-').replace(/\//gu, '_').replace(/=+$/gu, '')
  assert.deepEqual(await decodeReplayCode(`DC-G1-${payload}`), { ok: false, error: 'invalid_payload' })
})
