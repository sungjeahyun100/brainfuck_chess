import assert from 'node:assert/strict'
import test from 'node:test'

import type { SavedDeck } from '../types/deck.ts'
import { importDeckCode } from './useDeckCode.ts'
import {
  decodeDeckCode,
  encodeDeckCode,
  MAX_DECK_CODE_LENGTH,
  MAX_DECK_CODE_EXTRA_PIECES,
} from './useDeckCodeCodec.ts'
import {
  applyPieceMetadata,
  createPresetDeck,
  pieceCatalog,
  validateDeckForStorage,
  validateSavedDeck,
} from './useDeckValidation.ts'

const scores: Record<string, number> = {
  king: 0,
  queen: 9,
  rook: 5,
  bishop: 3,
  knight: 3,
  pawn: 1,
  'prime-minister': 7,
  dozer: 3,
  'bouncing-pawn': 2,
  'tempest-pawn': 2,
}
const frontPieces = new Set(['pawn', 'dozer', 'bouncing-pawn', 'tempest-pawn'])
applyPieceMetadata(Object.fromEntries(pieceCatalog.map(piece => [piece.id, {
  score: scores[piece.id] ?? 1,
  deployment_zone: frontPieces.has(piece.id) ? 'front' : 'back',
}])))

test('국무총리 is available in the built-in deck catalog at score 7', () => {
  const primeMinister = pieceCatalog.find(piece => piece.id === 'prime-minister')

  assert.equal(primeMinister?.name, '국무총리')
  assert.equal(primeMinister?.score, 7)
  assert.equal(primeMinister?.canPocket, true)
})

function savedDeck(): SavedDeck {
  const preset = createPresetDeck(8)
  return {
    ...preset,
    id: 'deck-1',
    name: '공유 테스트',
    mapId: 'standard-8x8',
    boardSize: 8,
    createdAt: 10,
    updatedAt: 20,
    customPieces: [],
  }
}

function base64Url(value: string): string {
  const bytes = new TextEncoder().encode(value)
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/u, '')
}

function codeFor(value: unknown): string {
  return `DC1.${base64Url(JSON.stringify(value))}`
}

function validPayload(): Record<string, unknown> {
  return {
    v: 1,
    boardSize: 8,
    starting: [
      { pieceId: 'king', file: 4, rank: 0 },
      ...Array.from({ length: 8 }, (_, file) => ({ pieceId: 'pawn', file, rank: 1 })),
    ],
    pocket: [{ pieceId: 'knight', count: 2 }],
  }
}

test('DC3 export and import round-trip preserves name, map, board, placement, and pocket', () => {
  const original = savedDeck()
  original.starting = [
    { pieceType: 'king', square: { file: 4, rank: 0 } },
    ...Array.from({ length: 7 }, (_, file) => ({ pieceType: 'pawn', square: { file, rank: 1 } })),
    { pieceType: 'dozer', square: { file: 7, rank: 1 } },
  ]
  original.pocket = { knight: 2, bishop: 1 }

  const code = encodeDeckCode(original)
  assert.match(code, /^DC3\.[A-Za-z0-9_-]+$/u)
  const decoded = decodeDeckCode(code)
  assert.equal(decoded.ok, true)
  if (!decoded.ok) return
  assert.equal(JSON.stringify(decoded.value).includes('score'), false)
  assert.equal(decoded.value.name, original.name)

  const imported = importDeckCode(code, original)
  assert.equal(imported.ok, true)
  if (!imported.ok) return
  assert.equal(imported.deck.boardSize, original.boardSize)
  assert.equal(imported.deck.mapId, original.mapId)
  assert.deepEqual(
    [...imported.deck.starting].sort((a, b) => a.square.rank - b.square.rank || a.square.file - b.square.file),
    [...original.starting].sort((a, b) => a.square.rank - b.square.rank || a.square.file - b.square.file),
  )
  assert.deepEqual(
    Object.fromEntries(Object.entries(imported.deck.pocket).filter(([, count]) => count > 0)),
    Object.fromEntries(Object.entries(original.pocket).filter(([, count]) => count > 0)),
  )
  assert.equal(imported.deck.id, original.id)
  assert.equal(imported.deck.name, original.name)
})

