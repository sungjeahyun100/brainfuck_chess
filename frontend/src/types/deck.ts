import type { BoardMapId, BotDifficulty, Square, TimeControlId } from './game'
import type { CustomPieceImage } from './customPiece'

export type LobbyPlayer = 'white' | 'black'
export type DeckPieceType = string
export type DeploymentZone = 'front' | 'back'
export type AppView = 'home' | 'deck-library' | 'deck-editor' | 'single-select' | 'bot-select' | 'challenges' | 'multiplayer' | 'piece-lab' | 'custom-piece-workshop' | 'replay-import' | 'game-history'

export interface PieceCatalogItem {
  id: DeckPieceType
  name: string
  score: number
  category: string
  canPocket: boolean
  deploymentZone: DeploymentZone
  uniqueStarting?: boolean
  aliases?: string[]
  custom?: {
    id: string
    version: number
    contentHash: string
    exposedPieceKey: string
    image: CustomPieceImage
    assetKey?: string
    active: boolean
  }
}

export interface PieceCatalogMetadata {
  score: number
  deployment_zone: DeploymentZone
}

export interface LobbyPlacement {
  pieceType: DeckPieceType
  square: Square
}

export interface LobbyDeck {
  starting: LobbyPlacement[]
  pocket: Record<DeckPieceType, number>
  customPieces?: CustomDeckPieceRef[]
}

export interface SavedDeck extends LobbyDeck {
  id: string
  name: string
  mapId: BoardMapId
  boardSize: number
  createdAt: number
  updatedAt: number
  customPieces: CustomDeckPieceRef[]
}

export interface CustomDeckPieceRef {
  id: string
  version: number
  contentHash: string
  exposedPieceKey: string
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
  localDeckId: string
  opponentDeckId: string
  localSide: LobbyPlayer | 'random'
  localNickname: string
  guestNickname: string
  timeControl: TimeControlId
}

export interface BotDeckSelection {
  humanSide: LobbyPlayer
  humanDeckId: string
  botDeckId: string
  difficulty: BotDifficulty
  timeControl: TimeControlId
}
