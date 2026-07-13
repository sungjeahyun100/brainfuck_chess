import assert from 'node:assert/strict'
import test from 'node:test'

import { resolvePieceAssetKey } from './pieceVisual.ts'
import type { Piece, PieceDefinition } from './types/game.ts'

function windmill(mode: string): Piece {
  return {
    id: 'windmill-1',
    owner: 'white',
    type_id: 'windmill',
    current_square: { file: 3, rank: 3 },
    in_pocket: false,
    captured: false,
    has_moved: false,
    state: { mode },
    move_option_cooldowns: {},
  }
}

const definition = {
  id: 'windmill',
  name: 'Windmill',
  score: 4,
  chessembly_code: '',
  chessembly_version: '1.0',
  is_king: false,
  state_schema: [{ key: 'mode', default_value: 'bishop' }],
  move_layers: [],
  move_options: [],
  visual: {
    default_asset_key: 'windmill-bishop',
    variants: [
      {
        id: 'rook-low',
        enabled_when: [{ key: 'mode', condition: { equals: 'rook' } }],
        asset_key: 'windmill-rook-low',
        priority: 1,
      },
      {
        id: 'rook-high',
        enabled_when: [{ key: 'mode', condition: { equals: 'rook' } }],
        asset_key: 'windmill-rook',
        priority: 10,
      },
    ],
  },
} satisfies PieceDefinition

test('visual variants resolve from current piece state and priority', () => {
  const piece = windmill('bishop')
  assert.equal(resolvePieceAssetKey(piece, definition), 'windmill-bishop')

  piece.state.mode = 'rook'
  assert.equal(resolvePieceAssetKey(piece, definition), 'windmill-rook')
})

test('visual resolver falls back to default and then type id', () => {
  assert.equal(resolvePieceAssetKey(windmill('unknown'), definition), 'windmill-bishop')
  assert.equal(resolvePieceAssetKey(windmill('rook'), undefined), 'windmill')
})
