import type { Square } from '../types/game'
import type { LobbyDeck, LobbyPlayer, SavedDeck } from '../types/deck'
import type { DeckPieceRequest, PlayerDeckRequest } from '../api/gameApi'

function serializePiece(deck: LobbyDeck, pieceType: string): DeckPieceRequest {
  const custom = deck.customPieces
    ?.find(piece => `custom:${piece.id}:v${piece.version}:${piece.exposedPieceKey}` === pieceType)
  if (!custom) return { piece_type: pieceType }
  return {
    custom_piece_id: custom.id,
    version: custom.version,
    content_hash: custom.contentHash,
    exposed_piece_key: custom.exposedPieceKey,
  }
}

function mirrorSquare(square: Square, boardSize: number): Square {
  return {
    file: square.file,
    rank: boardSize - 1 - square.rank,
  }
}

function serializeDeck(deck: LobbyDeck): PlayerDeckRequest {
  return {
    starting: deck.starting.map(piece => ({
      ...serializePiece(deck, piece.pieceType),
      square: piece.square,
    })),
    pocket: Object.entries(deck.pocket)
      .flatMap(([pieceType, count]) => Array.from({ length: count }, () => serializePiece(deck, pieceType))),
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
      ...serializePiece(deck, piece.pieceType),
      square: mirrorSquare(piece.square, deck.boardSize),
    })),
    pocket: Object.entries(deck.pocket)
      .flatMap(([pieceType, count]) => Array.from({ length: count }, () => serializePiece(deck, pieceType))),
  }
}
