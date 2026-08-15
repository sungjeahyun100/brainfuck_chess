import type { AbilityAction, MoveAction, MoveOptionDefinition, Square } from './types/game'

type MoveOptionExecution = Pick<MoveOptionDefinition, 'execution_mode'> | null | undefined

export function activeCooldownRemaining(
  cooldowns: Record<string, { remaining: number }> | null | undefined,
): number {
  return Object.values(cooldowns ?? {}).reduce(
    (largest, cooldown) => Math.max(largest, cooldown.remaining),
    0,
  )
}

export function usesMoveSubmission(option: MoveOptionExecution): boolean {
  return option?.execution_mode === 'move_modifier'
}

export function isImmediateAbilityAction(action: AbilityAction): boolean {
  return !action.to
    && !action.target_piece_id
    && !action.pocket_piece_id
    && action.deployments.length === 0
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
