import assert from 'node:assert/strict'
import test from 'node:test'

import { savedDeckToPlayerDeckRequest } from './useDeckSerialization.ts'
import {
  baseZoneDepth,
  applyPieceMetadata,
  canPieceBePlacedAtStart,
  frontmostBaseRank,
  customDeckPieceType,
  createPresetDeck,
  deactivateCustomPieceCatalog,
  pieceCatalog,
  pocketCatalog,
  replaceCustomPieceCatalog,
  placementRestriction,
  upsertCustomPieceCatalog,
  validateSavedDeck,
} from './useDeckValidation.ts'
import type { SavedDeck } from '../types/deck.ts'
import type { CustomPieceRecord } from '../types/customPiece.ts'

const frontPieceTypes = new Set(['pawn', 'tempest-pawn', 'bouncing-pawn', 'dozer'])
applyPieceMetadata(Object.fromEntries(pieceCatalog.map(piece => [piece.id, {
  score: piece.id === 'dozer' ? 3 : piece.id === 'knight' ? 1 : piece.score,
  deployment_zone: frontPieceTypes.has(piece.id) ? 'front' : 'back',
}])))

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
      { pieceType, square: { file: 2, rank: 0 } },
      ...Array.from({ length: 8 }, (_, file) => ({
        pieceType: 'pawn',
        square: { file, rank: 1 },
      })),
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

  assert.deepEqual(request.starting[1], { ...custom, square: { file: 2, rank: 0 } })
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

test('base zone expands to three ranks starting at board size 10', () => {
  assert.equal(baseZoneDepth(9), 2)
  assert.equal(baseZoneDepth(10), 3)

  const rankThreeDeck: SavedDeck = {
    ...deck('pawn'),
    boardSize: 10,
    starting: [
      { pieceType: 'king', square: { file: 4, rank: 1 } },
      ...Array.from({ length: 10 }, (_, file) => ({
        pieceType: 'pawn',
        square: { file, rank: 2 },
      })),
    ],
    pocket: {},
    customPieces: [],
  }

  assert.equal(validateSavedDeck(rankThreeDeck).valid, true)
  assert.equal(validateSavedDeck({ ...rankThreeDeck, boardSize: 9 }).valid, false)
})

test('presets place their pawn line on each board size frontmost setup rank', () => {
  for (const [boardSize, expectedRank] of [[8, 1], [9, 1], [10, 2], [11, 2], [12, 2]]) {
    const preset = createPresetDeck(boardSize)
    const pawns = preset.starting.filter(piece => piece.pieceType === 'pawn')
    assert.ok(pawns.length > 0)
    assert.ok(pawns.every(piece => piece.square.rank === expectedRank))
    assert.equal(validateSavedDeck({
      ...preset,
      id: `preset-${boardSize}`,
      name: 'Preset',
      boardSize,
      customPieces: [],
      createdAt: 1,
      updatedAt: 1,
    }).valid, true)
  }
})

test('deck validation requires every square in the front setup rank', () => {
  const complete = createPresetDeck(8)
  assert.equal(validateSavedDeck({
    ...complete,
    id: 'complete-front-rank',
    name: 'Complete',
    boardSize: 8,
    customPieces: [],
    createdAt: 1,
    updatedAt: 1,
  }).valid, true)

  const incomplete = {
    ...complete,
    starting: complete.starting.filter(piece => piece.square.file !== 7 || piece.square.rank !== 1),
  }
  const summary = validateSavedDeck({
    ...incomplete,
    id: 'incomplete-front-rank',
    name: 'Incomplete',
    boardSize: 8,
    customPieces: [],
    createdAt: 1,
    updatedAt: 1,
  })
  assert.equal(summary.valid, false)
  assert.match(summary.errors.join(' '), /앞줄.*7\/8/)
})

test('deployment zones replace score-based front-rank placement', () => {
  assert.equal(frontmostBaseRank(8, 'white'), 1)
  assert.equal(frontmostBaseRank(8, 'black'), 6)
  assert.equal(frontmostBaseRank(10, 'white'), 2)
  assert.equal(frontmostBaseRank(10, 'black'), 7)

  for (const pieceType of frontPieceTypes) {
    assert.equal(canPieceBePlacedAtStart(pieceType, 1, 8), true)
    assert.equal(canPieceBePlacedAtStart(pieceType, 0, 8), false)
  }
  for (const pieceType of ['knight', 'bishop', 'rook', 'queen', 'king', 'paratrooper']) {
    assert.equal(canPieceBePlacedAtStart(pieceType, 1, 8), false)
    assert.equal(canPieceBePlacedAtStart(pieceType, 0, 8), true)
  }

  assert.equal(pieceCatalog.find(piece => piece.id === 'dozer')?.score, 3)
  assert.equal(placementRestriction('dozer', 1, 8), null)
  assert.equal(pieceCatalog.find(piece => piece.id === 'knight')?.score, 1)
  assert.match(placementRestriction('knight', 1, 8) ?? '', /배치할 수 없습니다/)

  const validFront = deck('dozer')
  validFront.starting = validFront.starting.filter(
    piece => piece.square.rank !== 1 || piece.square.file !== 2,
  )
  validFront.starting[1].square.rank = 1
  assert.equal(validateSavedDeck(validFront).valid, true)

  const invalidBack = deck('knight')
  invalidBack.starting[1].square.rank = 1
  assert.match(validateSavedDeck(invalidBack).errors.join(' '), /배치할 수 없습니다/)
})
