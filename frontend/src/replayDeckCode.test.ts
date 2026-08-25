import assert from 'node:assert/strict'
import test from 'node:test'
import { decodeDeckCode, encodeDeckCode } from './composables/useDeckCodeCodec.ts'
import { frozenDeckCodeSource } from './replayDeckCode.ts'
import type { GameRecord } from './types/gameRecord.ts'

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
