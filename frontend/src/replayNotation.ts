import type { GameState, TurnAction } from './types/game'

function squareName(square: { file: number; rank: number }): string {
  return `${String.fromCharCode(97 + square.file)}${square.rank + 1}`
}

export function formatAction(action: TurnAction, state: GameState): string {
  const piece = state.pieces[action.piece_id]
  const name = piece ? state.piece_definitions[piece.type_id]?.name ?? piece.type_id : action.piece_id
  if (action.type === 'move') return `${name} ${squareName(action.from)}-${squareName(action.to)}`
  if (action.type === 'drop') return `${name}@${squareName(action.to)}`
  return `${name}.${action.ability_id}`
}
