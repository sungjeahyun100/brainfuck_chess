import amazonBlack from './assets/pieces/amazon-black.svg'
import amazonWhite from './assets/pieces/amazon-white.svg'
import bishopBlack from './assets/pieces/bishop-black.svg'
import bishopWhite from './assets/pieces/bishop-white.svg'
import bouncingBishopBlack from './assets/pieces/bouncing-bishop-black.svg'
import bouncingBishopWhite from './assets/pieces/bouncing-bishop-white.svg'
import kingBlack from './assets/pieces/king-black.svg'
import kingWhite from './assets/pieces/king-white.svg'
import knightBlack from './assets/pieces/knight-black.svg'
import knightWhite from './assets/pieces/knight-white.svg'
import pawnBlack from './assets/pieces/pawn-black.svg'
import pawnWhite from './assets/pieces/pawn-white.svg'
import queenBlack from './assets/pieces/queen-black.svg'
import queenWhite from './assets/pieces/queen-white.svg'
import rookBlack from './assets/pieces/rook-black.svg'
import rookWhite from './assets/pieces/rook-white.svg'
import tempestRookBlack from './assets/pieces/tempest-rook-black.svg'
import tempestRookWhite from './assets/pieces/tempest-rook-white.svg'
import type { PlayerId } from './types/game'

const PIECE_ASSETS: Record<string, Record<PlayerId, string>> = {
  amazon: { white: amazonWhite, black: amazonBlack },
  bishop: { white: bishopWhite, black: bishopBlack },
  'bouncing-bishop': { white: bouncingBishopWhite, black: bouncingBishopBlack },
  king: { white: kingWhite, black: kingBlack },
  knight: { white: knightWhite, black: knightBlack },
  pawn: { white: pawnWhite, black: pawnBlack },
  'pawn-white': { white: pawnWhite, black: pawnBlack },
  'pawn-black': { white: pawnWhite, black: pawnBlack },
  queen: { white: queenWhite, black: queenBlack },
  rook: { white: rookWhite, black: rookBlack },
  'tempest-rook': { white: tempestRookWhite, black: tempestRookBlack },
}

export function pieceAsset(typeId: string, owner: PlayerId): string | undefined {
  return PIECE_ASSETS[typeId]?.[owner]
}
