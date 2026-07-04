import type { Square } from '../types/game'
import type { LobbyDeck, LobbyPlayer, SavedDeck } from '../types/deck'
import type { PlayerDeckRequest } from '../api/gameApi'
import { pocketCatalog } from './useDeckValidation'

function mirrorSquare(square: Square, boardSize: number): Square {
  return {
    file: square.file,
    rank: boardSize - 1 - square.rank,
  }
}

function serializeDeck(deck: LobbyDeck): PlayerDeckRequest {
  return {
    starting: deck.starting.map(piece => ({
      piece_type: piece.pieceType,
      square: piece.square,
    })),
    pocket: pocketCatalog.flatMap(piece => Array.from({ length: deck.pocket[piece.id] ?? 0 }, () => piece.id)),
  }
}

export function savedDeckToPlayerDeckRequest(deck: SavedDeck): PlayerDeckRequest {
  return serializeDeck(deck)
}

export function serializeNeutralDeck(deck: SavedDeck, side: LobbyPlayer): PlayerDeckRequest {
  if (side === 'white') {
    return savedDeckToPlayerDeckRequest(deck)
  }

  return {
    starting: deck.starting.map(piece => ({
      piece_type: piece.pieceType,
      square: mirrorSquare(piece.square, deck.boardSize),
    })),
    pocket: pocketCatalog.flatMap(piece => Array.from({ length: deck.pocket[piece.id] ?? 0 }, () => piece.id)),
  }
}
