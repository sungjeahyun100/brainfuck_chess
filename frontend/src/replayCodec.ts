import type { GameRecord } from './types/gameRecord'

export const REPLAY_CODE_PREFIX = 'DC-G2-'
export const MAX_REPLAY_CODE_LENGTH = 4_000_000
export const MAX_REPLAY_JSON_BYTES = 32_000_000
export const MAX_REPLAY_ACTIONS = 4_096
export const MAX_REPLAY_PIECES = 2_048
export const MAX_REPLAY_DELTA_OPERATIONS = 512

export type ReplayDecodeError = 'empty' | 'too_large' | 'invalid_format' | 'unsupported_version' | 'invalid_payload' | 'invalid_schema'
export type ReplayDecodeResult = { ok: true; value: GameRecord } | { ok: false; error: ReplayDecodeError }

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function base64Url(bytes: Uint8Array): string {
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += 32_768) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 32_768))
  }
  return btoa(binary).replace(/\+/gu, '-').replace(/\//gu, '_').replace(/=+$/gu, '')
}

function fromBase64Url(value: string): Uint8Array | null {
  if (!value || !/^[A-Za-z0-9_-]+$/u.test(value) || value.length % 4 === 1) return null
  try {
    const binary = atob(value.replace(/-/gu, '+').replace(/_/gu, '/') + '='.repeat((4 - value.length % 4) % 4))
    return Uint8Array.from(binary, character => character.charCodeAt(0))
  } catch { return null }
}

async function gzip(bytes: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new CompressionStream('gzip'))
  return new Uint8Array(await new Response(stream).arrayBuffer())
}

async function gunzipBounded(bytes: Uint8Array): Promise<Uint8Array | null> {
  try {
    const reader = new Blob([bytes as BlobPart]).stream().pipeThrough(new DecompressionStream('gzip')).getReader()
    const chunks: Uint8Array[] = []
    let total = 0
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      total += value.byteLength
      if (total > MAX_REPLAY_JSON_BYTES) { await reader.cancel(); return null }
      chunks.push(value)
    }
    const output = new Uint8Array(total)
    let offset = 0
    for (const chunk of chunks) { output.set(chunk, offset); offset += chunk.length }
    return output
  } catch { return null }
}

function validState(value: unknown): boolean {
  if (!isRecord(value) || !isRecord(value.board) || !Number.isInteger(value.board.size)) return false
  const size = value.board.size as number
  if (size < 8 || size > 12 || !isRecord(value.pieces) || Object.keys(value.pieces).length > MAX_REPLAY_PIECES) return false
  return isRecord(value.piece_definitions) && isRecord(value.players) && Array.isArray(value.history)
}

function validClock(value: unknown): boolean {
  if (!isRecord(value) || !['countdown', 'unlimited'].includes(String(value.mode))) return false
  if (!['white', 'black'].includes(String(value.active_color)) || !Number.isFinite(value.server_now_ms)) return false
  return typeof value.time_control === 'string' && Number.isFinite(value.increment_ms)
}

function validSquare(value: unknown, size: number): boolean {
  return isRecord(value) && Number.isInteger(value.file) && Number.isInteger(value.rank)
    && (value.file as number) >= 0 && (value.file as number) < size && (value.rank as number) >= 0 && (value.rank as number) < size
}

function validText(value: unknown, max = 128): boolean { return typeof value === 'string' && value.length > 0 && value.length <= max }

const DELTA_ROOTS = new Set(['board', 'pieces', 'players', 'current_player', 'turn_number', 'phase', 'en_passant_target', 'en_passant_available_to', 'global_state', 'result'])
const FORBIDDEN_PATH_SEGMENTS = new Set(['__proto__', 'prototype', 'constructor'])
function validDelta(value: unknown): boolean {
  if (!Array.isArray(value) || value.length > MAX_REPLAY_DELTA_OPERATIONS) return false
  return value.every(operation => {
    if (!isRecord(operation) || !['set', 'remove'].includes(String(operation.op)) || !Array.isArray(operation.path)) return false
    const path = operation.path
    if (path.length < 1 || path.length > 8 || !path.every(segment => validText(segment, 256) && !FORBIDDEN_PATH_SEGMENTS.has(String(segment))) || !DELTA_ROOTS.has(String(path[0]))) return false
    if (path[0] === 'board' && !['squares', 'air_squares'].includes(String(path[1]))) return false
    if (operation.op === 'set' && !Object.prototype.hasOwnProperty.call(operation, 'value')) return false
    return operation.op !== 'remove' || !Object.prototype.hasOwnProperty.call(operation, 'value')
  })
}

function validNotation(value: unknown, size: number): boolean {
  if (!isRecord(value) || !Number.isInteger(value.turn_number) || (value.turn_number as number) < 1 || !Number.isInteger(value.move_number) || value.move_number !== Math.floor(((value.turn_number as number) + 1) / 2) || !['white', 'black'].includes(String(value.side))) return false
  if (!['move', 'move_with_ability', 'ability', 'drop'].includes(String(value.kind)) || !isRecord(value.actor)) return false
  if (!validText(value.actor.piece_id, 256) || !validText(value.actor.piece_type_id, 256) || !validText(value.actor.piece_name, 160)) return false
  if (value.from != null && !validSquare(value.from, size) || value.to != null && !validSquare(value.to, size) || value.target != null && !validSquare(value.target, size)) return false
  if (!Array.isArray(value.ability_events) || value.ability_events.length > 16) return false
  return value.ability_events.every(event => isRecord(event) && validText(event.ability_id, 128) && validText(event.ability_name, 160) && (event.target == null || validSquare(event.target, size)))
}

