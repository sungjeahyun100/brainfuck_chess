import type {
  BotDifficulty,
  BoardVariant,
  BoardMapId,
  BotTurnResponse,
  DropAction,
  GameState,
  MoveAction,
  MoveOptionExecutionMode,
  MoveOptionKind,
  PieceDefinition,
  PieceStateValue,
  PlayerId,
  Square,
  SubmitAction,
  TimeControlId,
  TurnAction,
} from '../types/game'
import type { PieceCatalogMetadata } from '../types/deck'
import type { AnalysisActionPreview, AnalysisAppendResult, AnalysisTree, GameRecord } from '../types/gameRecord'
import { actionIdentity } from '../replayAnalysis.ts'

const BASE = '/api/games'
const ROOM_BASE = '/api/rooms'
const CLIENT_ID_KEY = 'brainfuck_chess_tab_client_id'

export interface CustomDeckPieceRequest {
  custom_piece_id: string
  version: number
  content_hash: string
  exposed_piece_key: string
}

export type DeckPieceRequest = { piece_type: string } | CustomDeckPieceRequest

export interface DeckPlacementRequest {
  square: {
    file: number
    rank: number
  }
  piece_type?: string
  custom_piece_id?: string
  version?: number
  content_hash?: string
  exposed_piece_key?: string
}

export interface PlayerDeckRequest {
  name?: string
  starting: DeckPlacementRequest[]
  pocket: DeckPieceRequest[]
}

export interface ChallengeSummary {
  id: string
  name: string
  description: string
  board_size: number
  map_id: BoardMapId
  bot_difficulty: BotDifficulty
  time_control: TimeControlId
  cleared: boolean
}

export interface MultiplayerRoom {
  id: string
  board_size: number
  map_id: BoardMapId
  board_variant: BoardVariant
  host_side: 'white' | 'black'
  guest_side: 'white' | 'black'
  host_deck?: PlayerDeckRequest | null
  guest_deck?: PlayerDeckRequest | null
  host_ready: boolean
  guest_ready: boolean
  game_id?: string | null
  time_control: TimeControlId
}

interface ResignRoomRequest {
  client_id: string
  player_id: PlayerId
}

interface ResignGameRequest {
  player_id: PlayerId
}

export interface PieceOptionsResponse {
  moves: MoveAction[]
  attacks: Square[]
  ability_actions: import('../types/game').AbilityAction[]
}

export interface PieceLabPieceRequest {
  id: string
  piece_type: string
  owner: PlayerId
  square: Square
  state?: Record<string, PieceStateValue>
  move_option_cooldowns?: Record<string, { remaining: number }>
  current_ammo?: number
  layer?: 'ground' | 'air'
  remaining_flight_turns?: number
}

export interface PieceLabPocketPieceRequest {
  id: string
  piece_type: string
  owner: PlayerId
  state?: Record<string, PieceStateValue>
  current_ammo?: number
}

export interface PieceLabOptionsRequest {
  board_size: number
  pieces: PieceLabPieceRequest[]
  pocket_pieces: PieceLabPocketPieceRequest[]
  custom_pieces?: CustomDeckPieceRequest[]
  selected_piece_id: string
  move_option_id?: string
  global_state?: Record<string, number>
}

export interface PieceLabMoveOption {
  id: string
  name: string
  description: string
  available: boolean
  kind: MoveOptionKind
  execution_mode: MoveOptionExecutionMode
  cooldown_remaining: number
}

export interface PieceLabOptionsResponse {
  moves: Square[]
  legal_moves: MoveAction[]
  legal_drops: DropAction[]
  legal_ability_actions: import('../types/game').AbilityAction[]
  attacks: Square[]
  move_options: PieceLabMoveOption[]
  piece_definitions: Record<string, PieceDefinition>
  piece_states: Record<string, Record<string, PieceStateValue>>
  piece_cooldowns: Record<string, Record<string, { remaining: number }>>
  piece_runtime: Record<string, import('../types/game').Piece>
}

async function request<T>(url: string, options?: RequestInit): Promise<T> {
  const fetchRequest = () => fetch(url, {
    credentials: 'same-origin',
    headers: {
      'Content-Type': 'application/json',
    },
    ...options,
  })
  let res = await fetchRequest()
  if (res.status === 401) {
    const session = await fetch('/api/auth/session', {
      method: 'POST',
      credentials: 'same-origin',
    })
    if (session.ok) res = await fetchRequest()
  }
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(err.error ?? res.statusText)
  }
  if (res.status === 204) return undefined as T
  return res.json()
}

export function withTurnActionType(action: import('../types/game').TurnAction): import('../types/game').TurnAction {
  const type = 'ability_id' in action ? 'ability' : 'from' in action ? 'move' : 'drop'
  return { ...action, type } as import('../types/game').TurnAction
}

