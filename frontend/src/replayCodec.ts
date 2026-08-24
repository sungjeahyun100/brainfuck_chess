import type { GameRecord } from './types/gameRecord'

export const REPLAY_CODE_PREFIX = 'DC-G1-'
export const MAX_REPLAY_CODE_LENGTH = 4_000_000
export const MAX_REPLAY_JSON_BYTES = 32_000_000
export const MAX_REPLAY_ACTIONS = 4_096
export const MAX_REPLAY_PIECES = 2_048

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

function validAction(value: unknown, index: number): boolean {
  if (!isRecord(value) || value.ply !== index + 1 || !isRecord(value.action) || !validState(value.state_after) || !validClock(value.clock)) return false
  if (!['white', 'black'].includes(String(value.player_id)) || !Number.isInteger(value.piece_index) || (value.piece_index as number) < 0 || (value.piece_index as number) >= MAX_REPLAY_PIECES || !Number.isFinite(value.elapsed_ms) || typeof value.state_hash !== 'string') return false
  const type = value.action.type
  if (!['move', 'drop', 'ability'].includes(String(type))) return false
  return typeof value.action.piece_id === 'string' && value.action.piece_id.length <= 256
}

function parseRecord(value: unknown): GameRecord | null {
  if (!isRecord(value) || value.format_version !== 1 || typeof value.game_id !== 'string' || value.game_id.length > 128) return null
  if (typeof value.ruleset_version !== 'string' || value.ruleset_version.length > 64 || typeof value.chessembly_version !== 'string' || value.chessembly_version.length > 64 || typeof value.display_name !== 'string' || value.display_name.length > 160) return null
  if (!Number.isSafeInteger(value.started_at_ms) || !validState(value.initial_state) || !validClock(value.initial_clock)) return null
  if (!isRecord(value.players) || !isRecord(value.players.white) || !isRecord(value.players.black)) return null
  for (const side of ['white', 'black']) {
    const player = value.players[side]
    if (!isRecord(player) || player.side !== side || typeof player.public_id !== 'string' || player.public_id.length > 64 || typeof player.nickname !== 'string' || player.nickname.length > 80) return null
  }
  if (!isRecord(value.piece_id_map) || Object.keys(value.piece_id_map).length > MAX_REPLAY_PIECES) return null
  const indexes = Object.values(value.piece_id_map)
  if (indexes.some(index => !Number.isInteger(index) || (index as number) < 0 || (index as number) >= MAX_REPLAY_PIECES) || new Set(indexes).size !== indexes.length) return null
  if (!Array.isArray(value.actions) || value.actions.length > MAX_REPLAY_ACTIONS || !value.actions.every(validAction)) return null
  const allowedIndexes = new Set(indexes as number[])
  if ((value.actions as Array<Record<string, unknown>>).some(action => !allowedIndexes.has(action.piece_index as number))) return null
  if (value.final_state != null && !validState(value.final_state)) return null
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
  if (versionMatch[1] !== '1') return { ok: false, error: 'unsupported_version' }
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

export async function replayHashesMatch(record: GameRecord): Promise<boolean> {
  if (!globalThis.crypto?.subtle) return true
  for (const entry of record.actions) {
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify(entry.state_after))))
    const actual = [...digest].map(byte => byte.toString(16).padStart(2, '0')).join('')
    if (actual !== entry.state_hash.toLowerCase()) return false
  }
  return true
}
