import type { BotDifficulty, Square } from './game'

export type LobbyPlayer = 'white' | 'black'
export type DeckPieceType = string
export type AppView = 'home' | 'deck-library' | 'deck-editor' | 'single-select' | 'bot-select' | 'multiplayer' | 'piece-lab'

export interface PieceCatalogItem {
  id: DeckPieceType
  name: string
  score: number
  category: string
  canPocket: boolean
  uniqueStarting?: boolean
  aliases?: string[]
}

export interface LobbyPlacement {
  pieceType: DeckPieceType
  square: Square
}

export interface LobbyDeck {
  starting: LobbyPlacement[]
  pocket: Record<DeckPieceType, number>
}

export interface SavedDeck extends LobbyDeck {
  id: string
  name: string
  boardSize: number
  createdAt: number
  updatedAt: number
}

export interface DeckSummary {
  totalScore: number
  scoreLimit: number
  valid: boolean
  errors: string[]
}

export interface DeckPresetLayout {
  backline: (DeckPieceType | null)[]
  pawns: (DeckPieceType | null)[]
  pocket: Partial<Record<DeckPieceType, number>>
}

export interface DeckPreset {
  id: string
  name: string
  description: string
  layouts: Record<number, DeckPresetLayout>
}

export type DeckSelectMode = 'single' | 'bot'

export interface SingleDeckSelection {
  whiteDeckId: string
  blackDeckId: string
}

export interface BotDeckSelection {
  humanSide: LobbyPlayer
  humanDeckId: string
  botDeckId: string
  difficulty: BotDifficulty
}