test('DC3 round-trip freezes the canonical custom piece identity and content hash', () => {
  const deck = savedDeck()
  deck.customPieces = [{ id: 'airship', version: 7, contentHash: 'sha256_frozen', exposedPieceKey: 'captain' }]
  const decoded = decodeDeckCode(encodeDeckCode(deck))
  assert.equal(decoded.ok, true)
  if (!decoded.ok) return
  assert.deepEqual(decoded.value.customPieces, deck.customPieces)
})

test('decoder accepts harmless whitespace around or inside a copied code', () => {
  const code = encodeDeckCode(savedDeck())
  const wrapped = ` \n${code.slice(0, 20)}\n${code.slice(20)} \t`
  assert.equal(decodeDeckCode(wrapped).ok, true)
})

test('DC2 preserves a terrain map separately from board size', () => {
  const deck = savedDeck()
  deck.mapId = 'central-high-ground-12x12'
  deck.boardSize = 12
  const decoded = decodeDeckCode(encodeDeckCode(deck))
  assert.equal(decoded.ok, true)
  if (!decoded.ok) return
  assert.equal(decoded.value.mapId, 'central-high-ground-12x12')
  assert.equal(decoded.value.boardSize, 12)
})

test('decoder returns explicit failures for malformed envelopes and payloads', () => {
  assert.deepEqual(decodeDeckCode(''), { ok: false, error: 'empty' })
  assert.deepEqual(decodeDeckCode('not-a-code'), { ok: false, error: 'invalid_format' })
  assert.deepEqual(decodeDeckCode('XX1.aaaa'), { ok: false, error: 'invalid_format' })
  assert.deepEqual(decodeDeckCode('DC5.aaaa'), { ok: false, error: 'unsupported_version' })
  assert.deepEqual(decodeDeckCode('DC1.%%%'), { ok: false, error: 'invalid_payload' })
  assert.deepEqual(decodeDeckCode(`DC1.${base64Url('not json')}`), { ok: false, error: 'invalid_payload' })
  assert.deepEqual(decodeDeckCode(`DC1.${base64Url('{"v":1')}`), { ok: false, error: 'invalid_payload' })
  assert.deepEqual(decodeDeckCode('x'.repeat(MAX_DECK_CODE_LENGTH + 1)), { ok: false, error: 'too_large' })
})

test('decoder rejects malformed schema, unexpected keys, and unsafe pocket sizes', () => {
  assert.deepEqual(decodeDeckCode(codeFor({ v: 1 })), { ok: false, error: 'invalid_schema' })
  assert.deepEqual(
    decodeDeckCode(codeFor({ ...validPayload(), score: 1 })),
    { ok: false, error: 'invalid_schema' },
  )
  assert.deepEqual(
    decodeDeckCode(`DC1.${base64Url('{"v":1,"boardSize":8,"starting":[],"pocket":[],"__proto__":{"polluted":true}}')}`),
    { ok: false, error: 'invalid_schema' },
  )
  assert.deepEqual(
    decodeDeckCode(codeFor({ ...validPayload(), pocket: [{ pieceId: 'pawn', count: 1_025 }] })),
    { ok: false, error: 'invalid_schema' },
  )
  assert.deepEqual(
    decodeDeckCode(codeFor({ ...validPayload(), pocket: [{ pieceId: 'pawn', count: 1 }, { pieceId: 'pawn', count: 1 }] })),
    { ok: false, error: 'invalid_schema' },
  )
})

