import type { CustomDeckPieceRef, LobbyDeck } from './types/deck'
import type { BoardMapId, PlayerId } from './types/game'
import type { GameRecord } from './types/gameRecord'
import { neutralPieceCatalogId } from './composables/useDeckValidation.ts'

export type FrozenDeckCodeSource = LobbyDeck & { name: string; mapId: BoardMapId; boardSize: number }

export function frozenDeckCodeSource(record: GameRecord, side: PlayerId): FrozenDeckCodeSource | null {
  const deck = record.decks[side]
  if (!deck.map_id || !deck.board_size || !deck.deployments.every(piece => piece.piece_type_id) || !deck.pocket.every(piece => piece.piece_type_id)) return null
  const customPieces = new Map<string, CustomDeckPieceRef>()
  for (const entry of [...deck.deployments, ...deck.pocket]) {
    const pieceId = entry.piece_type_id as string
    if (entry.custom_piece) {
      customPieces.set(pieceId, { id: entry.custom_piece.custom_piece_id, version: entry.custom_piece.version, contentHash: entry.custom_piece.content_hash, exposedPieceKey: entry.custom_piece.exposed_piece_key })
      continue
    }
    const match = /^custom:(.+):v(\d+):(.+)$/u.exec(pieceId)
    const manifest = record.initial_state.custom_piece_manifest.find(item => item.exposed_type_id === pieceId)
    if (match && manifest) customPieces.set(pieceId, { id: match[1], version: Number(match[2]), contentHash: manifest.content_hash, exposedPieceKey: match[3] })
  }
  return {
    name: deck.deck_name, mapId: deck.map_id as BoardMapId, boardSize: deck.board_size,
    starting: deck.deployments.map(piece => ({ pieceType: neutralPieceCatalogId(piece.piece_type_id as string), square: { file: piece.square.file, rank: side === 'black' ? deck.board_size as number - 1 - piece.square.rank : piece.square.rank } })),
    pocket: Object.fromEntries(deck.pocket.map(piece => [neutralPieceCatalogId(piece.piece_type_id as string), piece.count])),
    customPieces: [...customPieces.values()],
  }
}
