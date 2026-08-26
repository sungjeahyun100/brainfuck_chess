import type { CustomDeckPieceRef, LobbyDeck } from './types/deck'
import type { BoardMapId, PlayerId } from './types/game'
import type { GameRecord } from './types/gameRecord'
import { neutralPieceCatalogId } from './composables/useDeckValidation.ts'
import { findBoardMap } from './boardMaps.ts'

export type FrozenDeckCodeSource = LobbyDeck & { name: string; mapId: BoardMapId; boardSize: number }

const CUSTOM_PIECE_ID = /^custom:(.+):v(\d+):(.+)$/u
const CONTENT_HASH = /^[A-Za-z0-9_-]{1,256}$/u

function validIdentityPart(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 256
}

function validSquare(value: unknown, boardSize: number): value is { file: number; rank: number } {
  if (value === null || typeof value !== 'object') return false
  const square = value as { file?: unknown; rank?: unknown }
  return Number.isInteger(square.file) && Number.isInteger(square.rank)
    && (square.file as number) >= 0 && (square.file as number) < boardSize
    && (square.rank as number) >= 0 && (square.rank as number) < boardSize
}

function customPieceRef(
  snapshot: { custom_piece_id: string; version: number; content_hash: string; exposed_piece_key: string },
): CustomDeckPieceRef | null {
  if (!validIdentityPart(snapshot.custom_piece_id)
    || !Number.isInteger(snapshot.version) || snapshot.version <= 0
    || typeof snapshot.content_hash !== 'string' || !CONTENT_HASH.test(snapshot.content_hash)
    || !validIdentityPart(snapshot.exposed_piece_key)) return null
  return {
    id: snapshot.custom_piece_id,
    version: snapshot.version,
    contentHash: snapshot.content_hash,
    exposedPieceKey: snapshot.exposed_piece_key,
  }
}

export function frozenDeckCodeSource(record: GameRecord, side: PlayerId): FrozenDeckCodeSource | null {
  const deck = record.decks?.[side]
  if (!deck || !Number.isInteger(deck.board_size) || typeof deck.deck_name !== 'string'
    || !Array.isArray(deck.deployments) || !Array.isArray(deck.pocket)) return null

  const boardSize = deck.board_size as number
  const map = findBoardMap(deck.map_id)
  if (!map || map.boardSize !== boardSize
    || !deck.deployments.every(piece => validIdentityPart(piece?.piece_type_id) && validSquare(piece?.square, boardSize))
    || !deck.pocket.every(piece => validIdentityPart(piece?.piece_type_id) && Number.isInteger(piece?.count) && piece.count > 0)) return null

  const manifest = Array.isArray(record.initial_state?.custom_piece_manifest)
    ? record.initial_state.custom_piece_manifest
    : []
  const customPieces = new Map<string, CustomDeckPieceRef>()
  const canonicalCustomIds = new Map<string, string>()
  for (const entry of [...deck.deployments, ...deck.pocket]) {
    const pieceId = entry.piece_type_id as string
    if (entry.custom_piece) {
      const identity = customPieceRef(entry.custom_piece)
      if (!identity) return null
      const canonicalId = `custom:${identity.id}:v${identity.version}:${identity.exposedPieceKey}`
      const existingId = canonicalCustomIds.get(pieceId)
      if (existingId && existingId !== canonicalId) return null
      canonicalCustomIds.set(pieceId, canonicalId)
      customPieces.set(canonicalId, identity)
      continue
    }
    const match = CUSTOM_PIECE_ID.exec(pieceId)
    if (!match) continue
    const manifestEntry = manifest.find(item => item?.exposed_type_id === pieceId)
    if (!manifestEntry) return null
    const identity = customPieceRef({
      custom_piece_id: match[1],
      version: Number(match[2]),
      content_hash: manifestEntry.content_hash,
      exposed_piece_key: match[3],
    })
    if (!identity) return null
    canonicalCustomIds.set(pieceId, pieceId)
    customPieces.set(pieceId, identity)
  }
  const deckCodePieceId = (pieceId: string) => canonicalCustomIds.get(pieceId) ?? neutralPieceCatalogId(pieceId)
  return {
    name: deck.deck_name, mapId: map.id, boardSize,
    starting: deck.deployments.map(piece => ({ pieceType: deckCodePieceId(piece.piece_type_id as string), square: { file: piece.square.file, rank: side === 'black' ? boardSize - 1 - piece.square.rank : piece.square.rank } })),
    pocket: Object.fromEntries(deck.pocket.map(piece => [deckCodePieceId(piece.piece_type_id as string), piece.count])),
    customPieces: [...customPieces.values()],
  }
}