test('import rejects unknown pieces without modifying the current deck', () => {
  const current = savedDeck()
  const before = structuredClone(current)
  const payload = validPayload()
  payload.starting = [{ pieceId: 'king', file: 4, rank: 0 }, { pieceId: 'missing-piece', file: 0, rank: 0 }]
  const result = importDeckCode(codeFor(payload), current)
  assert.equal(result.ok, false)
  if (!result.ok) assert.match(result.message, /존재하지 않거나/)
  assert.deepEqual(current, before)
})

test('import reuses current validation for squares, placement zones, score, and pocket rules', () => {
  const current = savedDeck()
  const cases: Array<[string, Record<string, unknown>, RegExp]> = [
    ['duplicate square', {
      ...validPayload(),
      starting: [{ pieceId: 'king', file: 4, rank: 0 }, { pieceId: 'rook', file: 4, rank: 0 }],
    }, /같은 칸/],
    ['out of board', {
      ...validPayload(),
      starting: [{ pieceId: 'king', file: 8, rank: 0 }],
    }, /보드 범위/],
    ['wrong deployment zone', {
      ...validPayload(),
      starting: [{ pieceId: 'king', file: 4, rank: 0 }, { pieceId: 'pawn', file: 0, rank: 0 }],
    }, /가장 앞쪽/],
    ['score limit', {
      ...validPayload(),
      pocket: [{ pieceId: 'queen', count: 5 }],
    }, /덱 점수/],
    ['king in pocket', {
      ...validPayload(),
      pocket: [{ pieceId: 'king', count: 1 }],
    }, /King은 포켓/],
    ['unsupported board size', {
      ...validPayload(),
      boardSize: 99,
    }, /데이터 구조/],
  ]

  for (const [name, payload, expected] of cases) {
    const result = importDeckCode(codeFor(payload), current)
    assert.equal(result.ok, false, name)
    if (!result.ok) assert.match(result.message, expected, name)
  }
})


test('DC1/DC2/DC3 retain Legacy meaning even when imported into a Standard draft', () => {
  const current = { ...savedDeck(), ruleset: 'standard' as const }
  const before = JSON.parse(JSON.stringify(current))
  const v1 = validPayload()
  const codes = [
    codeFor(v1),
    `DC2.${base64Url(JSON.stringify({ ...v1, v: 2, mapId: 'standard-8x8' }))}`,
    encodeDeckCode({ ...savedDeck(), ruleset: 'legacy' }),
  ]
  for (const code of codes) {
    const result = importDeckCode(code, current)
    assert.equal(result.ok, true)
    if (result.ok) assert.equal(result.deck.ruleset, 'legacy')
  }
  assert.deepEqual(current, before)
  assert.equal(encodeDeckCode(savedDeck()), encodeDeckCode({ ...savedDeck(), ruleset: 'legacy' }))
  assert.match(encodeDeckCode(current), /^DC4\./u)
  assert.throws(() => encodeDeckCode({ ...current, ruleset: 'future' } as never), /지원하지 않는 덱 룰/)
  for (const v of [1, 2, 3]) {
    const payload = v === 1 ? v1 : v === 2 ? { ...v1, v, mapId: 'standard-8x8' } : { ...v1, v, mapId: 'standard-8x8', name: 'Deck', customPieces: [] }
    const result = decodeDeckCode(`DC${v}.${base64Url(JSON.stringify({ ...payload, ruleset: 'legacy' }))}`)
    assert.deepEqual(result, { ok: false, error: 'invalid_schema' })
  }
})

test('Extra cannot be exported by DC3 and explicit Legacy code import replaces the Extra draft', () => {
  const source = { ...createPresetDeck(8), boardSize: 8, mapId: 'standard-8x8' as const, name: 'Legacy' }
  const code = encodeDeckCode(source)
  assert.match(encodeDeckCode({ ...source, ruleset: 'standard', extra: ['guhang'] }), /^DC4\./u)
  assert.match(encodeDeckCode({ ...source, ruleset: 'legacy', extra: ['guhang'] }), /^DC4\./u)
  const draft = { ...source, id: 'draft', ruleset: 'standard', extra: ['guhang'], customPieces: [], createdAt: 1, updatedAt: 1 } as SavedDeck
  const imported = importDeckCode(code, draft)
  assert.equal(imported.ok, true)
  if (imported.ok) { assert.deepEqual(imported.deck.extra, []); assert.equal(imported.deck.ruleset, 'legacy') }
  assert.deepEqual(draft.extra, ['guhang'])
})

