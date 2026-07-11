import amazonBlack from './assets/pieces/amazon-black.svg'
import amazonWhite from './assets/pieces/amazon-white.svg'
import bishopBlack from './assets/pieces/bishop-black.svg'
import bishopWhite from './assets/pieces/bishop-white.svg'
import bouncingBishopBlack from './assets/pieces/bouncing-bishop-black.svg'
import bouncingBishopWhite from './assets/pieces/bouncing-bishop-white.svg'
import cannonRookBlack from './assets/pieces/cannon-rook-black.svg'
import cannonRookWhite from './assets/pieces/cannon-rook-white.svg'
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
  'tempest-pawn': { white: tempestPawnWhite, black: tempestPawnBlack },
  'tempest-pawn-white': { white: tempestPawnWhite, black: tempestPawnBlack },
  'tempest-pawn-black': { white: tempestPawnWhite, black: tempestPawnBlack },
  queen: { white: queenWhite, black: queenBlack },
  rook: { white: rookWhite, black: rookBlack },
  // TODO: temporary placeholder art (rook flipped upside-down) to visually distinguish from the regular rook
  'cannon-rook': { white: cannonRookWhite, black: cannonRookBlack },
  'tempest-queen': { white: tempestQueenWhite, black: tempestQueenBlack },
  'tempest-bishop': { white: tempestBishopWhite, black: tempestBishopBlack },
  'tempest-knight': { white: tempestKnightWhite, black: tempestKnightBlack },
  'tempest-rook': { white: tempestRookWhite, black: tempestRookBlack },
  windmill: { white: windmillWhite, black: windmillBlack },
}

export function pieceAsset(typeId: string, owner: PlayerId): string | undefined {
  return PIECE_ASSETS[typeId]?.[owner]
}
