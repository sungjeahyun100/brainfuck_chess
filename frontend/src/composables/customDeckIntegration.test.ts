import assert from 'node:assert/strict'
import test from 'node:test'

import { savedDeckToPlayerDeckRequest } from './useDeckSerialization.ts'
import {
  customDeckPieceType,
  deactivateCustomPieceCatalog,
  pieceCatalog,
  pocketCatalog,
  replaceCustomPieceCatalog,
  upsertCustomPieceCatalog,
  validateSavedDeck,
} from './useDeckValidation.ts'
import type { SavedDeck } from '../types/deck.ts'
import type { CustomPieceRecord } from '../types/customPiece.ts'

const record: CustomPieceRecord = {
  id: 'package-one',
  owner_id: 'alice',
  name: 'Hero',
  description: 'custom hero',
  score: 7,
  image: { kind: 'built_in', asset_key: 'knight' },
  raw_script: '{}',
  exposed_piece_key: 'hero',
  internal_piece_keys: ['hidden-state'],
  validation_status: 'valid',
  version: 3,
  content_hash: 'abc123',
  created_at: 1,
  updated_at: 2,
  active: true,
}

function deck(pieceType: string): SavedDeck {
  return {
    id: 'deck',
    name: 'Custom deck',
    boardSize: 8,
    starting: [
      { pieceType: 'king', square: { file: 4, rank: 0 } },
      { pieceType, square: { file: 2, rank: 1 } },
    ],
    pocket: { [pieceType]: 1 },
    customPieces: [{
      id: record.id,
      version: record.version,
      contentHash: record.content_hash,
      exposedPieceKey: record.exposed_piece_key,
    }],
    createdAt: 1,
    updatedAt: 1,
  }
}

test('custom catalog exposes only the representative and contributes its pinned server score', () => {
  replaceCustomPieceCatalog([record])
  const pieceType = customDeckPieceType(record)
  const customItems = pieceCatalog.filter(piece => piece.custom)

  assert.deepEqual(customItems.map(piece => piece.id), [pieceType])
  assert.equal(customItems.some(piece => piece.id.includes('hidden-state')), false)
  assert.equal(validateSavedDeck(deck(pieceType)).totalScore, 14)
})

test('deck serialization sends immutable references and never sends source or score', () => {
  replaceCustomPieceCatalog([record])
  const request = savedDeckToPlayerDeckRequest(deck(customDeckPieceType(record)))
  const custom = {
    custom_piece_id: record.id,
    version: record.version,
    content_hash: record.content_hash,
    exposed_piece_key: record.exposed_piece_key,
  }

  assert.deepEqual(request.starting[1], { ...custom, square: { file: 2, rank: 1 } })
  assert.deepEqual(request.pocket, [custom])
  assert.equal(JSON.stringify(request).includes('raw_script'), false)
  assert.equal(JSON.stringify(request).includes('"score"'), false)
})

test('missing or inactive pinned versions are explicit deck validation failures', () => {
  replaceCustomPieceCatalog([])
  assert.equal(validateSavedDeck(deck(customDeckPieceType(record))).valid, false)

  replaceCustomPieceCatalog([{ ...record, active: false }])
  assert.match(validateSavedDeck(deck(customDeckPieceType(record))).errors.join(' '), /비활성화/)
})

test('saved custom piece changes are published to the live deck and pocket catalogs', () => {
  replaceCustomPieceCatalog([record])
  const updated = {
    ...record,
    name: 'Hero Plus',
    score: 9,
    version: 4,
    content_hash: 'def456',
    updated_at: 3,
  }

  upsertCustomPieceCatalog(updated)

  const versions = pieceCatalog
    .filter(piece => piece.custom?.id === record.id)
    .sort((left, right) => left.custom!.version - right.custom!.version)
  assert.deepEqual(versions.map(piece => piece.custom!.version), [3, 4])
  assert.equal(versions[1].name, 'Hero Plus')
  assert.equal(versions[1].score, 9)
  assert.equal(
    pocketCatalog.some(piece => piece.id === customDeckPieceType(updated)),
    true,
  )

  deactivateCustomPieceCatalog(record.id)
  assert.equal(versions.every(piece => piece.custom?.active === false), true)
})
