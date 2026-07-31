import type { DropAction, GameState, MoveAction, PieceDefinition, PieceStateValue, PlayerId, Square } from './game'

export type CustomPieceImage =
  | { kind: 'built_in'; asset_key: BuiltInPieceAsset }
  | { kind: 'uploaded'; asset_id: string }

export type BuiltInPieceAsset = 'pawn' | 'rook' | 'bishop' | 'knight' | 'queen' | 'king'

export interface SimpleCustomPieceDraft {
  name: string
  description: string
  score: number
  image: CustomPieceImage
  movementCode: string
  abilities: CustomPieceAbilityDraft[]
}

export interface AdvancedCustomPieceDraft {
  name: string
  description: string
  score: number
  image: CustomPieceImage
  rawScript: string
  exposedPieceKey: string
}

export type CustomPieceAbilityDraft =
  | { kind: 'remember_value'; name: string; initialValue: number }

export interface CustomPieceInput {
  name: string
  description: string
  score: number
  image: CustomPieceImage
  raw_script: string
  exposed_piece_key: string
}

export interface CustomPieceRecord extends CustomPieceInput {
  id: string
  owner_id: string
  internal_piece_keys: string[]
  validation_status: 'valid'
  version: number
  content_hash: string
  created_at: number
  updated_at: number
  active: boolean
}

export interface CustomPieceDiagnostic {
  severity: 'error' | 'warning'
  code: string
  message: string
  limit_exceeded: boolean
  line?: number
  column?: number
  piece_key?: string
}

export interface CustomPieceValidation {
  valid: boolean
  diagnostics: CustomPieceDiagnostic[]
  exposed_piece_key: string | null
  internal_piece_keys: string[]
  preview_definitions: PieceDefinition[]
}

export interface CustomPieceImageAsset {
  asset_id: string
  media_type: string
  width: number
  height: number
  content_hash: string
}

export interface CustomPieceTestPiece {
  id: string
  piece_key: string
  owner: PlayerId
  square: Square
  state?: Record<string, PieceStateValue>
}

export interface CustomPieceTestBoard {
  board_size: number
  pieces: CustomPieceTestPiece[]
  current_player: PlayerId
}

export type CustomPieceTestDefinition =
  | CustomPieceInput
  | { custom_piece_id: string; version: number }

export interface CustomPieceTestResult {
  state: GameState
  legal_moves: MoveAction[]
  legal_drops: DropAction[]
  attacks: Square[]
}
