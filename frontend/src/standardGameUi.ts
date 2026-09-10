import type { GameState } from './types/game'

// Object insertion order is not a gameplay change (Rust maps can reorder on sync).
function ordered(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(ordered)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => [k, ordered(v)]))
  return value
}
export function gameplayKey(state: GameState): string {
  return JSON.stringify(ordered([state.id, state.ruleset, state.current_player, state.turn_number,
    state.phase, state.result, state.history.length, state.board, state.players, state.pieces,
    state.piece_definitions, state.global_state, state.en_passant_target, state.en_passant_available_to]))
}

// Only known public messages cross this UI boundary; never echo arbitrary server errors.
export function gameActionError(cause: unknown): string {
  const message = cause instanceof Error ? cause.message : ''
  const messages: Record<string, string> = {
    '제물 점수가 소환 비용보다 부족합니다.': '제물 점수가 부족합니다.',
    'Hand 기물이 없습니다.': '선택한 기물이 더 이상 Hand에 없습니다.',
    'Extra 기물이 없습니다.': '더 이상 Extra Deck에 없는 기물입니다.',
    '자신의 Extra 기물만 소환할 수 있습니다.': '현재 자신의 Extra Deck에 있는 기물을 선택하세요.',
    '현재 차례가 아닙니다.': '현재 턴이 아닙니다.',
    '현재 Extra 소환을 할 수 없습니다.': '현재 소환할 수 없습니다. 턴과 강제 착륙 상태를 확인하세요.',
    '소환할 수 없는 위치입니다.': '소환할 수 없는 위치입니다.',
    '허용되지 않는 제물 zone입니다.': '선택한 제물을 사용할 수 없습니다. 후보를 다시 선택하세요.',
  }
  return messages[message] ?? '요청을 처리하지 못했습니다. 현재 턴과 기물 상태를 확인한 뒤 다시 시도하세요.'
}
