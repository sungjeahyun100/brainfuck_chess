import type { GameClock, GameResult, GameState, PlayerId, TimeControlId, TurnAction } from './game'

export interface GameRecordPlayer {
  public_id: string
  nickname: string
  side: PlayerId
}

export interface RecordedAction {
  ply: number
  piece_index: number
  player_id: PlayerId
  action: TurnAction
  elapsed_ms: number
  clock_before_ms?: number | null
  clock_after_ms?: number | null
  clock: GameClock
  state_hash: string
  state_after: GameState
}

export interface GameRecord {
  format_version: 1
  game_id: string
  display_name: string
  ruleset_version: string
  chessembly_version: string
  started_at_ms: number
  ended_at_ms?: number | null
  result?: GameResult | null
  players: Record<PlayerId, GameRecordPlayer>
  time_control: TimeControlId
  initial_state: GameState
  initial_clock: GameClock
  piece_id_map: Record<string, number>
  actions: RecordedAction[]
  final_state?: GameState | null
  final_clock?: GameClock | null
}
