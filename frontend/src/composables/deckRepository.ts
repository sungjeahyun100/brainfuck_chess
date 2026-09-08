import type { SavedDeck } from '../types/deck.ts'
import { accountDeckApi } from '../api/deckApi.ts'
import { useLocalSavedDecks } from './localDeckRepository.ts'

export interface DeckRepository {
  listDecks(): Promise<SavedDeck[]>
  getDeck(id: string): Promise<SavedDeck | null>
  saveDeck(deck: SavedDeck): Promise<SavedDeck>
  deleteDeck(deck: SavedDeck): Promise<void>
}
export class LocalDeckRepository implements DeckRepository {
  private local = useLocalSavedDecks()
  async listDecks() { return this.local.loadDecks() }
  async getDeck(id: string) { return this.local.getDeck(id) }
  async saveDeck(deck: SavedDeck) {
    // Account revisions must never become guest metadata.
    const { version: _version, ...localDeck } = deck
    this.local.saveDeck(localDeck)
    const saved = this.local.getDeck(deck.id)
    if (!saved) throw new Error('브라우저에 저장된 덱을 확인하지 못했습니다.')
    return saved
  }
  async deleteDeck(deck: SavedDeck) { this.local.deleteDeck(deck.id) }
}
export class AccountDeckRepository implements DeckRepository {
  private api: ReturnType<typeof accountDeckApi>
  constructor(account: string) { this.api = accountDeckApi(account) }
  listDecks() { return this.api.list() }
  getDeck(id: string) { return this.api.get(id) }
  saveDeck(deck: SavedDeck) { return deck.version === undefined ? this.api.create(deck) : this.api.update(deck) }
  deleteDeck(deck: SavedDeck) {
    if (deck.version === undefined) throw new Error('계정 덱 버전을 확인할 수 없습니다. 목록을 다시 불러와 주세요.')
    return this.api.delete(deck)
  }
  importDeck(deck: SavedDeck) { return this.api.import(deck) }
}