function validAction(value: unknown, index: number, size: number): boolean {
  if (!isRecord(value) || value.ply !== index + 1 || !isRecord(value.action) || !validClock(value.clock)) return false
  if (!['white', 'black'].includes(String(value.player_id)) || !Number.isFinite(value.elapsed_ms) || !validNotation(value.notation, size) || !validDelta(value.state_delta)) return false
  const type = value.action.type
  if (!['move', 'drop', 'ability'].includes(String(type))) return false
  if (!validText(value.action.piece_id, 256) || value.action.player_id !== value.player_id || (value.notation as Record<string, unknown>).side !== value.player_id) return false
  if (type === 'move') return validSquare(value.action.from, size) && validSquare(value.action.to, size) && validText(value.action.move_option_id, 128)
  if (type === 'drop') return validSquare(value.action.to, size)
  return validText(value.action.ability_id, 128) && (value.action.to == null || validSquare(value.action.to, size))
}

function parseRecord(value: unknown): GameRecord | null {
  if (!isRecord(value) || value.format_version !== 2 || typeof value.game_id !== 'string' || value.game_id.length > 128) return null
  if (typeof value.ruleset_version !== 'string' || value.ruleset_version.length > 64 || typeof value.chessembly_version !== 'string' || value.chessembly_version.length > 64 || typeof value.display_name !== 'string' || value.display_name.length > 160) return null
  if (!Number.isSafeInteger(value.started_at_ms) || !validState(value.initial_state) || !validClock(value.initial_clock)) return null
  if (!isRecord(value.players) || !isRecord(value.players.white) || !isRecord(value.players.black)) return null
  for (const side of ['white', 'black']) {
    const player = value.players[side]
    if (!isRecord(player) || player.side !== side || (player.public_id !== null && (typeof player.public_id !== 'string' || player.public_id.length > 64)) || typeof player.nickname !== 'string' || player.nickname.length > 80) return null
  }
  if (!isRecord(value.decks) || !isRecord(value.decks.white) || !isRecord(value.decks.black)) return null
  const size = (value.initial_state as Record<string, unknown> & { board: { size: number } }).board.size
  for (const side of ['white', 'black']) {
    const deck = value.decks[side]
    if (!isRecord(deck) || deck.side !== side || !validText(deck.deck_name, 160) || !Array.isArray(deck.deployments) || !Array.isArray(deck.pocket)) return null
    if (deck.deployments.length > MAX_REPLAY_PIECES || deck.pocket.length > MAX_REPLAY_PIECES) return null
    if (!deck.deployments.every(entry => isRecord(entry) && validText(entry.piece_name, 160) && validSquare(entry.square, size))) return null
    if (!deck.pocket.every(entry => isRecord(entry) && validText(entry.piece_name, 160) && Number.isInteger(entry.count) && (entry.count as number) > 0 && (entry.count as number) <= MAX_REPLAY_PIECES)) return null
  }
  if (!Array.isArray(value.actions) || value.actions.length > MAX_REPLAY_ACTIONS || !value.actions.every((action, index) => validAction(action, index, size))) return null
  const moveNumbers = (value.actions as Array<{ notation: { move_number: number } }>).map(action => action.notation.move_number)
  if (moveNumbers.some((moveNumber, index) => index > 0 && (moveNumber < moveNumbers[index - 1] || moveNumber > moveNumbers[index - 1] + 1))) return null
  const turnNumbers = (value.actions as Array<{ notation: { turn_number: number } }>).map(action => action.notation.turn_number)
  if (turnNumbers.some((turnNumber, index) => index > 0 && (turnNumber < turnNumbers[index - 1] || turnNumber > turnNumbers[index - 1] + 1))) return null
  if (value.final_clock != null && !validClock(value.final_clock)) return null
  return value as unknown as GameRecord
}

export async function encodeReplayCode(record: GameRecord): Promise<string> {
  const json = new TextEncoder().encode(JSON.stringify(record))
  if (json.byteLength > MAX_REPLAY_JSON_BYTES) throw new Error('대국 기록이 공유 코드 제한을 초과했습니다.')
  return `${REPLAY_CODE_PREFIX}${base64Url(await gzip(json))}`
}

export async function decodeReplayCode(input: string): Promise<ReplayDecodeResult> {
  if (!input.trim()) return { ok: false, error: 'empty' }
  if (input.length > MAX_REPLAY_CODE_LENGTH) return { ok: false, error: 'too_large' }
  const compact = input.replace(/\s/gu, '')
  const versionMatch = /^DC-G(\d+)-/u.exec(compact)
  if (!versionMatch) return { ok: false, error: 'invalid_format' }
  if (versionMatch[1] !== '2') return { ok: false, error: 'unsupported_version' }
  const compressed = fromBase64Url(compact.slice(REPLAY_CODE_PREFIX.length))
  if (!compressed) return { ok: false, error: 'invalid_payload' }
  const bytes = await gunzipBounded(compressed)
  if (!bytes) return { ok: false, error: 'invalid_payload' }
  try {
    const value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as unknown
    const record = parseRecord(value)
    return record ? { ok: true, value: record } : { ok: false, error: 'invalid_schema' }
  } catch { return { ok: false, error: 'invalid_payload' } }
}
