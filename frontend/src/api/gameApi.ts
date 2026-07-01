import type {
  BotDifficulty,
  BotTurnResponse,
  DropAction,
  GameState,
  MoveAction,
  PlayerId,
  Square,
  TurnAction,
} from '../types/game'

const BASE = '/api/games'
const ROOM_BASE = '/api/rooms'
const CLIENT_ID_KEY = 'brainfuck_chess_tab_client_id'

export interface DeckPlacementRequest {
  piece_type: string
  square: {
    file: number
    rank: number
  }
}

export interface PlayerDeckRequest {
  starting: DeckPlacementRequest[]
  pocket: string[]
}

export interface MultiplayerRoom {
  id: string
  board_size: number
  host_side: 'white' | 'black'
  guest_side: 'white' | 'black'
  host_deck: PlayerDeckRequest
  guest_deck?: PlayerDeckRequest | null
  game_id?: string | null
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
}

async function request<T>(url: string, options?: RequestInit): Promise<T> {
  const res = await fetch(url, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(err.error ?? res.statusText)
  }
  return res.json()
}

function getClientId(): string {
  const existing = sessionStorage.getItem(CLIENT_ID_KEY)
  if (existing) return existing

  const next = crypto.randomUUID?.() ?? `${Date.now()}_${Math.random().toString(16).slice(2)}`
  sessionStorage.setItem(CLIENT_ID_KEY, next)
  return next
}

export const api = {
  createGame(
    boardSize: number,
    whiteDeck: PlayerDeckRequest,
    blackDeck: PlayerDeckRequest,
  ): Promise<{ id: string; state: GameState }> {
    return request(`${BASE}`, {
      method: 'POST',
      body: JSON.stringify({
        board_size: boardSize,
        white_deck: whiteDeck,
        black_deck: blackDeck,
      }),
    })
  },

  getGame(id: string): Promise<GameState> {
    return request(`${BASE}/${id}`)
  },

  submitAction(id: string, action: TurnAction): Promise<GameState> {
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

  getPieceOptions(id: string, pieceId: string): Promise<PieceOptionsResponse> {
    return request(`${BASE}/${id}/pieces/${pieceId}/options`)
  },

  getLegalDrops(id: string): Promise<{ drops: DropAction[] }> {
    return request(`${BASE}/${id}/legal-drops`)
  },

  createRoom(
    boardSize: number,
    hostSide: 'white' | 'black',
    deck: PlayerDeckRequest,
  ): Promise<MultiplayerRoom> {
    return request(`${ROOM_BASE}`, {
      method: 'POST',
      body: JSON.stringify({
        board_size: boardSize,
        host_side: hostSide,
        client_id: getClientId(),
        deck,
      }),
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
