import type { GameState, TurnAction } from './types/game'
import type { RecordedNotationAction } from './types/gameRecord'

export function squareName(square: { file: number; rank: number } | null | undefined): string {
  if (!square) return '?'
  return `${String.fromCharCode(97 + square.file)}${square.rank + 1}`
}

export function formatNotation(notation: RecordedNotationAction): string {
  const name = notation.actor.piece_name
  if (notation.kind === 'drop') return `${name} - 착수 - ${squareName(notation.to)}`
  if (notation.kind === 'ability' || notation.kind === 'move_with_ability') {
    return `${name} - ${notation.ability_name ?? notation.ability_id ?? '능력'} - ${squareName(notation.from)} - ${squareName(notation.to ?? notation.target)}`
  }
  return `${name} - ${squareName(notation.from)} - ${squareName(notation.to)}`
}

export function groupNotation<T extends { notation: RecordedNotationAction }>(actions: T[]): Array<{ moveNumber: number; white?: T; black?: T }> {
  const rows = new Map<number, { moveNumber: number; white?: T; black?: T }>()
  for (const entry of actions) {
    const row = rows.get(entry.notation.move_number) ?? { moveNumber: entry.notation.move_number }
    row[entry.notation.side] = entry
    rows.set(row.moveNumber, row)
  }
  return [...rows.values()].sort((left, right) => left.moveNumber - right.moveNumber)
}

export function formatLiveAction(action: TurnAction, state: GameState, ply: number): string {
  const piece = state.pieces[action.piece_id]
  const definition = piece ? state.piece_definitions[piece.type_id] : undefined
  const moveOption = action.type === 'move' ? definition?.move_options.find(option => option.id === action.move_option_id) : undefined
  const abilityId = action.type === 'ability' ? action.ability_id : action.type === 'move' && moveOption?.kind === 'ability' ? action.move_option_id : undefined
  const abilityName = abilityId ? definition?.move_options.find(option => option.id === abilityId)?.name ?? abilityId : undefined
  return formatNotation({
    move_number: Math.floor((ply + 1) / 2), side: action.player_id,
    actor: { piece_id: action.piece_id, piece_type_id: piece?.type_id ?? 'unknown', piece_name: definition?.name ?? piece?.type_id ?? 'unknown', from: action.type === 'move' ? action.from : piece?.current_square, layer: piece?.layer ?? 'ground', current_ammo: piece?.current_ammo, state: piece?.state ?? {} },
    kind: action.type === 'drop' ? 'drop' : action.type === 'ability' ? 'ability' : abilityId ? 'move_with_ability' : 'move',
    ability_id: abilityId, ability_name: abilityName, from: action.type === 'move' ? action.from : piece?.current_square,
    to: action.type === 'move' || action.type === 'drop' ? action.to : action.to, target: action.type === 'ability' ? action.to : undefined,
    ability_events: abilityId ? [{ ability_id: abilityId, ability_name: abilityName ?? abilityId, target: action.type === 'ability' ? action.to : undefined }] : [],
  })
}
