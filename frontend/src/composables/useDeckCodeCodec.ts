import { isDeckRuleset, parseDeckRuleset, type DeckRuleset } from '../deckRulesets.ts'
import type { CustomDeckPieceRef, LobbyDeck } from '../types/deck'
import type { BoardMapId } from '../types/game'
import { findBoardMap, standardMapId } from '../boardMaps.ts'

export const STANDARD_DECK_CODE_PREFIX = 'DC4.'
// Same structural draft bound as SavedDeck; game validation separately enforces 3.
export const MAX_DECK_CODE_EXTRA_PIECES = 4_096

export const DECK_CODE_PREFIX = 'DC3.'
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

// v:1 is the existing normalized geometry shape, not the envelope version.
export interface DecodedDeckCode extends DeckCodeV1 {
  mapId: BoardMapId
  ruleset?: DeckRuleset
  extra?: string[]
  name?: string
  customPieces?: CustomDeckPieceRef[]
}

export type DeckCodeDecodeError =
  | 'empty'
  | 'too_large'
  | 'invalid_format'
  | 'unsupported_version'
  | 'invalid_payload'
  | 'invalid_schema'

export type DeckCodeDecodeResult =
  | { ok: true; value: DecodedDeckCode }
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

export function encodeDeckCode(deck: LobbyDeck & { boardSize: number; mapId: BoardMapId; name?: string }): string {
  const ruleset = parseDeckRuleset(deck.ruleset)
  const useV4 = ruleset === 'standard' || (deck.extra?.length ?? 0) > 0
  const value = {
    v: 3,
    name: 'name' in deck && typeof deck.name === 'string' ? deck.name : '',
    mapId: deck.mapId,
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
    customPieces: [...(deck.customPieces ?? [])]
      .map(piece => ({ id: piece.id, version: piece.version, contentHash: piece.contentHash, exposedPieceKey: piece.exposedPieceKey }))
      .sort((left, right) => left.id.localeCompare(right.id) || left.version - right.version || left.exposedPieceKey.localeCompare(right.exposedPieceKey)),
  }
  if (!useV4) return `${DECK_CODE_PREFIX}${encodeBase64Url(JSON.stringify(value))}`
  const extra = [...(deck.extra ?? [])].sort((a, b) => a.localeCompare(b))
  const v4 = { ...value, v: 4, ruleset, extra }
  if (!parseV4(v4)) throw new Error('덱 코드의 데이터 구조가 올바르지 않습니다.')
  const code = `${STANDARD_DECK_CODE_PREFIX}${encodeBase64Url(JSON.stringify(v4))}`
  if (code.length > MAX_DECK_CODE_LENGTH) throw new Error('덱 코드가 허용된 최대 길이를 초과했습니다.')
  return code
}

/** DC4 adds only explicit ruleset and instance-based Extra to the frozen DC3 fields. */
function parseV4(parsed: unknown): DecodedDeckCode | null {
  if (!isRecord(parsed) || !hasExactlyKeys(parsed, ['v', 'name', 'mapId', 'boardSize', 'starting', 'pocket', 'customPieces', 'ruleset', 'extra'])
    || parsed.v !== 4 || !isDeckRuleset(parsed.ruleset)
    || !Array.isArray(parsed.extra) || parsed.extra.length > MAX_DECK_CODE_EXTRA_PIECES) return null
  const safeId = (id: unknown): id is string => isSafePieceId(id) && !/[\p{Cc}]/u.test(id)
  if (!parsed.extra.every(safeId)) return null
  const { ruleset, extra, ...base } = parsed
  const decoded = parseLegacyPayload({ ...base, v: 3 }, '3')
  if (!decoded.ok) return null
  const value = decoded.value
  const occupied = new Set<string>()
  for (const entry of value.starting) {
    const square = `${entry.file}:${entry.rank}`
    if (!safeId(entry.pieceId) || entry.file < 0 || entry.rank < 0
      || entry.file >= value.boardSize || entry.rank >= value.boardSize || occupied.has(square)) return null
    occupied.add(square)
  }
  if (!value.pocket.every(entry => safeId(entry.pieceId))) return null
  const refs = new Map<string, CustomDeckPieceRef>()
  for (const ref of value.customPieces ?? []) {
    if (!safeId(ref.id) || !safeId(ref.exposedPieceKey) || !Number.isSafeInteger(ref.version)) return null
    refs.set(`custom:${ref.id}:v${ref.version}:${ref.exposedPieceKey}`, ref)
  }
  for (const id of [...value.starting.map(entry => entry.pieceId), ...value.pocket.map(entry => entry.pieceId), ...extra]) {
    if (id.startsWith('custom:') && !refs.has(id)) return null
  }
  return { ...value, ruleset, extra: [...extra] }
}

