import type { SavedDeck } from '../types/deck.ts'

export class DeckApiError extends Error {
  readonly status: number
  readonly code: string
  constructor(message: string, status: number, code: string) {
    super(message)
    this.status = status
    this.code = code
  }
}

// Whitelist the existing content shape. Never transmit owner, timestamps or local metadata.
export function deckInput(deck: SavedDeck) {
  return {
    name: deck.name,
    deckData: {
      mapId: deck.mapId,
      boardSize: deck.boardSize,
      starting: deck.starting,
      pocket: deck.pocket,
      customPieces: deck.customPieces ?? [],
    },
  }
}

export function accountDeckApi(account: string) {
  async function request<T>(path = '', method = 'GET', body?: unknown): Promise<T> {
    const response = await fetch(`/api/decks${path}`, {
      method,
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'Content-Type': 'application/json', 'X-Deck-Account': account },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    })
    if (!response.ok) {
      const data = await response.json().catch(() => ({})) as { error?: string; code?: string }
      throw new DeckApiError(data.error ?? '계정 덱 요청에 실패했습니다.', response.status, data.code ?? 'deck_request_failed')
    }
    if (response.status === 204) return undefined as T
    return response.json() as Promise<T>
  }
  return {
    list: () => request<{ items: SavedDeck[] }>().then(result => result.items),
    get: (id: string) => request<SavedDeck>(`/${encodeURIComponent(id)}`),
    create: (deck: SavedDeck) => request<SavedDeck>('', 'POST', { requestId: deck.id, ...deckInput(deck) }),
    import: (deck: SavedDeck) => request<SavedDeck>('/import', 'POST', deckInput(deck)),
    update: (deck: SavedDeck) => request<SavedDeck>(`/${encodeURIComponent(deck.id)}`, 'PUT', { expectedVersion: deck.version, ...deckInput(deck) }),
    delete: (deck: SavedDeck) => request<void>(`/${encodeURIComponent(deck.id)}`, 'DELETE', { expectedVersion: deck.version }),
  }
}
