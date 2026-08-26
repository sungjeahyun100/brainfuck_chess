import { toRaw } from 'vue'
import type { GameState } from './types/game'
import type { GameRecord, StateDeltaOperation } from './types/gameRecord'

function unwrapVueProxies(value: unknown, seen = new WeakMap<object, unknown>()): unknown {
  if (value === null || typeof value !== 'object') return value
  const raw = toRaw(value)
  const existing = seen.get(raw)
  if (existing) return existing

  if (Array.isArray(raw)) {
    const copy: unknown[] = []
    seen.set(raw, copy)
    for (const entry of raw) copy.push(unwrapVueProxies(entry, seen))
    return copy
  }

  if (raw instanceof Date) return new Date(raw.getTime())
  if (raw instanceof Map) {
    const copy = new Map<unknown, unknown>()
    seen.set(raw, copy)
    for (const [key, entry] of raw) copy.set(unwrapVueProxies(key, seen), unwrapVueProxies(entry, seen))
    return copy
  }
  if (raw instanceof Set) {
    const copy = new Set<unknown>()
    seen.set(raw, copy)
    for (const entry of raw) copy.add(unwrapVueProxies(entry, seen))
    return copy
  }

  const copy: Record<PropertyKey, unknown> = {}
  seen.set(raw, copy)
  for (const key of Reflect.ownKeys(raw)) {
    Object.defineProperty(copy, key, {
      configurable: true,
      enumerable: Object.prototype.propertyIsEnumerable.call(raw, key),
      value: unwrapVueProxies(Reflect.get(raw, key), seen),
      writable: true,
    })
  }
  return copy
}

function clone<T>(value: T): T {
  return structuredClone(unwrapVueProxies(value)) as T
}
const FORBIDDEN_SEGMENTS = new Set(['__proto__', 'prototype', 'constructor'])

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function isRenderableReplayRecord(value: unknown): value is GameRecord {
  if (!isRecord(value) || !isRecord(value.initial_state) || !isRecord(value.initial_clock)) return false
  const initialState = value.initial_state
  if (!isRecord(initialState.board) || !isRecord(initialState.pieces) || !isRecord(initialState.piece_definitions)) return false
  if (!isRecord(value.players) || !isRecord(value.players.white) || !isRecord(value.players.black)) return false
  if (!isRecord(value.decks) || !isRecord(value.decks.white) || !isRecord(value.decks.black)) return false
  for (const side of ['white', 'black']) {
    const deck = value.decks[side]
    if (!isRecord(deck) || !Array.isArray(deck.deployments) || !Array.isArray(deck.pocket)) return false
  }
  return Array.isArray(value.actions) && value.actions.every(action => (
    isRecord(action)
    && Array.isArray(action.state_delta)
    && isRecord(action.clock)
    && isRecord(action.action)
    && isRecord(action.notation)
    && isRecord(action.notation.actor)
    && Array.isArray(action.notation.ability_events)
  ))
}

export type ReplayFramesResult =
  | { ok: true; frames: GameState[] }
  | { ok: false; error: 'invalid_replay' }

export function applyStateDelta(state: GameState, operations: StateDeltaOperation[]): GameState {
  const next = clone(state) as unknown as Record<string, unknown>
  for (const operation of operations) {
    if (operation.path.some(segment => FORBIDDEN_SEGMENTS.has(segment))) throw new Error('위험한 Replay state delta 경로입니다.')
    let parent: Record<string, unknown> = next
    for (const segment of operation.path.slice(0, -1)) {
      const child = parent[segment]
      if (child === null || typeof child !== 'object' || Array.isArray(child)) throw new Error('잘못된 Replay state delta입니다.')
      parent = child as Record<string, unknown>
    }
    const key = operation.path[operation.path.length - 1]
    if (!key) throw new Error('빈 Replay state delta 경로입니다.')
    if (operation.op === 'remove') delete parent[key]
    else parent[key] = clone(operation.value)
  }
  next.history = []
  return next as unknown as GameState
}

export function buildReplayFrames(record: GameRecord): GameState[] {
  const frames = [clone(record.initial_state)]
  for (const entry of record.actions) frames.push(applyStateDelta(frames[frames.length - 1], entry.state_delta))
  return frames
}

export function buildReplayFramesResult(record: unknown): ReplayFramesResult {
  if (!isRenderableReplayRecord(record)) return { ok: false, error: 'invalid_replay' }
  try {
    return { ok: true, frames: buildReplayFrames(record) }
  } catch {
    return { ok: false, error: 'invalid_replay' }
  }
}