function getClientId(): string {
  const existing = sessionStorage.getItem(CLIENT_ID_KEY)
  if (existing) return existing

  const next = crypto.randomUUID?.() ?? `${Date.now()}_${Math.random().toString(16).slice(2)}`
  sessionStorage.setItem(CLIENT_ID_KEY, next)
  return next
}

export const api = {
  listChallenges(): Promise<ChallengeSummary[]> {
    return request('/api/challenges')
  },

  createChallengeGame(
    challengeId: string,
    playerDeck: PlayerDeckRequest,
    localNickname?: string,
  ): Promise<{ id: string; state: GameState }> {
    return request(`/api/challenges/${encodeURIComponent(challengeId)}/games`, {
      method: 'POST',
      body: JSON.stringify({ player_deck: playerDeck, local_nickname: localNickname }),
    })
  },

  getPieceScores(): Promise<Record<string, number>> {
    return request('/api/piece-scores')
  },

  getPieceCatalog(): Promise<Record<string, PieceCatalogMetadata>> {
    return request('/api/piece-catalog')
  },

  createGame(
    boardSize: number,
    whiteDeck: PlayerDeckRequest,
    blackDeck: PlayerDeckRequest,
    mapId: BoardMapId,
    timeControl: TimeControlId,
    player?: { localSide: PlayerId; localNickname?: string; guestNickname: string },
  ): Promise<{ id: string; state: GameState }> {
    return request(`${BASE}`, {
      method: 'POST',
      body: JSON.stringify({
        board_size: boardSize,
        map_id: mapId,
        white_deck: whiteDeck,
        black_deck: blackDeck,
        time_control: timeControl,
        ...(player ? { local_side: player.localSide, local_nickname: player.localNickname, guest_nickname: player.guestNickname } : {}),
      }),
    })
  },

  getGame(id: string): Promise<GameState> {
    return request(`${BASE}/${id}`)
  },

  getGameRecord(id: string): Promise<GameRecord> {
    return request(`${BASE}/${id}/record`)
  },

  listGameRecords(): Promise<import('../types/gameRecord').GameRecordSummary[]> {
    return request('/api/game-records')
  },

  updateGameRetention(id: string, permanent: boolean): Promise<GameRecord> {
    return request(`${BASE}/${id}/retention`, { method: 'PATCH', body: JSON.stringify({ permanent }) })
  },

  listAnalysis(id: string): Promise<AnalysisTree[]> {
    return request(`${BASE}/${id}/analysis`)
  },

  getAnalysisOptions(id: string, position: { base_ply: number; tree_id?: string; node_id?: string; pending_actions?: TurnAction[] }, pieceId: string, moveOptionId?: string): Promise<{ moves: MoveAction[]; drops: DropAction[]; ability_actions: import('../types/game').AbilityAction[]; previews: AnalysisActionPreview[] }> {
    return request(`${BASE}/${id}/analysis/options`, { method: 'POST', body: JSON.stringify({ ...position, piece_id: pieceId, move_option_id: moveOptionId }) })
  },

  createAnalysis(id: string, basePly: number, action: TurnAction, name?: string): Promise<AnalysisTree> {
    return request(`${BASE}/${id}/analysis`, { method: 'POST', body: JSON.stringify({ base_ply: basePly, action: withTurnActionType(action), name, request_id: crypto.randomUUID() }) })
  },

  appendAnalysis(id: string, tree: AnalysisTree, parentNodeId: string, action: TurnAction): Promise<AnalysisAppendResult> {
    return request<AnalysisAppendResult | AnalysisTree>(`${BASE}/${id}/analysis/${tree.id}/nodes`, { method: 'POST', body: JSON.stringify({ parent_node_id: parentNodeId, action: withTurnActionType(action), expected_version: tree.version, request_id: crypto.randomUUID() }) }).then(response => {
      if ('node' in response) return response
      const node = [...response.nodes].reverse().find(candidate => candidate.parent_node_id === parentNodeId && actionIdentity(candidate.action) === actionIdentity(action))
      if (!node) throw new Error('저장된 분석 노드를 확인할 수 없습니다.')
      return { node, version: response.version, updated_at_ms: response.updated_at_ms }
    })
  },

  renameAnalysis(id: string, tree: AnalysisTree, name: string): Promise<AnalysisTree> {
    return request(`${BASE}/${id}/analysis/${tree.id}`, { method: 'PATCH', body: JSON.stringify({ name, expected_version: tree.version }) })
  },

  deleteAnalysis(id: string, treeId: string): Promise<void> {
    return request(`${BASE}/${id}/analysis/${treeId}`, { method: 'DELETE' })
  },

  deleteAnalysisSubtree(id: string, tree: AnalysisTree, nodeId: string): Promise<AnalysisTree> {
    return request(`${BASE}/${id}/analysis/${tree.id}/nodes/${nodeId}`, { method: 'DELETE', body: JSON.stringify({ expected_version: tree.version }) })
  },

  submitAction(id: string, action: SubmitAction): Promise<GameState> {
    return request(`${BASE}/${id}/actions`, {
      method: 'POST',
      body: JSON.stringify({ action }),
    })
  },

  botTurn(id: string, botPlayerId: PlayerId, difficulty: BotDifficulty): Promise<BotTurnResponse> {
    return request(`${BASE}/${id}/bot-turn`, {
      method: 'POST',
      body: JSON.stringify({
        bot_player_id: botPlayerId,
        difficulty,
      }),
    })
  },

  resignGame(id: string, playerId: PlayerId): Promise<GameState> {
    const body: ResignGameRequest = { player_id: playerId }
    return request(`${BASE}/${id}/resign`, {
      method: 'POST',
      body: JSON.stringify(body),
    })
  },

  getLegalMoves(id: string): Promise<{ moves: MoveAction[] }> {
    return request(`${BASE}/${id}/legal-moves`)
  },

  getPieceAttacks(id: string, pieceId: string): Promise<{ squares: Square[] }> {
    return request(`${BASE}/${id}/piece-attacks/${pieceId}`)
  },

  getPlayerAttacks(id: string, playerId: PlayerId): Promise<{ squares: Square[] }> {
    return request(`${BASE}/${id}/players/${encodeURIComponent(playerId)}/attacks`)
  },

  getPieceOptions(id: string, pieceId: string, moveOptionId?: string | null): Promise<PieceOptionsResponse> {
    const query = moveOptionId ? `?move_option_id=${encodeURIComponent(moveOptionId)}` : ''
    return request(`${BASE}/${id}/pieces/${pieceId}/options${query}`)
  },

  getPieceLabOptions(payload: PieceLabOptionsRequest): Promise<PieceLabOptionsResponse> {
    return request('/api/lab/piece-options', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },

  applyPieceLabAction(payload: PieceLabOptionsRequest, action: import('../types/game').TurnAction): Promise<GameState> {
    return request('/api/lab/apply-action', {
      method: 'POST',
      body: JSON.stringify({ lab: payload, action: withTurnActionType(action) }),
    })
  },

  getLegalDrops(id: string): Promise<{ drops: DropAction[] }> {
    return request(`${BASE}/${id}/legal-drops`)
  },

  createRoom(
    boardSize: number,
    hostSide: 'white' | 'black',
    deck: PlayerDeckRequest,
    mapId: BoardMapId,
    timeControl: TimeControlId,
  ): Promise<MultiplayerRoom> {
    return request(`${ROOM_BASE}`, {
      method: 'POST',
      body: JSON.stringify({
        board_size: boardSize,
        map_id: mapId,
        host_side: hostSide,
        client_id: getClientId(),
        deck,
        time_control: timeControl,
      }),
    })
  },

  heartbeatRoom(id: string, playerId: PlayerId): Promise<GameState> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}/heartbeat`, {
      method: 'POST',
      body: JSON.stringify({ client_id: getClientId(), player_id: playerId }),
    })
  },

  getRoom(id: string): Promise<MultiplayerRoom> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}`)
  },

  joinRoom(id: string, deck: PlayerDeckRequest): Promise<{ id: string; state: GameState }> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}/join`, {
      method: 'POST',
      body: JSON.stringify({
        client_id: getClientId(),
        deck,
      }),
    })
  },

  selectRoomDeck(id: string, deck: PlayerDeckRequest): Promise<MultiplayerRoom> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}/select-deck`, {
      method: 'POST',
      body: JSON.stringify({
        client_id: getClientId(),
        deck,
      }),
    })
  },

  readyRoom(id: string): Promise<MultiplayerRoom> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}/ready`, {
      method: 'POST',
      body: JSON.stringify({
        client_id: getClientId(),
      }),
    })
  },

  unreadyRoom(id: string): Promise<MultiplayerRoom> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}/unready`, {
      method: 'POST',
      body: JSON.stringify({
        client_id: getClientId(),
      }),
    })
  },

  resignRoom(id: string, playerId: PlayerId): Promise<GameState> {
    return request(`${ROOM_BASE}/${encodeURIComponent(id)}/resign`, {
      method: 'POST',
      body: JSON.stringify({
        client_id: getClientId(),
        player_id: playerId,
      }),
    })
  },

  sendResignBeacon(id: string, playerId: PlayerId): boolean {
    const url = `${ROOM_BASE}/${encodeURIComponent(id)}/resign`
    const body: ResignRoomRequest = {
      client_id: getClientId(),
      player_id: playerId,
    }
    const payload = JSON.stringify(body)

    if (navigator.sendBeacon) {
      return navigator.sendBeacon(url, new Blob([payload], { type: 'application/json' }))
    }

    fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: payload,
      keepalive: true,
    }).catch(() => undefined)
    return true
  },
}
