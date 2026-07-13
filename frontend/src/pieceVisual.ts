import type { Piece, PieceDefinition, PieceStatePredicate } from './types/game.ts'

function predicateMatches(piece: Piece, predicate: PieceStatePredicate): boolean {
  const current = piece.state?.[predicate.key]
  if ('equals' in predicate.condition) return current === predicate.condition.equals
  return current !== undefined && current !== predicate.condition.not_equals
}

export function resolvePieceAssetKey(
  piece: Piece,
  definition: PieceDefinition | undefined,
): string {
  if (!definition?.visual) return piece.type_id

  const variant = [...(definition.visual.variants ?? [])]
    .sort((left, right) => right.priority - left.priority)
    .find(candidate => candidate.enabled_when.every(predicate => predicateMatches(piece, predicate)))

  return variant?.asset_key || definition.visual.default_asset_key || piece.type_id
}