export function decodeDeckCode(input: string): DeckCodeDecodeResult {
  if (!input.trim()) return { ok: false, error: 'empty' }
  if (input.length > MAX_DECK_CODE_LENGTH) return { ok: false, error: 'too_large' }
  const code = input.replace(/\s/gu, '')

  const match = /^DC(\d+)\.(.*)$/u.exec(code)
  if (!match) return { ok: false, error: 'invalid_format' }
  if (match[1] !== '1' && match[1] !== '2' && match[1] !== '3' && match[1] !== '4') return { ok: false, error: 'unsupported_version' }

  const json = decodeBase64Url(match[2])
  if (json === null) return { ok: false, error: 'invalid_payload' }
  try {
    const parsed = JSON.parse(json) as unknown
    if (match[1] === '4') {
      const value = parseV4(parsed)
      return value ? { ok: true, value } : { ok: false, error: 'invalid_schema' }
    }
    return parseLegacyPayload(parsed, match[1])
  } catch {
    return { ok: false, error: 'invalid_payload' }
  }
}

// Frozen DC1/DC2/DC3 exact-key readers; DC4 reuses the DC3 field contract.
function parseLegacyPayload(parsed: unknown, version: string): DeckCodeDecodeResult {
  if (version === '1') {
    const value = parseV1(parsed)
    const mapId = value ? standardMapId(value.boardSize) : null
    return value && mapId ? { ok: true, value: { ...value, mapId } } : { ok: false, error: 'invalid_schema' }
  }
  const expectedKeys = version === '3'
    ? ['v', 'name', 'mapId', 'boardSize', 'starting', 'pocket', 'customPieces']
    : ['v', 'mapId', 'boardSize', 'starting', 'pocket']
  if (!isRecord(parsed) || !hasExactlyKeys(parsed, expectedKeys)) {
    return { ok: false, error: 'invalid_schema' }
  }
  const { mapId: _mapId, name: _name, customPieces: _customPieces, ...withoutMapId } = parsed
  const legacyShape = { ...withoutMapId, v: 1 }
  const value = parseV1(legacyShape)
  const map = typeof parsed.mapId === 'string' ? findBoardMap(parsed.mapId) : null
  let customPieces: CustomDeckPieceRef[] | undefined
  if (version === '3') {
    if (parsed.v !== 3 || typeof parsed.name !== 'string' || Array.from(parsed.name).length > 100 || !Array.isArray(parsed.customPieces) || parsed.customPieces.length > 256) return { ok: false, error: 'invalid_schema' }
    customPieces = []
    const identities = new Set<string>()
    for (const item of parsed.customPieces) {
      if (!isRecord(item) || !hasExactlyKeys(item, ['id', 'version', 'contentHash', 'exposedPieceKey'])
        || !isSafePieceId(item.id) || !Number.isInteger(item.version) || (item.version as number) <= 0
        || typeof item.contentHash !== 'string' || !/^[A-Za-z0-9_:-]{1,256}$/u.test(item.contentHash)
        || !isSafePieceId(item.exposedPieceKey)) return { ok: false, error: 'invalid_schema' }
      const key = `${item.id}:${item.version}:${item.exposedPieceKey}`
      if (identities.has(key)) return { ok: false, error: 'invalid_schema' }
      identities.add(key)
      customPieces.push({ id: item.id, version: item.version as number, contentHash: item.contentHash, exposedPieceKey: item.exposedPieceKey })
    }
  }
  return value && map && map.boardSize === value.boardSize
    ? { ok: true, value: { ...value, mapId: map.id, ...(version === '3' ? { name: parsed.name as string, customPieces } : {}) } }
    : { ok: false, error: 'invalid_schema' }
}
