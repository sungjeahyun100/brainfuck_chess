import type { ActionTimelineFrame, GameState } from '../types/game'

export function cloneGameState(state: GameState): GameState {
  return JSON.parse(JSON.stringify(state)) as GameState
}

/** Apply an authoritative server timeline frame. */
export function applyTimelineFrame(_current: GameState, frame: ActionTimelineFrame): GameState {
  const next = cloneGameState(frame.state)
  next.clock ??= _current.clock
  next.presence ??= _current.presence
  next.player_info ??= _current.player_info
  return next
}
