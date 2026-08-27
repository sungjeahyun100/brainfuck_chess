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

test('singleplayer create request sends only side and per-game nicknames as player metadata', async () => {
  let body: Record<string, unknown> = {}
  globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
    body = JSON.parse(String(init?.body)) as Record<string, unknown>
    return new Response(JSON.stringify({ id: 'game', state: {} }), { status: 200, headers: { 'Content-Type': 'application/json' } })
  }) as typeof fetch
  const deck = { name: 'Deck', starting: [], pocket: [] }
  await api.createGame(8, deck, deck, 'standard-8x8', 'ten_five', { localSide: 'black', localNickname: 'Match Name', guestNickname: 'Guest Name' })
  assert.equal(body.local_side, 'black')
  assert.equal(body.local_nickname, 'Match Name')
  assert.equal(body.guest_nickname, 'Guest Name')
  assert.equal('public_id' in body, false)
  assert.equal('user_id' in body, false)
})

test('challenge create request sends a player deck but no opponent or rule settings', async () => {
  let url = ''
  let body: Record<string, unknown> = {}
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    url = String(input)
    body = JSON.parse(String(init?.body)) as Record<string, unknown>
    return new Response(JSON.stringify({ id: 'challenge-game', state: {} }), { status: 200, headers: { 'Content-Type': 'application/json' } })
  }) as typeof fetch
  const playerDeck = { name: 'My Deck', starting: [], pocket: [] }
  await api.createChallengeGame('tempest_horde', playerDeck, 'Player')
  assert.equal(url, '/api/challenges/tempest_horde/games')
  assert.deepEqual(body.player_deck, playerDeck)
  assert.equal(body.local_nickname, 'Player')
  for (const forbidden of ['opponent_deck', 'board_size', 'bot_difficulty', 'challenge_result', 'cleared']) {
    assert.equal(forbidden in body, false)
  }
})
