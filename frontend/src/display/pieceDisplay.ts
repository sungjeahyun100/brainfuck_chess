import type { Piece, PlayerId } from '../types/game'
import { pieceAsset } from '../pieceAssets'

const PIECE_SYMBOLS: Record<string, string> = {
  king: '♔',
  queen: '♕',
  rook: '♖',
  bishop: '♗',
  knight: '♘',

  amazon: 'A',
  'cannon-rook': 'C',
  'tempest-queen': 'Q',
  'tempest-rook': 'T',
  'tempest-knight': 'N',
  'bouncing-bishop': 'B',

  'pawn-white': '♙',
  'pawn-black': '♟',
  'tempest-pawn-white': '♙',
  'tempest-pawn-black': '♟',
}

export function pieceSymbol(typeId: string): string {
  return PIECE_SYMBOLS[typeId] ?? '?'
}

export function pieceImage(typeId: string, owner: PlayerId): string | undefined {
  return pieceAsset(typeId, owner)
}

export function pieceAlt(piece: Piece): string {
  return `${piece.owner} ${piece.type_id}`
}

export function pieceLabel(typeId: string, definitions: Record<string, { name?: string }>): string {
  return definitions[typeId]?.name ?? typeId
}
