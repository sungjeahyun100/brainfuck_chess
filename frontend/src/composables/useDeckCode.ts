import type { CustomDeckPieceRef, SavedDeck } from '../types/deck'
import { decodeDeckCode, type DeckCodeDecodeError } from './useDeckCodeCodec.ts'
import {
  emptyPocket,
  findPieceCatalogItem,
  validateLobbyDeck,
} from './useDeckValidation.ts'

export type DeckCodeImportResult =
  | { ok: true; deck: SavedDeck; totalScore: number; scoreLimit: number }
  | { ok: false; message: string }

const decodeErrorMessages: Record<DeckCodeDecodeError, string> = {
  empty: '덱 코드를 입력해 주세요.',
  too_large: '덱 코드가 허용된 최대 길이를 초과했습니다.',
  invalid_format: '올바른 덱 코드 형식이 아닙니다.',
  unsupported_version: '지원하지 않는 덱 코드 버전입니다.',
  invalid_payload: '덱 코드의 데이터가 손상되었거나 올바른 JSON이 아닙니다.',
  invalid_schema: '덱 코드의 데이터 구조가 올바르지 않습니다.',
}

export function importDeckCode(code: string, currentDeck: SavedDeck): DeckCodeImportResult {
  const decoded = decodeDeckCode(code)
  if (!decoded.ok) return { ok: false, message: decodeErrorMessages[decoded.error] }

  const pocket = emptyPocket()
  for (const entry of decoded.value.pocket) pocket[entry.pieceId] = entry.count

  const usedPieceIds = new Set([
    ...decoded.value.starting.map(piece => piece.pieceId),
    ...decoded.value.pocket.map(piece => piece.pieceId),
  ])
  const customPieces: CustomDeckPieceRef[] = []
  for (const pieceId of usedPieceIds) {
    const catalogItem = findPieceCatalogItem(pieceId)
    if (!catalogItem) {
      return { ok: false, message: `존재하지 않거나 현재 사용할 수 없는 기물이 포함되어 있습니다: ${pieceId}` }
    }
    if (catalogItem.custom) {
      customPieces.push({
        id: catalogItem.custom.id,
        version: catalogItem.custom.version,
        contentHash: catalogItem.custom.contentHash,
        exposedPieceKey: catalogItem.custom.exposedPieceKey,
      })
    }
  }

  const candidate: SavedDeck = {
    ...currentDeck,
    mapId: decoded.value.mapId,
    boardSize: decoded.value.boardSize,
    starting: decoded.value.starting.map(piece => ({
      pieceType: piece.pieceId,
      square: { file: piece.file, rank: piece.rank },
    })),
    pocket,
    customPieces,
  }
  const summary = validateLobbyDeck(candidate, candidate.boardSize, currentDeck.name.trim() || '불러온 덱')
  if (!summary.valid) {
    return {
      ok: false,
      message: summary.errors[0] ?? '현재 규칙에서는 사용할 수 없는 덱입니다.',
    }
  }
  return { ok: true, deck: candidate, totalScore: summary.totalScore, scoreLimit: summary.scoreLimit }
}
