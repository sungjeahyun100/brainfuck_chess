import assert from 'node:assert/strict'
import test from 'node:test'
import { decodeDeckCode, encodeDeckCode } from './composables/useDeckCodeCodec.ts'
import { frozenDeckCodeSource } from './replayDeckCode.ts'
import { buildReplayFramesResult } from './replayState.ts'
import type { GameRecord } from './types/gameRecord.ts'

function replayRecord(pieceTypeId = 'pawn-white', customPiece?: Record<string, unknown> | null): GameRecord {
  const deck = {
    side: 'white', deck_name: 'Legacy', map_id: 'standard-8x8', board_size: 8,
    deployments: [{ piece_type_id: pieceTypeId, piece_name: 'Piece', square: { file: 0, rank: 0 }, ...(customPiece === undefined ? {} : { custom_piece: customPiece }) }],
    pocket: [{ piece_type_id: 'rook', piece_name: 'Rook', count: 1 }],
  }
  return {
    format_version: 2, game_id: 'legacy', display_name: 'Legacy', ruleset_version: 'deck-chess-1', chessembly_version: 'chessembly-1',
    started_at_ms: 1, players: { white: { public_id: null, nickname: 'White', side: 'white' }, black: { public_id: null, nickname: 'Black', side: 'black' } },
    time_control: 'unlimited', initial_state: { board: { size: 8, squares: {} }, pieces: {}, piece_definitions: {}, players: {}, current_player: 'white', turn_number: 1, phase: 'playing', history: [], clock: {} as never, player_info: {} as never },
    initial_clock: {} as never, decks: { white: deck, black: { ...deck, side: 'black', deployments: [] } }, actions: [],
  } as unknown as GameRecord
}

test('built-in-only replay without a custom manifest still replays and copies its deck', () => {
  const record = replayRecord()
  const replay = buildReplayFramesResult(record)
  assert.equal(replay.ok, true)
  if (replay.ok) assert.deepEqual(replay.frames[0].custom_piece_manifest, [])
  assert.doesNotThrow(() => frozenDeckCodeSource(record, 'white'))
  assert.ok(frozenDeckCodeSource(record, 'white'))
})

test('explicit custom snapshot restores Deck Code identity without a manifest', () => {
  const record = replayRecord('historical-airship-captain', {
    custom_piece_id: 'airship', version: 7, content_hash: 'sha256_frozen', exposed_piece_key: 'captain',
  })
  const source = frozenDeckCodeSource(record, 'white')
  assert.ok(source)
  assert.equal(source.starting[0].pieceType, 'custom:airship:v7:captain')
  assert.deepEqual(source.customPieces, [{ id: 'airship', version: 7, contentHash: 'sha256_frozen', exposedPieceKey: 'captain' }])
})

test('legacy encoded custom ID without reconstructable metadata only disables Deck Code copy', () => {
  const record = replayRecord('custom:airship:v7:captain', null)
  assert.doesNotThrow(() => frozenDeckCodeSource(record, 'white'))
  assert.equal(frozenDeckCodeSource(record, 'white'), null)
  assert.equal(buildReplayFramesResult(record).ok, true)
})

test('malformed custom snapshot metadata does not invalidate replay frames', () => {
  const record = replayRecord('custom:airship:v7:captain', {
    custom_piece_id: 'airship', version: 7, exposed_piece_key: 'captain',
  })
  assert.equal(frozenDeckCodeSource(record, 'white'), null)
  assert.equal(buildReplayFramesResult(record).ok, true)
})

test('frozen black replay deck copies through the shared codec with custom identity intact', () => {
  const pieceId = 'custom:airship:v7:captain'
  const record = { decks: { black: { side: 'black', snapshot_version: 1, deck_name: 'Frozen', map_id: 'standard-8x8', board_size: 8,
    deployments: [{ piece_type_id: pieceId, piece_name: 'Old Captain', square: { file: 2, rank: 7 }, custom_piece: { custom_piece_id: 'airship', version: 7, content_hash: 'sha256_frozen', exposed_piece_key: 'captain' } }],
    pocket: [{ piece_type_id: 'pawn', piece_name: 'Pawn', count: 2 }] } }, initial_state: { custom_piece_manifest: [] } } as unknown as GameRecord
  const source = frozenDeckCodeSource(record, 'black')
  assert.ok(source)
  assert.equal(source.starting[0].square.rank, 0)
  const decoded = decodeDeckCode(encodeDeckCode(source))
  assert.equal(decoded.ok, true)
  if (!decoded.ok) return
  assert.equal(decoded.value.name, 'Frozen')
  assert.deepEqual(decoded.value.customPieces, source.customPieces)
  assert.deepEqual(decoded.value.starting, [{ pieceId, file: 2, rank: 0 }])
  assert.deepEqual(decoded.value.pocket, [{ pieceId: 'pawn', count: 2 }])
})

test('directional built-in engine ids become canonical neutral Deck Code ids', () => {
  const cases = [
    ['pawn-white', 'pawn'], ['pawn-black', 'pawn'], ['tempest-pawn-black', 'tempest-pawn'],
    ['bouncing-pawn-white', 'bouncing-pawn'], ['dozer-black', 'dozer'],
    ['surface-to-air-missile-white', 'surface-to-air-missile'],
    ['custom:airship:v7:captain', 'custom:airship:v7:captain'],
  ] as const
  for (const [engineId, canonicalId] of cases) {
    const record = { decks: { white: { side: 'white', deck_name: 'Frozen', map_id: 'standard-8x8', board_size: 8,
      deployments: [{ piece_type_id: engineId, piece_name: 'Historical name', square: { file: 0, rank: 0 } }], pocket: [] } },
      initial_state: { custom_piece_manifest: engineId.startsWith('custom:')
        ? [{ exposed_type_id: engineId, content_hash: 'sha256_frozen' }]
        : [] } } as unknown as GameRecord
    const source = frozenDeckCodeSource(record, 'white')
    assert.ok(source)
    const decoded = decodeDeckCode(encodeDeckCode(source))
    assert.equal(decoded.ok, true)
    if (decoded.ok) assert.equal(decoded.value.starting[0].pieceId, canonicalId)
  }
})
