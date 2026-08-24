import assert from 'node:assert/strict'
import test from 'node:test'

import { api } from './gameApi.ts'
import type { PieceLabOptionsRequest } from './gameApi.ts'
import type { TurnAction } from '../types/game.ts'

const lab: PieceLabOptionsRequest = {
  board_size: 8,
  pieces: [],
  pocket_pieces: [],
  custom_pieces: [],
  selected_piece_id: 'lab-rook',
  global_state: {},
}

test('piece lab action adds the move discriminator omitted by legal-action responses', async () => {
  const calls: Array<{ url: string; init?: RequestInit }> = []
  globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init })
    return new Response(JSON.stringify({}), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    })
  }) as typeof fetch
  const legalMove = {
    player_id: 'white',
    piece_id: 'lab-rook',
    from: { file: 0, rank: 0 },
    to: { file: 0, rank: 1 },
    move_option_id: 'normal',
    source_layer_ids: ['default'],
    effects: {
      global_state_updates: [],
      piece_state_updates: [],
      cooldown_updates: [],
    },
  } as unknown as TurnAction

  await api.applyPieceLabAction(lab, legalMove)

  assert.equal(calls[0]?.url, '/api/lab/apply-action')
  const body = JSON.parse(String(calls[0]?.init?.body))
  assert.equal(body.action.type, 'move')
  assert.equal(body.action.piece_id, 'lab-rook')
})
