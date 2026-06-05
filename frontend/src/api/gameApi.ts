import type { GameState, MoveAction, DropAction, TurnAction } from '../types/game'

const BASE = '/games'

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

export const api = {
  createGame(boardSize: number): Promise<{ id: string; state: GameState }> {
    return request(`${BASE}`, {
      method: 'POST',
      body: JSON.stringify({ board_size: boardSize }),
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

  endTurn(id: string): Promise<GameState> {
    return request(`${BASE}/${id}/end-turn`, { method: 'POST' })
  },

  getLegalMoves(id: string): Promise<{ moves: MoveAction[] }> {
    return request(`${BASE}/${id}/legal-moves`)
  },

  getLegalDrops(id: string): Promise<{ drops: DropAction[] }> {
    return request(`${BASE}/${id}/legal-drops`)
  },
}
