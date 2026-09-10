import assert from 'node:assert/strict'
import test from 'node:test'

import { savedDeckToPlayerDeckRequest, serializeNeutralDeck } from './useDeckSerialization.ts'
import {
  baseZoneDepth,
  baseZoneSquares,
  frontZoneSquares,
  backZoneSquares,
  deploymentZoneAtSquare,
  applyPieceMetadata,
  canPieceBePlacedAtStart,
  frontmostBaseRank,
  neutralPieceCatalogId,
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

test('direction-specific engine IDs resolve to their neutral inspector catalog entries', () => {
  assert.equal(neutralPieceCatalogId('surface-to-air-missile-white'), 'surface-to-air-missile')
  assert.equal(neutralPieceCatalogId('surface-to-air-missile-black'), 'surface-to-air-missile')
  assert.equal(neutralPieceCatalogId('pawn-white'), 'pawn')
  assert.equal(neutralPieceCatalogId('knight'), 'knight')
})

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

const standardExpected: Record<number, { back: number[][]; front: number[][] }> = {
  8: { back: [[3,0],[4,0]], front: [[2,0],[5,0],[2,1],[3,1],[4,1],[5,1]] },
  9: { back: [[3,0],[4,0],[5,0]], front: [[2,0],[6,0],[2,1],[3,1],[4,1],[5,1],[6,1]] },
  10: { back: [[4,0],[5,0]], front: [[3,0],[6,0],[3,1],[4,1],[5,1],[6,1]] },
  11: { back: [[4,0],[5,0],[6,0]], front: [[3,0],[7,0],[3,1],[4,1],[5,1],[6,1],[7,1]] },
  12: { back: [[5,0],[6,0]], front: [[4,0],[7,0],[4,1],[5,1],[6,1],[7,1]] },
}

test('Standard exact Front/Back/Base sets and initial placement match every size and side', () => {
  for (const size of [8, 9, 10, 11, 12]) {
    const expected = standardExpected[size]
    for (const side of ['white', 'black'] as const) {
      const mirror = ([file, rank]: number[]) => ({ file, rank: side === 'white' ? rank : size - 1 - rank })
      const back = expected.back.map(mirror)
      const front = expected.front.map(mirror)
      assert.deepEqual(new Set(backZoneSquares(size, side, 'standard')), new Set(back))
      assert.deepEqual(new Set(frontZoneSquares(size, side, 'standard')), new Set(front))
      assert.deepEqual(new Set(baseZoneSquares(size, side, 'standard')), new Set([...back, ...front]))
      for (let rank = -1; rank <= size; rank++) {
        for (let file = -1; file <= size; file++) {
          const square = { file, rank }
          const includes = (squares: {file: number; rank: number}[]) => squares.some(s => s.file === file && s.rank === rank)
          assert.equal(deploymentZoneAtSquare(square, size, side, 'standard'), includes(back) ? 'back' : includes(front) ? 'front' : null)
          for (const kind of ['king', 'knight', 'guhang', 'bomber']) {
            assert.equal(canPieceBePlacedAtStart(kind, square, size, side, 'standard'), !['guhang', 'bomber'].includes(kind) && includes(back))
          }
          assert.equal(canPieceBePlacedAtStart('pawn', square, size, side, 'standard'), includes(front))
        }
      }
      assert.equal(canPieceBePlacedAtStart('pawn', side === 'white' ? 1 : size - 2, size, side, 'standard'), false)
    }
  }
})

test('Standard preset is playable with every Front square and optional Back vacancies; drafts remain storable', () => {
  for (const size of [8, 9, 10, 11, 12]) {
    const standard: SavedDeck = {
      ...deck('knight'), ...createPresetDeck(size, 'classic', 'standard'),
      boardSize: size, mapId: `standard-${size}x${size}`, customPieces: [], ruleset: 'standard',
    }
    assert.equal(validateSavedDeck(standard).valid, true)
    assert.equal(standard.starting.filter(p => p.pieceType === 'king').length, 1)
    assert.equal(standard.starting.length, standardExpected[size].front.length + 1)
    assert.equal(Object.values(standard.pocket).reduce((a,b) => a+b, 0), 0)
    for (const square of frontZoneSquares(size, 'white', 'standard')) {
      const missing = { ...standard, starting: standard.starting.filter(p => p.square.file !== square.file || p.square.rank !== square.rank) }
      assert.equal(validateSavedDeck(missing).valid, false)
      assert.ok(validateSavedDeck(missing).errors.some(e => e.includes('모든 칸')))
      const wrong = { ...standard, starting: standard.starting.map(p => p.square.file === square.file && p.square.rank === square.rank ? { ...p, pieceType: 'knight' } : p) }
      assert.ok(validateSavedDeck(wrong).errors.some(e => e.includes('모든 칸')))
    }
    const request = serializeNeutralDeck(standard, 'black')
    assert.equal(request.ruleset, 'standard')
    assert.deepEqual(request.starting.map(p => p.square), standard.starting.map(p => ({ file: p.square.file, rank: size - 1 - p.square.rank })))
    if (size === 12) assert.equal(validateSavedDeck({ ...standard, mapId: 'central-high-ground-12x12' }).valid, true)
  }
})

test('Standard Extra validates instance count, eligibility and independent original scores', () => {
  applyPieceMetadata(Object.fromEntries(pieceCatalog.filter(p => !p.custom).map(p => [p.id, {
    score: p.id === 'guhang' ? 25 : p.id === 'bomber' ? 13 : p.id === 'pawn' ? 1 : 0,
    deployment_zone: p.id === 'pawn' ? 'front' : 'back',
  }])))
  const base: SavedDeck = { ...deck('knight'), ...createPresetDeck(8, 'classic', 'standard'), ruleset: 'standard', boardSize: 8, mapId: 'standard-8x8' }
  // Main 39/39 remains 39/39 even with 51 Extra points.
  base.pocket = { pawn: 33 }
  for (const extra of [[], ['guhang'], ['guhang', 'bomber', 'bomber']]) {
    const result = validateSavedDeck({ ...base, extra })
    assert.equal(result.valid, true, result.errors.join(' '))
    assert.equal(result.totalScore, 39)
  }
  assert.equal(pieceCatalog.find(p => p.id === 'guhang')!.score, 25)
  assert.equal(pieceCatalog.find(p => p.id === 'bomber')!.score, 13)
  assert.match(validateSavedDeck({ ...base, extra: ['bomber', 'bomber', 'bomber', 'bomber'] }).errors.join(' '), /최대 3기/)
  assert.equal(validateSavedDeck({ ...base, extra: ['knight'] }).valid, false)
  for (const kind of ['guhang', 'bomber']) {
    assert.equal(validateSavedDeck({ ...base, pocket: { [kind]: 1 } }).valid, false)
    assert.equal(validateSavedDeck({ ...base, pocket: {}, starting: [...base.starting, { pieceType: kind, square: { file: 3, rank: 0 } }] }).valid, false)
  }
})

test('Extra serialization collects pinned custom references without granting custom eligibility', () => {
  replaceCustomPieceCatalog([record])
  const type = customDeckPieceType(record)
  const source = { ...deck(type), ruleset: 'standard' as const, extra: [type, type] }
  const request = savedDeckToPlayerDeckRequest(source)
  assert.deepEqual(request.extra, [request.pocket[0], request.pocket[0]])
  assert.match(validateSavedDeck(source).errors.join(' '), /Extra Deck에 넣을 수 없습니다/)
})
