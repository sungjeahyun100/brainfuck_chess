import amazonBlack from './assets/pieces/amazon-black.svg'
import amazonWhite from './assets/pieces/amazon-white.svg'
import bishopBlack from './assets/pieces/bishop-black.svg'
import bishopWhite from './assets/pieces/bishop-white.svg'
import bouncingBishopBlack from './assets/pieces/bouncing-bishop-black.svg'
import bouncingBishopWhite from './assets/pieces/bouncing-bishop-white.svg'
import bouncingRookBlack from './assets/pieces/bouncing-rook-black.svg'
import bouncingRookWhite from './assets/pieces/bouncing-rook-white.svg'
import bouncingQueenBlack from './assets/pieces/bouncing-queen-black.svg'
import bouncingQueenWhite from './assets/pieces/bouncing-queen-white.svg'
import cannonRookBlack from './assets/pieces/cannon-rook-black.svg'
import cannonRookWhite from './assets/pieces/cannon-rook-white.svg'
import dozerBlack from './assets/pieces/dozer-black.svg'
import dozerWhite from './assets/pieces/dozer-white.svg'
import kingBlack from './assets/pieces/king-black.svg'
import kingWhite from './assets/pieces/king-white.svg'
import guhangBlack from './assets/pieces/guhang-black.svg'
import guhangWhite from './assets/pieces/guhang-white.svg'
import knightBlack from './assets/pieces/knight-black.svg'
import knightWhite from './assets/pieces/knight-white.svg'
import nightriderBlack from './assets/pieces/nightrider-black.svg'
import nightriderWhite from './assets/pieces/nightrider-white.svg'
import pawnBlack from './assets/pieces/pawn-black.svg'
import pawnWhite from './assets/pieces/pawn-white.svg'
import paratrooperBlack from './assets/pieces/paratrooper-black.svg'
import paratrooperWhite from './assets/pieces/paratrooper-white.svg'
import queenBlack from './assets/pieces/queen-black.svg'
import queenWhite from './assets/pieces/queen-white.svg'
import rookBlack from './assets/pieces/rook-black.svg'
import rookWhite from './assets/pieces/rook-white.svg'
import tempestQueenBlack from './assets/pieces/tempest-queen-black.svg'
import tempestQueenWhite from './assets/pieces/tempest-queen-white.svg'
import tempestBishopBlack from './assets/pieces/tempest-bishop-black.svg'
import tempestBishopWhite from './assets/pieces/tempest-bishop-white.svg'
import tempestKnightBlack from './assets/pieces/tempest-knight-black.svg'
import tempestKnightWhite from './assets/pieces/tempest-knight-white.svg'
import tempestPawnBlack from './assets/pieces/tempest-pawn-black.svg'
import tempestPawnWhite from './assets/pieces/tempest-pawn-white.svg'
import tempestRookBlack from './assets/pieces/tempest-rook-black.svg'
import tempestRookWhite from './assets/pieces/tempest-rook-white.svg'
import windmillBlack from './assets/pieces/windmill-black.svg'
import windmillWhite from './assets/pieces/windmill-white.svg'
import windmillRookBlack from './assets/pieces/windmill-rook-black.svg'
import windmillRookWhite from './assets/pieces/windmill-rook-white.svg'
import { findPieceCatalogItem } from './composables/useDeckValidation'
import type { Piece, PieceDefinition, PlayerId } from './types/game'
import { resolvePieceAssetKey } from './pieceVisual'
export { resolvePieceAssetKey } from './pieceVisual'

const PIECE_ASSETS: Record<string, Record<PlayerId, string>> = {
  amazon: { white: amazonWhite, black: amazonBlack },
  bishop: { white: bishopWhite, black: bishopBlack },
  'bouncing-bishop': { white: bouncingBishopWhite, black: bouncingBishopBlack },
  'bouncing-rook': { white: bouncingRookWhite, black: bouncingRookBlack },
  'bouncing-queen': { white: bouncingQueenWhite, black: bouncingQueenBlack },
  king: { white: kingWhite, black: kingBlack },
  guhang: { white: guhangWhite, black: guhangBlack },
  knight: { white: knightWhite, black: knightBlack },
  nightrider: { white: nightriderWhite, black: nightriderBlack },
  pawn: { white: pawnWhite, black: pawnBlack },
  paratrooper: { white: paratrooperWhite, black: paratrooperBlack },
  'pawn-white': { white: pawnWhite, black: pawnBlack },
  'pawn-black': { white: pawnWhite, black: pawnBlack },
  'tempest-pawn': { white: tempestPawnWhite, black: tempestPawnBlack },
  'tempest-pawn-white': { white: tempestPawnWhite, black: tempestPawnBlack },
  'tempest-pawn-black': { white: tempestPawnWhite, black: tempestPawnBlack },
  queen: { white: queenWhite, black: queenBlack },
  rook: { white: rookWhite, black: rookBlack },
  // TODO: temporary placeholder art (rook flipped upside-down) to visually distinguish from the regular rook
  'cannon-rook': { white: cannonRookWhite, black: cannonRookBlack },
  dozer: { white: dozerWhite, black: dozerBlack },
  'dozer-white': { white: dozerWhite, black: dozerBlack },
  'dozer-black': { white: dozerWhite, black: dozerBlack },
  'tempest-queen': { white: tempestQueenWhite, black: tempestQueenBlack },
  'tempest-bishop': { white: tempestBishopWhite, black: tempestBishopBlack },
  'tempest-knight': { white: tempestKnightWhite, black: tempestKnightBlack },
  'tempest-rook': { white: tempestRookWhite, black: tempestRookBlack },
  windmill: { white: windmillWhite, black: windmillBlack },
  'windmill-bishop': { white: windmillWhite, black: windmillBlack },
  'windmill-rook': { white: windmillRookWhite, black: windmillRookBlack },
}

export function pieceAsset(typeId: string, owner: PlayerId): string | undefined {
  if (typeId.startsWith('data:image/')) return typeId

  const custom = findPieceCatalogItem(typeId)?.custom
  const customAssetKey = custom?.assetKey
    ?? (custom?.image.kind === 'built_in' ? custom.image.asset_key : undefined)
  if (customAssetKey && customAssetKey !== typeId) {
    return pieceAsset(customAssetKey, owner)
  }

  return PIECE_ASSETS[typeId]?.[owner]
}

export function renderedPieceAsset(
  piece: Piece,
  definition: PieceDefinition | undefined,
): string | undefined {
  return pieceAsset(resolvePieceAssetKey(piece, definition), piece.owner)
}
