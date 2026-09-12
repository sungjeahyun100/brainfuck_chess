import type { GameState, TurnAction } from './types/game'
import type { RecordedNotationAction } from './types/gameRecord'

export function squareName(square: { file: number; rank: number } | null | undefined): string {
  if (!square) return '?'
  return `${String.fromCharCode(97 + square.file)}${square.rank + 1}`
}

export function formatNotation(notation: RecordedNotationAction): string {
  if (notation.kind === 'draw') return '카드 1장 드로우'
  const name = notation.actor.piece_name
  if (notation.kind === 'extra_summon') return `${name} - 특수 소환 - ${squareName(notation.to)}`
  if (notation.kind === 'drop') return `${name} - 착수 - ${squareName(notation.to)}`
  if (notation.kind === 'ability' || notation.kind === 'move_with_ability') {
    return `${name} - ${notation.ability_name ?? notation.ability_id ?? '능력'} - ${squareName(notation.from)} - ${squareName(notation.to ?? notation.target)}`
  }
  return `${name} - ${squareName(notation.from)} - ${squareName(notation.to)}`
}

export interface NotationGroup<T> { moveNumber: number; entries: T[] }

export function groupNotation<T extends { notation: RecordedNotationAction }>(actions: T[]): NotationGroup<T>[] {
  const rows = new Map<number, NotationGroup<T>>()
  for (const entry of actions) {
    const row = rows.get(entry.notation.move_number) ?? { moveNumber: entry.notation.move_number, entries: [] }
    row.entries.push(entry)
    rows.set(row.moveNumber, row)
  }
  return [...rows.values()]
}

export function fullMoveNumber(engineTurnNumber: number): number {
  return Math.floor((engineTurnNumber + 1) / 2)
}

export function formatLiveAction(action: TurnAction, state: GameState, engineTurnNumber: number): string {
  if (action.type === 'draw') return '카드 1장 드로우'
  if (action.type === 'extra_summon') {
    const piece = state.pieces[action.extra_piece_id]
    const name = state.piece_definitions[piece?.type_id ?? '']?.name ?? action.extra_piece_id
    return `${name} - 특수 소환 - ${squareName(action.target_square)}`
  }
  const piece = state.pieces[action.piece_id]
  const definition = piece ? state.piece_definitions[piece.type_id] : undefined
  const moveOption = action.type === 'move' ? definition?.move_options.find(option => option.id === action.move_option_id) : undefined
  const abilityId = action.type === 'ability' ? action.ability_id : action.type === 'move' && moveOption?.kind === 'ability' ? action.move_option_id : undefined
  const abilityName = abilityId ? definition?.move_options.find(option => option.id === abilityId)?.name ?? abilityId : undefined
  return formatNotation({
    turn_number: engineTurnNumber, move_number: fullMoveNumber(engineTurnNumber), side: action.player_id,
    actor: { piece_id: action.piece_id, piece_type_id: piece?.type_id ?? 'unknown', piece_name: definition?.name ?? piece?.type_id ?? 'unknown', from: action.type === 'move' ? action.from : piece?.current_square, layer: piece?.layer ?? 'ground', current_ammo: piece?.current_ammo, state: piece?.state ?? {} },
    kind: action.type === 'drop' ? 'drop' : action.type === 'ability' ? 'ability' : abilityId ? 'move_with_ability' : 'move',
    ability_id: abilityId, ability_name: abilityName, from: action.type === 'move' ? action.from : piece?.current_square,
    to: action.type === 'move' || action.type === 'drop' ? action.to : action.to, target: action.type === 'ability' ? action.to : undefined,
    ability_events: abilityId ? [{ ability_id: abilityId, ability_name: abilityName ?? abilityId, target: action.type === 'ability' ? action.to : undefined }] : [],
  })
}

/** Count-only automatic event. Safe for ordinary/live notation; never names pieces. */
export function formatDraw(draw: { player_id: string; piece_ids: readonly string[] }): string {
  return draw.piece_ids.length ? `${draw.player_id === 'white' ? '백' : '흑'} 드로우 ${draw.piece_ids.length}기` : ''
}

/** Committed sacrifices remain public removed pieces; selected IDs retain their order. */
export function summonDetail(action: TurnAction | null | undefined, state: GameState): string {
  if (action?.type === 'drop' && action.sacrifice_piece_ids?.length) {
    const label = (id: string) => state.piece_definitions[state.pieces[id]?.type_id]?.name ?? id
    return `${label(action.piece_id)} → ${squareName(action.to)} · 제물 ${action.sacrifice_piece_ids.length}체: ${action.sacrifice_piece_ids.map(label).join(', ')}`
  }
  if (action?.type !== 'extra_summon') return ''
  const label = (id: string) => { const piece = state.pieces[id]; const def = state.piece_definitions[piece?.type_id ?? '']; return `${def?.name ?? id} [${def?.score ?? 0}]` }
  const total = action.sacrifice_piece_ids.reduce((sum, id) => sum + (state.piece_definitions[state.pieces[id]?.type_id ?? '']?.score ?? 0), 0)
  return `${label(action.extra_piece_id)} → ${squareName(action.target_square)} · 제물: ${action.sacrifice_piece_ids.map(label).join(', ')} · 합계 ${total}점`
}