function v4Payload() {
  return { v: 4, name: 'Standard 초안', mapId: 'standard-8x8', boardSize: 8,
    ruleset: 'standard', starting: [{ pieceId: 'king', file: 4, rank: 0 }],
    pocket: [{ pieceId: 'knight', count: 2 }], extra: ['guhang', 'guhang', 'bomber'], customPieces: [] }
}
function dc4(value: unknown) { return `DC4.${base64Url(JSON.stringify(value))}` }

test('G7 DC4 preserves Standard 8–12 and terrain maps, duplicate Extra, name and canonical content', () => {
  for (const size of [8, 9, 10, 11, 12]) {
    for (const mapId of size === 12 ? ['standard-12x12', 'central-high-ground-12x12'] : [`standard-${size}x${size}`]) {
      const source = { ...savedDeck(), ...createPresetDeck(size, 'classic', 'standard'), ruleset: 'standard', boardSize: size, mapId,
        name: '왕복 이름 ♞', pocket: { rook: 1, knight: 2 }, extra: ['guhang', 'guhang', 'bomber'] } as SavedDeck
      const code = encodeDeckCode(source)
      assert.match(code, /^DC4\./u)
      assert.equal(code, encodeDeckCode({ ...source, starting: [...source.starting].reverse(), pocket: { knight: 2, rook: 1 }, extra: [...source.extra!].reverse() }))
      const result = importDeckCode(code, { ...savedDeck(), ruleset: 'legacy' })
      assert.ok(result.ok)
      assert.equal(result.deck.ruleset, source.ruleset)
      assert.equal(result.deck.mapId, mapId)
      assert.equal(result.deck.boardSize, size)
      assert.equal(result.deck.name, source.name)
      assert.deepEqual(result.deck.extra, ['bomber', 'guhang', 'guhang'])
      assert.deepEqual(result.deck.starting, [...source.starting].sort((a,b) => a.square.rank-b.square.rank || a.square.file-b.square.file))
      assert.equal(result.deck.pocket.rook, 1)
      assert.equal(result.deck.pocket.knight, 2)
      assert.ok(validateSavedDeck(result.deck).valid)
      assert.equal(encodeDeckCode(result.deck), code)
    }
  }
})

test('G7 DC4 strict bounds/schema reject malformed imports atomically', () => {
  const source = { ...savedDeck(), ruleset: 'standard' as const, extra: ['bomber'] }
  const before = structuredClone(source)
  const payload = v4Payload()
  const { ruleset: _ruleset, ...missing } = payload
  const malformed = [missing, { ...payload, ruleset: null }, { ...payload, ruleset: 'future' },
    { ...payload, v: 3 }, { ...payload, hand: [] }, { ...payload, mapId: 'standard-12x12' },
    { ...payload, extra: Array(MAX_DECK_CODE_EXTRA_PIECES + 1).fill('bomber') },
    { ...payload, extra: [{ pieceId: 'bomber', count: 1 }] }, { ...payload, extra: ['bomber\n'] },
    { ...payload, pocket: [{ pieceId: 'knight', count: -1 }] },
    { ...payload, pocket: [{ pieceId: 'knight', count: Number.MAX_SAFE_INTEGER + 1 }] },
    { ...payload, starting: [{ pieceId: 'king', file: -1, rank: 0 }] },
    { ...payload, starting: [...payload.starting, ...payload.starting] },
    { ...payload, extra: ['custom:lost:v1:hero'] },
    { ...payload, customPieces: [{ id: 'bad', version: 0, contentHash: 'hash', exposedPieceKey: 'hero' }] },
  ]
  for (const value of malformed) {
    assert.deepEqual(decodeDeckCode(dc4(value)), { ok: false, error: 'invalid_schema' })
    assert.equal(importDeckCode(dc4(value), source).ok, false)
    assert.deepEqual(source, before)
  }
  assert.deepEqual(decodeDeckCode('DC4.' + 'a'.repeat(MAX_DECK_CODE_LENGTH)), { ok: false, error: 'too_large' })
  for (const extra of [{ ruleset: 'legacy' }, { extra: [] }]) {
    const { ruleset: _r, extra: _e, ...base } = payload
    assert.deepEqual(decodeDeckCode(`DC3.${base64Url(JSON.stringify({ ...base, v: 3, ...extra }))}`), { ok: false, error: 'invalid_schema' })
  }
})

