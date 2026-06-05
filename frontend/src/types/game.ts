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
}

export interface Piece {
  id: PieceId
  owner: PlayerId
  type_id: PieceTypeId
  current_square?: Square
  in_pocket: boolean
  captured: boolean
  move_stack: number
  has_moved: boolean
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
  moved_piece_ids: PieceId[]
}

export interface MoveAction {
  type: 'move'
  player_id: PlayerId
  piece_id: PieceId
  from: Square
  to: Square
  captured_piece_id?: PieceId
}

export interface DropAction {
  type: 'drop'
  player_id: PlayerId
  piece_id: PieceId
  to: Square
}

export type TurnAction = MoveAction | DropAction

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
