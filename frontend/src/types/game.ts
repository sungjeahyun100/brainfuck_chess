// Types mirroring the Rust engine's JSON serialization.

export type PlayerId = 'white' | 'black'
export type BoardVariant = 'plain' | 'central-high-ground'
export type BoardMapId =
  | 'standard-8x8'
  | 'standard-9x9'
  | 'standard-10x10'
  | 'standard-11x11'
  | 'standard-12x12'
  | 'central-high-ground-12x12'
export type SquareId = string
export type PieceId = string
export type PieceTypeId = string
export type DeploymentZone = 'front' | 'back'

export interface Square {
  file: number
  rank: number
}

export interface Board {
  size: number
  /** SquareId → PieceId | null */
  squares: Record<SquareId, PieceId | null>
  /** SquareId → terrain that remains on the square when pieces move. */
  terrain?: Record<SquareId, TerrainCell>
}

export interface TerrainCell {
  type_id: string
}

export type ChessemblyDialect = 'classic' | 'brainfuck-chess'

export interface PieceDefinition {
  id: PieceTypeId
  name: string
  score: number
  deployment_zone: DeploymentZone
  chessembly_code: string
  chessembly_version: string
  dialect?: ChessemblyDialect
  extensions?: string[]
  is_king: boolean
  can_capture_on_drop: boolean
  promotion?: PromotionRule
  promotion_pool?: PieceTypeId[]
  state_schema: PieceStateDefinition[]
  move_layers: MoveLayerDefinition[]
  move_options: MoveOptionDefinition[]
  visual: PieceVisualDefinition
}

export interface PromotionRule {
  condition:
    | { type: 'first_rank' }
    | { type: 'last_rank' }
    | { type: 'rank'; rank: number }
}

export type PieceStateValue = number | boolean | string

export interface PieceStateDefinition {
  key: string
  default_value: PieceStateValue
}

export type PieceStateCondition =
  | { equals: PieceStateValue }
  | { not_equals: PieceStateValue }

export interface PieceStatePredicate {
  key: string
  condition: PieceStateCondition
}

export interface PieceStateUpdateDefinition {
  key: string
  value: PieceStateValue
}

export interface MoveLayerDefinition {
  id: string
  chessembly_code: string
  enabled_when: PieceStatePredicate[]
  on_commit: PieceStateUpdateDefinition[]
}

export type MoveOptionKind = 'normal' | 'ability'
export type MoveOptionExecutionMode = 'move_modifier' | 'standalone_action'
export type CooldownClock = 'owner_turns' | 'global_turns'

export interface CooldownDefinition {
  turns: number
  clock: CooldownClock
}

export interface MoveOptionDefinition {
  id: string
  name: string
  description: string
  kind: MoveOptionKind
  layer_ids: string[]
  execution_mode: MoveOptionExecutionMode
  contributes_to_attack_map: boolean
  cooldown?: CooldownDefinition
}

export interface PieceVisualVariantDefinition {
  id: string
  enabled_when: PieceStatePredicate[]
  asset_key: string
  priority: number
}

export interface PieceVisualDefinition {
  default_asset_key: string
  variants: PieceVisualVariantDefinition[]
}

export interface Piece {
  id: PieceId
  owner: PlayerId
  type_id: PieceTypeId
  current_square?: Square
  in_pocket: boolean
  captured: boolean
  has_moved: boolean
  state: Record<string, PieceStateValue>
  move_option_cooldowns: Record<string, { remaining: number }>
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

export interface MoveAction {
  type: 'move'
  player_id: PlayerId
  piece_id: PieceId
  from: Square
  to: Square
  captured_piece_id?: PieceId
  promotion?: PieceTypeId
  move_option_id: string
  source_layer_ids: string[]
  effects: {
    global_state_updates: Array<{
      key: string
      value: number
    }>
    piece_state_updates: Array<{
      piece_id: PieceId
      key: string
      value: PieceStateValue
    }>
    cooldown_updates: Array<{
      piece_id: PieceId
      move_option_id: string
      remaining: number
    }>
  }
}

export interface SubmitMoveAction {
  type: 'move'
  piece_id: PieceId
  to: Square
  promotion?: PieceTypeId
  move_option_id?: string
}

export interface SubmitDropAction {
  type: 'drop'
  piece_id: PieceId
  to: Square
}

export interface AbilityAction {
  type: 'ability'
  player_id: PlayerId
  piece_id: PieceId
  ability_id: string
  target_piece_id?: PieceId
  pocket_piece_id?: PieceId
  to?: Square
  deployments: AbilityDeployment[]
}

export interface AbilityDeployment {
  pocket_piece_id: PieceId
  to: Square
}

export interface SubmitAbilityAction {
  type: 'ability'
  piece_id: PieceId
  ability_id: string
  target_piece_id?: PieceId
  pocket_piece_id?: PieceId
  to?: Square
  deployments?: AbilityDeployment[]
}

export type SubmitAction = SubmitMoveAction | SubmitDropAction | SubmitAbilityAction

export interface GlobalStateUpdate {
    key: string
    value: number
}

export interface DropAction {
  type: 'drop'
  player_id: PlayerId
  piece_id: PieceId
  to: Square
  captured_piece_id?: PieceId
}

export type TurnAction = MoveAction | DropAction | AbilityAction

export type BotDifficulty = 'easy' | 'normal' | 'hard'

export type AiAction = MoveAction | DropAction | AbilityAction

export interface ActionTimelineFrame {
  action: AiAction
  /** Authoritative state after applying action. */
  state: GameState
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
  timeline: ActionTimelineFrame[]
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
  custom_piece_manifest: Array<{
    package_id: string
    version: number
    content_hash: string
    definition_snapshot_hash: string
    exposed_type_id: PieceTypeId
    runtime_type_ids: PieceTypeId[]
  }>
  players: Record<PlayerId, Player>
  current_player: PlayerId
  turn_number: number
  phase: GamePhase
  en_passant_target?: Square | null
  en_passant_available_to?: PlayerId | null
  global_state?: Record<string, number>
  history: Array<{
    turn_number: number
    player_id: PlayerId
    action: TurnAction
  }>
  result?: GameResult
}

export interface AttackMap {
  player_id: PlayerId
  attacked_squares: string[]
  source_map: Record<SquareId, PieceId[]>
}