test('G7 DC4 draft import keeps the separate storage/game limit and Legacy Extra contract', () => {
  for (const ruleset of ['standard', 'legacy'] as const) {
    const payload = { ...v4Payload(), ruleset, extra: Array(4).fill('bomber') }
    const imported = importDeckCode(dc4(payload), savedDeck())
    assert.ok(imported.ok)
    assert.equal(imported.deck.ruleset, ruleset)
    assert.deepEqual(imported.deck.extra, payload.extra)
    assert.ok(validateDeckForStorage(imported.deck).valid)
    assert.equal(validateSavedDeck(imported.deck).valid, false)
    assert.match(encodeDeckCode(imported.deck), /^DC4\./u)
  }
  const max = { ...v4Payload(), extra: Array(MAX_DECK_CODE_EXTRA_PIECES).fill('bomber') }
  assert.ok(decodeDeckCode(dc4(max)).ok)
})

test('G7 custom references used only by Extra round-trip without catalog substitution', t => {
  const ref = { id: 'g7-package', version: 2, contentHash: 'sha256_fixed', exposedPieceKey: 'hero' }
  const id = `custom:${ref.id}:v${ref.version}:${ref.exposedPieceKey}`
  pieceCatalog.push({ id, name: 'Custom', score: 3, category: 'custom', canPocket: true, deploymentZone: 'back',
    custom: { ...ref, active: false, image: { kind: 'built_in', asset_key: 'knight' } } })
  t.after(() => pieceCatalog.splice(pieceCatalog.findIndex(piece => piece.id === id), 1))
  const source: SavedDeck = { ...savedDeck(), ruleset: 'standard', extra: [id, id], customPieces: [ref] }
  const result = importDeckCode(encodeDeckCode(source), savedDeck())
  assert.ok(result.ok)
  assert.deepEqual(result.deck.extra, source.extra)
  assert.deepEqual(result.deck.customPieces, [ref])
  assert.equal(validateSavedDeck(result.deck).valid, false)
  const bad = { ...v4Payload(), extra: [id], customPieces: [{ ...ref, contentHash: 'changed' }] }
  const before = structuredClone(source)
  assert.equal(importDeckCode(dc4(bad), source).ok, false)
  assert.deepEqual(source, before)
})

test('G7 Legacy DC3 bytes remain frozen, including ordering and empty Extra', () => {
  const source = { ruleset: 'legacy' as const, name: 'Old', boardSize: 8, mapId: 'standard-8x8' as const,
    starting: [{ pieceType: 'king', square: { file: 4, rank: 0 } }], pocket: { knight: 2 }, customPieces: [], extra: [] }
  // Captured pre-G7 DC3 wire JSON, deliberately independent of encoder output.
  const wire = '{"v":3,"name":"Old","mapId":"standard-8x8","boardSize":8,"starting":[{"pieceId":"king","file":4,"rank":0}],"pocket":[{"pieceId":"knight","count":2}],"customPieces":[]}'
  assert.equal(encodeDeckCode(source), `DC3.${base64Url(wire)}`)
  assert.deepEqual(decodeDeckCode(`DC3.${base64Url(wire.replace('"v":3', '"v":4'))}`), { ok: false, error: 'invalid_schema' })
})
