import assert from 'node:assert/strict'
import test from 'node:test'

import type { SavedDeck } from '../types/deck.ts'
import { importDeckCode } from './useDeckCode.ts'
import {
  decodeDeckCode,
  encodeDeckCode,
  MAX_DECK_CODE_LENGTH,
} from './useDeckCodeCodec.ts'
import {
  applyPieceMetadata,
  createPresetDeck,
  pieceCatalog,
} from './useDeckValidation.ts'

const scores: Record<string, number> = {
  king: 0,
  queen: 9,
  rook: 5,
  bishop: 3,
  knight: 3,
  pawn: 1,
  dozer: 3,
  'bouncing-pawn': 2,
  'tempest-pawn': 2,
}
const frontPieces = new Set(['pawn', 'dozer', 'bouncing-pawn', 'tempest-pawn'])
applyPieceMetadata(Object.fromEntries(pieceCatalog.map(piece => [piece.id, {
  score: scores[piece.id] ?? 1,
  deployment_zone: frontPieces.has(piece.id) ? 'front' : 'back',
}])))

function savedDeck(): SavedDeck {
  const preset = createPresetDeck(8)
  return {
    ...preset,
    id: 'deck-1',
    name: '공유 테스트',
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
      { pieceId: 'pawn', file: 0, rank: 1 },
    ],
    pocket: [{ pieceId: 'knight', count: 2 }],
  }
}

test('DC1 export and import round-trip preserves board, placement, and pocket only', () => {
  const original = savedDeck()
  original.starting = [
    { pieceType: 'king', square: { file: 4, rank: 0 } },
    { pieceType: 'pawn', square: { file: 0, rank: 1 } },
    { pieceType: 'dozer', square: { file: 7, rank: 1 } },
  ]
  original.pocket = { knight: 2, bishop: 1 }

  const code = encodeDeckCode(original)
  assert.match(code, /^DC1\.[A-Za-z0-9_-]+$/u)
  const decoded = decodeDeckCode(code)
  assert.equal(decoded.ok, true)
  if (!decoded.ok) return
  assert.equal(JSON.stringify(decoded.value).includes('score'), false)
  assert.equal(JSON.stringify(decoded.value).includes(original.name), false)

  const imported = importDeckCode(code, original)
  assert.equal(imported.ok, true)
  if (!imported.ok) return
  assert.equal(imported.deck.boardSize, original.boardSize)
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

test('decoder accepts harmless whitespace around or inside a copied code', () => {
  const code = encodeDeckCode(savedDeck())
  const wrapped = ` \n${code.slice(0, 20)}\n${code.slice(20)} \t`
  assert.equal(decodeDeckCode(wrapped).ok, true)
})

test('decoder returns explicit failures for malformed envelopes and payloads', () => {
  assert.deepEqual(decodeDeckCode(''), { ok: false, error: 'empty' })
  assert.deepEqual(decodeDeckCode('not-a-code'), { ok: false, error: 'invalid_format' })
  assert.deepEqual(decodeDeckCode('XX1.aaaa'), { ok: false, error: 'invalid_format' })
  assert.deepEqual(decodeDeckCode('DC2.aaaa'), { ok: false, error: 'unsupported_version' })
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
    }, /기본 진영/],
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
    }, /지원하지 않는 보드/],
  ]

  for (const [name, payload, expected] of cases) {
    const result = importDeckCode(codeFor(payload), current)
    assert.equal(result.ok, false, name)
    if (!result.ok) assert.match(result.message, expected, name)
  }
})
