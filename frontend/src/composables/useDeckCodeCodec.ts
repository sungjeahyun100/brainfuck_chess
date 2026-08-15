import type { LobbyDeck } from '../types/deck'

export const DECK_CODE_PREFIX = 'DC1.'
export const MAX_DECK_CODE_LENGTH = 65_536

const MAX_STARTING_PIECES = 144
const MAX_POCKET_TYPES = 256
const MAX_POCKET_COUNT = 1_024
const MAX_TOTAL_POCKET_PIECES = 4_096

export interface DeckCodeV1 {
  v: 1
  boardSize: number
  starting: Array<{
    pieceId: string
    file: number
    rank: number
  }>
  pocket: Array<{
    pieceId: string
    count: number
  }>
}

export type DeckCodeDecodeError =
  | 'empty'
  | 'too_large'
  | 'invalid_format'
  | 'unsupported_version'
  | 'invalid_payload'
  | 'invalid_schema'

export type DeckCodeDecodeResult =
  | { ok: true; value: DeckCodeV1 }
  | { ok: false; error: DeckCodeDecodeError }

function isRecord(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null
}

function hasExactlyKeys(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort()
  const expected = [...keys].sort()
  return actual.length === expected.length && actual.every((key, index) => key === expected[index])
}

function isSafePieceId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 256
}

function parseV1(value: unknown): DeckCodeV1 | null {
  if (!isRecord(value) || !hasExactlyKeys(value, ['v', 'boardSize', 'starting', 'pocket'])) return null
  if (value.v !== 1 || !Number.isInteger(value.boardSize)) return null
  if (!Array.isArray(value.starting) || value.starting.length > MAX_STARTING_PIECES) return null
  if (!Array.isArray(value.pocket) || value.pocket.length > MAX_POCKET_TYPES) return null

  const starting: DeckCodeV1['starting'] = []
  for (const entry of value.starting) {
    if (!isRecord(entry) || !hasExactlyKeys(entry, ['pieceId', 'file', 'rank'])) return null
    if (!isSafePieceId(entry.pieceId) || !Number.isInteger(entry.file) || !Number.isInteger(entry.rank)) return null
    starting.push({ pieceId: entry.pieceId, file: entry.file as number, rank: entry.rank as number })
  }

  const pocket: DeckCodeV1['pocket'] = []
  const pocketTypes = new Set<string>()
  let totalPocketPieces = 0
  for (const entry of value.pocket) {
    if (!isRecord(entry) || !hasExactlyKeys(entry, ['pieceId', 'count'])) return null
    if (!isSafePieceId(entry.pieceId) || !Number.isInteger(entry.count)) return null
    const count = entry.count as number
    if (count <= 0 || count > MAX_POCKET_COUNT || pocketTypes.has(entry.pieceId)) return null
    totalPocketPieces += count
    if (totalPocketPieces > MAX_TOTAL_POCKET_PIECES) return null
    pocketTypes.add(entry.pieceId)
    pocket.push({ pieceId: entry.pieceId, count })
  }

  return {
    v: 1,
    boardSize: value.boardSize as number,
    starting,
    pocket,
  }
}

function encodeBase64Url(text: string): string {
  const bytes = new TextEncoder().encode(text)
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/u, '')
}

function decodeBase64Url(payload: string): string | null {
  if (!payload || !/^[A-Za-z0-9_-]+$/u.test(payload) || payload.length % 4 === 1) return null
  const padding = '='.repeat((4 - payload.length % 4) % 4)
  try {
    const binary = atob(payload.replace(/-/g, '+').replace(/_/g, '/') + padding)
    const bytes = Uint8Array.from(binary, character => character.charCodeAt(0))
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return null
  }
}

export function encodeDeckCode(deck: LobbyDeck & { boardSize: number }): string {
  const value: DeckCodeV1 = {
    v: 1,
    boardSize: deck.boardSize,
    starting: deck.starting
      .map(piece => ({
        pieceId: piece.pieceType,
        file: piece.square.file,
        rank: piece.square.rank,
      }))
      .sort((left, right) => left.rank - right.rank || left.file - right.file || left.pieceId.localeCompare(right.pieceId)),
    pocket: Object.entries(deck.pocket)
      .filter(([, count]) => count > 0)
      .map(([pieceId, count]) => ({ pieceId, count }))
      .sort((left, right) => left.pieceId.localeCompare(right.pieceId)),
  }
  return `${DECK_CODE_PREFIX}${encodeBase64Url(JSON.stringify(value))}`
}

export function decodeDeckCode(input: string): DeckCodeDecodeResult {
  if (!input.trim()) return { ok: false, error: 'empty' }
  if (input.length > MAX_DECK_CODE_LENGTH) return { ok: false, error: 'too_large' }
  const code = input.replace(/\s/gu, '')

  const match = /^DC(\d+)\.(.*)$/u.exec(code)
  if (!match) return { ok: false, error: 'invalid_format' }
  if (match[1] !== '1') return { ok: false, error: 'unsupported_version' }

  const json = decodeBase64Url(match[2])
  if (json === null) return { ok: false, error: 'invalid_payload' }
  try {
    const value = parseV1(JSON.parse(json) as unknown)
    return value ? { ok: true, value } : { ok: false, error: 'invalid_schema' }
  } catch {
    return { ok: false, error: 'invalid_payload' }
  }
}
