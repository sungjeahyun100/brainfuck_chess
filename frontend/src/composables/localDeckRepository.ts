import { normalizeExtra } from './useDeckValidation.ts'
import { parseDeckRuleset } from '../deckRulesets.ts'
import type { SavedDeck } from '../types/deck'
import type { BoardMapId } from '../types/game'
import { findBoardMap, normalizeBoardMapId } from '../boardMaps.ts'
import { createPresetDeck } from './useDeckValidation.ts'

const STORAGE_KEY = 'brainfuck_chess_saved_decks_v1'

function readStorage(): SavedDeck[] {
  if (typeof localStorage === 'undefined') return []

  let parsed: unknown
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    parsed = JSON.parse(raw)
  } catch {
    return []
  }
  if (!Array.isArray(parsed)) return []
  // Keep ruleset errors outside the legacy JSON fallback: never silently discard
  // an unknown format and later overwrite the user's original storage.
  return parsed.filter(isSavedDeck).flatMap(deck => {
    const ruleset = parseDeckRuleset(deck.ruleset)
    const mapId = normalizeBoardMapId(deck.mapId, deck.boardSize)
    if (!mapId) return []
    return [{
      ...deck,
      ruleset,
      extra: normalizeExtra(deck.extra),
      mapId,
      customPieces: Array.isArray(deck.customPieces) ? deck.customPieces : [],
    }]
  })
}

function writeStorage(decks: SavedDeck[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(decks))
}

function isSavedDeck(value: unknown): value is SavedDeck {
  if (!value || typeof value !== 'object') return false
  const deck = value as Partial<SavedDeck>
  return typeof deck.id === 'string'
    && typeof deck.name === 'string'
    && typeof deck.boardSize === 'number'
    && Array.isArray(deck.starting)
    && typeof deck.pocket === 'object'
    && typeof deck.createdAt === 'number'
    && typeof deck.updatedAt === 'number'
}

export function nextId(): string {
  return crypto.randomUUID?.() ?? `${Date.now()}_${Math.random().toString(16).slice(2)}`
}

function createDeckName(existing: SavedDeck[]): string {
  const base = '새 덱'
  let suffix = existing.length + 1
  let name = `${base} ${suffix}`
  while (existing.some(deck => deck.name === name)) {
    suffix += 1
    name = `${base} ${suffix}`
  }
  return name
}

export function createNewSavedDeck(mapId: BoardMapId = 'standard-8x8'): SavedDeck {
  const map = findBoardMap(mapId)
  if (!map) throw new Error(`지원하지 않는 맵입니다: ${mapId}`)
  const now = Date.now()
  const baseDeck = createPresetDeck(map.boardSize)
  const existing = readStorage()

  return {
    ruleset: 'legacy',
    extra: [],
    id: nextId(),
    name: createDeckName(existing),
    mapId: map.id,
    boardSize: map.boardSize,
    starting: baseDeck.starting,
    pocket: baseDeck.pocket,
    createdAt: now,
    updatedAt: now,
    customPieces: [],
  }
}

export function useLocalSavedDecks() {
  function loadDecks(): SavedDeck[] {
    return readStorage().sort((a, b) => b.updatedAt - a.updatedAt)
  }

  function saveDeck(deck: SavedDeck): void {
    const decks = readStorage()
    const now = Date.now()
    const mapId = normalizeBoardMapId(deck.mapId, deck.boardSize)
    if (!mapId) throw new Error('덱의 맵 정보가 올바르지 않습니다.')
    const normalized: SavedDeck = {
      ...deck,
      ruleset: parseDeckRuleset(deck.ruleset),
      extra: normalizeExtra(deck.extra),
      mapId,
      name: deck.name.trim(),
      updatedAt: now,
      createdAt: deck.createdAt || now,
    }
    const index = decks.findIndex(existing => existing.id === deck.id)
    if (index >= 0) {
      decks[index] = normalized
    } else {
      decks.push(normalized)
    }
    writeStorage(decks)
  }

  function deleteDeck(id: string): void {
    writeStorage(readStorage().filter(deck => deck.id !== id))
  }

  function duplicateDeck(id: string): void {
    const decks = readStorage()
    const source = decks.find(deck => deck.id === id)
    if (!source) return
    const now = Date.now()
    decks.push({
      ...source,
      id: nextId(),
      name: `${source.name} 복사본`,
      createdAt: now,
      updatedAt: now,
    })
    writeStorage(decks)
  }

  function renameDeck(id: string, name: string): void {
    const decks = readStorage()
    const deck = decks.find(entry => entry.id === id)
    if (!deck) return
    deck.name = name.trim()
    deck.updatedAt = Date.now()
    writeStorage(decks)
  }

  function getDeck(id: string): SavedDeck | null {
    return readStorage().find(deck => deck.id === id) ?? null
  }

  return {
    loadDecks,
    saveDeck,
    deleteDeck,
    duplicateDeck,
    renameDeck,
    getDeck,
  }
}
