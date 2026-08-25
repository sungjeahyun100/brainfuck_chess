import type { GameState } from './types/game'
import type { GameRecord, StateDeltaOperation } from './types/gameRecord'

function clone<T>(value: T): T { return structuredClone(value) }
const FORBIDDEN_SEGMENTS = new Set(['__proto__', 'prototype', 'constructor'])

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
