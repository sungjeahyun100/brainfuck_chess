import type { DropAction, GameState } from './types/game.ts'

export function requiredDropSacrifices(state: GameState, id: string | null): number {
  if (state.ruleset !== 'standard' || !id) return 0
  const score = state.piece_definitions[state.pieces[id]?.type_id]?.score ?? 0
  return score >= 10 ? 2 : score >= 5 ? 1 : 0
}

export function matchesDropSacrifices(action: DropAction, selected: string[]): boolean {
  const ids = action.sacrifice_piece_ids ?? []
  return ids.length === selected.length && ids.every(id => selected.includes(id))
}

export function dropSacrificeCandidates(actions: DropAction[]): string[] {
  return [...new Set(actions.flatMap(action => action.sacrifice_piece_ids ?? []))].sort()
}
