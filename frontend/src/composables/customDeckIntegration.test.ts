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
  resolved_image_asset_key: 'knight',
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
  assert.equal(customItems[0].custom?.assetKey, 'knight')
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

test('catalog refresh shows only the newest version while retaining pinned metadata', () => {
  const updated = {
    ...record,
    name: 'Hero Plus',
    score: 9,
    version: 4,
    content_hash: 'def456',
    updated_at: 3,
  }

  replaceCustomPieceCatalog([updated, record])

  const visible = pieceCatalog.filter(piece => piece.custom?.id === record.id)
  assert.deepEqual(visible.map(piece => piece.custom!.version), [4])
  assert.equal(validateSavedDeck(deck(customDeckPieceType(record))).totalScore, 14)
})

test('saving a new version replaces the previous live catalog item but preserves pinned metadata', () => {
  replaceCustomPieceCatalog([record])
  const updated = {
    ...record,
    name: 'Hero Plus',
    score: 9,
    version: 4,
    content_hash: 'def456',
    resolved_image_asset_key: 'data:image/svg+xml;base64,PHN2Zy8+',
    updated_at: 3,
  }

  upsertCustomPieceCatalog(updated)

  const versions = pieceCatalog.filter(piece => piece.custom?.id === record.id)
  assert.deepEqual(versions.map(piece => piece.custom!.version), [4])
  assert.equal(versions[0].name, 'Hero Plus')
  assert.equal(versions[0].score, 9)
  assert.equal(versions[0].custom?.assetKey, updated.resolved_image_asset_key)
  assert.equal(
    pocketCatalog.some(piece => piece.id === customDeckPieceType(updated)),
    true,
  )
  assert.equal(
    pocketCatalog.some(piece => piece.id === customDeckPieceType(record)),
    false,
  )

  const pinnedSummary = validateSavedDeck(deck(customDeckPieceType(record)))
  assert.equal(pinnedSummary.valid, true)
  assert.equal(pinnedSummary.totalScore, 14)

  deactivateCustomPieceCatalog(record.id)
  assert.equal(versions[0].custom?.active, false)
  assert.match(validateSavedDeck(deck(customDeckPieceType(record))).errors.join(' '), /비활성화/)
})
