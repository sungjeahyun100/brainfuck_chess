import type { AbilityAction, GameState, MoveAction, MoveOptionDefinition, Square } from './types/game'

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

export function abilityActionTargetsSquare(
  action: AbilityAction,
  actorSquare: Square | undefined,
  target: Square,
): boolean {
  if (action.to) return action.to.file === target.file && action.to.rank === target.rank
  return action.ability_id === 'bomb'
    && actorSquare?.file === target.file
    && actorSquare.rank === target.rank
}

export function abilitySelectionSquares(
  actions: AbilityAction[],
  actorSquare: Square | undefined,
): Square[] {
  return actions.flatMap(action => {
    if (action.to) return [action.to]
    return action.ability_id === 'bomb' && actorSquare ? [actorSquare] : []
  })
}

export function pendingForcedLandingPieceId(
  state: Pick<GameState, 'current_player' | 'pieces'>,
): string | null {
  return Object.values(state.pieces).find(piece => (
    piece.owner === state.current_player
      && !piece.captured
      && !piece.in_pocket
      && piece.current_square
      && piece.layer === 'air'
      && (piece.remaining_flight_turns ?? 0) === 0
      && piece.state.airborne === true
  ))?.id ?? null
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
