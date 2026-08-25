import type { GameClock, GameResult, GameState, PieceStateValue, PlayerId, Square, TimeControlId, TurnAction } from './game'

export interface GameRecordPlayer { public_id: string; nickname: string; side: PlayerId }
export interface DeckSnapshot { side: PlayerId; deck_name: string; deployments: Array<{ piece_name: string; square: Square }>; pocket: Array<{ piece_name: string; count: number }> }
export type NotationActionKind = 'move' | 'move_with_ability' | 'ability' | 'drop'
export interface ActorSnapshot { piece_id: string; piece_type_id: string; piece_name: string; from?: Square | null; layer: 'ground' | 'air'; current_ammo?: number | null; state: Record<string, PieceStateValue> }
export interface AbilityEventSnapshot { ability_id: string; ability_name: string; target?: Square | null }
export interface RecordedNotationAction {
  move_number: number; side: PlayerId; actor: ActorSnapshot; kind: NotationActionKind
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
