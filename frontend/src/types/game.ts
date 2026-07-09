// Types mirroring the Rust engine's JSON serialization.

export type PlayerId = 'white' | 'black'
export type SquareId = string
export type PieceId = string
export type PieceTypeId = string

export interface Square {
  file: number
  rank: number
}

export interface Board {
  size: number
  /** SquareId → PieceId | null */
  squares: Record<SquareId, PieceId | null>
}

export type ChessemblyDialect = 'classic' | 'brainfuck-chess'

export interface PieceDefinition {
  id: PieceTypeId
  name: string
  score: number
  chessembly_code: string
  chessembly_version: string
  dialect?: ChessemblyDialect
  extensions?: string[]
  is_king: boolean
  abilities?: PieceAbilityDefinition[]
}

export interface PieceAbilityDefinition {
  id: string
  name: string
  description: string
  duration: 'until_turn_end' | 'until_piece_moves' | 'permanent' | { turns: number }
  once_per_turn: boolean
  cooldown_turns?: number
}

export interface ActiveAbilityState {
  ability_id: string
  activated_turn_number: number
  activated_player: PlayerId
  duration: PieceAbilityDefinition['duration']
}

export interface Piece {
  id: PieceId
  owner: PlayerId
  type_id: PieceTypeId
  current_square?: Square
  in_pocket: boolean
  captured: boolean
  has_moved: boolean
  active_ability?: ActiveAbilityState | null
  ability_cooldowns?: Record<string, number>
}

export interface Deck {
  player_id: PlayerId
  starting_pieces: PieceId[]
  pocket_pieces: PieceId[]
  score_limit: number
  total_score: number
}

export interface Player {
  id: PlayerId
  deck: Deck
  captured_pieces: PieceId[]
}

export type TurnMode = 'undecided' | 'move' | 'drop'

export interface TurnState {
  mode: TurnMode
  actions: TurnAction[]
}

export interface MoveAction {
  type: 'move'
  player_id: PlayerId
  piece_id: PieceId
  from: Square
  to: Square
  captured_piece_id?: PieceId
  promotion?: PieceTypeId
  ability_id?: string
}

export interface DropAction {
  type: 'drop'
  player_id: PlayerId
  piece_id: PieceId
  to: Square
}

export type TurnAction = MoveAction | DropAction

export type BotDifficulty = 'easy' | 'normal' | 'hard'

export interface EndTurnAiAction {
  type: 'end_turn'
}

export type AiAction = MoveAction | DropAction | EndTurnAiAction

export type ActionEffect =
  | {
    type: 'move_piece'
    piece_id: PieceId
    from: Square
    to: Square
  }
  | {
    type: 'capture_piece'
    piece_id: PieceId
    at: Square
  }
  | {
    type: 'drop_piece'
    piece_id: PieceId
    to: Square
  }
  | {
    type: 'promote_piece'
    piece_id: PieceId
    from_type: PieceTypeId
    to_type: PieceTypeId
  }
  | {
    type: 'swap_pieces'
    first_piece_id: PieceId
    second_piece_id: PieceId
    first_to: Square
    second_to: Square
  }
  | {
    type: 'set_piece_ability'
    piece_id: PieceId
    ability_id: string
  }
  | {
    type: 'clear_piece_ability'
    piece_id: PieceId
    ability_id: string
  }
  | {
    type: 'set_ability_cooldown'
    piece_id: PieceId
    ability_id: string
    usable_turn: number
  }
  | {
    type: 'set_en_passant'
    target: Square | null
    available_to: PlayerId | null
  }
  | {
    type: 'advance_turn'
    from_player: PlayerId
    to_player: PlayerId
    turn_number: number
  }
  | {
    type: 'end_game'
    result: GameResult
  }

export interface ActionTimelineFrame {
  action: AiAction
  effects: ActionEffect[]
}

export interface BotTurnStats {
  searched_nodes: number
  depth_reached: number
  elapsed_ms: number
}

export interface BotTurnResponse {
  ok: boolean
  game_state: GameState
  actions: AiAction[]
  timeline?: ActionTimelineFrame[]
  stats: BotTurnStats
}

export type GamePhase = 'setup' | 'playing' | 'ended'

export type GameEndReason = 'king_capture' | 'resignation' | 'timeout' | 'draw'

export interface GameResult {
  winner?: PlayerId
  reason: GameEndReason
}

export interface GameState {
  id: string
  board: Board
  pieces: Record<PieceId, Piece>
  piece_definitions: Record<PieceTypeId, PieceDefinition>
  players: Record<PlayerId, Player>
  current_player: PlayerId
  turn_number: number
  phase: GamePhase
  en_passant_target?: Square | null
  en_passant_available_to?: PlayerId | null
  turn_state: TurnState
  result?: GameResult
}

export interface AttackMap {
  player_id: PlayerId
  attacked_squares: string[]
  source_map: Record<SquareId, PieceId[]>
}
