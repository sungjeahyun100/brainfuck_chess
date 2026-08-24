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
} from '../types/game'
import type { PieceCatalogMetadata } from '../types/deck'
import type { GameRecord } from '../types/gameRecord'

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
  starting: DeckPlacementRequest[]
  pocket: DeckPieceRequest[]
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
  ): Promise<{ id: string; state: GameState }> {
    return request(`${BASE}`, {
      method: 'POST',
      body: JSON.stringify({
        board_size: boardSize,
        map_id: mapId,
        white_deck: whiteDeck,
        black_deck: blackDeck,
        time_control: timeControl,
      }),
    })
  },

  getGame(id: string): Promise<GameState> {
    return request(`${BASE}/${id}`)
  },

  getGameRecord(id: string): Promise<GameRecord> {
    return request(`${BASE}/${id}/record`)
  },

  listGameRecords(): Promise<GameRecord[]> {
    return request('/api/game-records')
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
