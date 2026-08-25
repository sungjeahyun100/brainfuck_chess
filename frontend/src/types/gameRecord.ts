import type { GameClock, GameResult, GameState, PieceStateValue, PlayerId, Square, TimeControlId, TurnAction } from './game'

export interface GameRecordPlayer { public_id: string | null; nickname: string; side: PlayerId }
export interface DeckSnapshot {
  snapshot_version?: number; side: PlayerId; deck_name: string; map_id?: string; board_size?: number
  deployments: Array<{ piece_type_id?: string; piece_name: string; custom_piece?: { custom_piece_id: string; version: number; content_hash: string; exposed_piece_key: string } | null; square: Square }>
  pocket: Array<{ piece_type_id?: string; piece_name: string; custom_piece?: { custom_piece_id: string; version: number; content_hash: string; exposed_piece_key: string } | null; count: number }>
}
export type NotationActionKind = 'move' | 'move_with_ability' | 'ability' | 'drop'
export interface ActorSnapshot { piece_id: string; piece_type_id: string; piece_name: string; from?: Square | null; layer: 'ground' | 'air'; current_ammo?: number | null; state: Record<string, PieceStateValue> }
export interface AbilityEventSnapshot { ability_id: string; ability_name: string; target?: Square | null }
export interface RecordedNotationAction {
  turn_number: number; move_number: number; side: PlayerId; actor: ActorSnapshot; kind: NotationActionKind
  ability_id?: string | null; ability_name?: string | null; from?: Square | null; to?: Square | null; target?: Square | null
  ability_events: AbilityEventSnapshot[]
}
export type StateDeltaOperation = { op: 'set'; path: string[]; value: unknown } | { op: 'remove'; path: string[] }
export interface RecordedAction {
  ply: number; player_id: PlayerId; action: TurnAction; notation: RecordedNotationAction; state_delta: StateDeltaOperation[]
  elapsed_ms: number; clock_before_ms?: number | null; clock_after_ms?: number | null; clock: GameClock
}
export interface GameRecord {
  format_version: 2; game_id: string; display_name: string; ruleset_version: string; chessembly_version: string
  started_at_ms: number; ended_at_ms?: number | null; result?: GameResult | null
  players: Record<PlayerId, GameRecordPlayer>; time_control: TimeControlId
  initial_state: GameState; initial_clock: GameClock; decks: Record<PlayerId, DeckSnapshot>; actions: RecordedAction[]; final_clock?: GameClock | null
}
export interface GameRecordSummary {
  game_id: string; display_name: string; started_at_ms: number; ended_at_ms?: number | null
  result?: GameResult | null; players: Record<PlayerId, GameRecordPlayer>; time_control: TimeControlId; owner_side: PlayerId
}
