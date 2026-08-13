import type { AbilityAction, MoveAction, MoveOptionDefinition, Square } from './types/game'

type MoveOptionExecution = Pick<MoveOptionDefinition, 'execution_mode'> | null | undefined

export function usesMoveSubmission(option: MoveOptionExecution): boolean {
  return option?.execution_mode === 'move_modifier'
}

export function moveOptionTargets(moves: MoveAction[], abilityActions: AbilityAction[]): {
  legalTargets: Square[]
  movable: Square[]
  captures: Square[]
} {
  const abilityTargets = abilityActions.flatMap(action => action.to ? [action.to] : [])
  return {
    legalTargets: [...moves.map(move => move.to), ...abilityTargets],
    movable: [
      ...moves.filter(move => !move.captured_piece_id).map(move => move.to),
      ...abilityTargets,
    ],
    captures: moves.filter(move => Boolean(move.captured_piece_id)).map(move => move.to),
  }
}
