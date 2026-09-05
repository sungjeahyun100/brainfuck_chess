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

test('analysis writes send canonical actions, parent identity, version, and no client state', async () => {
  const calls: Array<{ url: string; body: Record<string, unknown> }> = []
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(input), body: JSON.parse(String(init?.body ?? '{}')) })
    const response = calls.length === 1
      ? { id: 'tree', game_id: 'game', name: 'V', base_ply: 3, version: 2, nodes: [] }
      : { node: { id: 'child', parent_node_id: 'parent', action, state_after: {}, state_hash: 'hash', created_at_ms: 1 }, version: 3, updated_at_ms: 1 }
    return new Response(JSON.stringify(response), { status: 200, headers: { 'Content-Type': 'application/json' } })
  }) as typeof fetch
  const action = {
    type: 'move', player_id: 'white', piece_id: 'rook', from: { file: 0, rank: 0 }, to: { file: 0, rank: 1 }, move_option_id: 'normal', source_layer_ids: [],
    effects: { global_state_updates: [], piece_state_updates: [], cooldown_updates: [] },
  } as TurnAction
  await api.createAnalysis('game', 3, action)
  await api.appendAnalysis('game', { id: 'tree', game_id: 'game', name: 'V', base_ply: 3, version: 2, created_at_ms: 0, updated_at_ms: 0, nodes: [] }, 'parent', action)
  assert.equal(calls[0]?.url, '/api/games/game/analysis')
  assert.equal(calls[0]?.body.base_ply, 3)
  assert.equal('state_after' in calls[0]!.body, false)
  assert.equal(calls[1]?.body.parent_node_id, 'parent')
  assert.equal(calls[1]?.body.expected_version, 2)
})

test('analysis append accepts the previous whole-tree response during rolling deploys', async () => {
  const action = {
    type: 'drop', player_id: 'white', piece_id: 'reserve', to: { file: 2, rank: 2 },
  } as TurnAction
  globalThis.fetch = (async () => new Response(JSON.stringify({
    id: 'tree', game_id: 'game', name: 'V', base_ply: 3, version: 4,
    created_at_ms: 0, updated_at_ms: 9,
    nodes: [{ id: 'new-node', parent_node_id: 'parent', action: { to: { rank: 2, file: 2 }, piece_id: 'reserve', type: 'drop', player_id: 'white' }, state_after: {}, state_hash: 'hash', created_at_ms: 8 }],
  }), { status: 200, headers: { 'Content-Type': 'application/json' } })) as typeof fetch
  const result = await api.appendAnalysis('game', { id: 'tree', game_id: 'game', name: 'V', base_ply: 3, version: 3, created_at_ms: 0, updated_at_ms: 0, nodes: [] }, 'parent', action)
  assert.equal(result.node.id, 'new-node')
  assert.equal(result.version, 4)
})

test('retention update only sends the requested permanent state', async () => {
  let body: Record<string, unknown> = {}
  globalThis.fetch = (async (_input: string | URL | Request, init?: RequestInit) => {
    body = JSON.parse(String(init?.body))
    return new Response(JSON.stringify({}), { status: 200, headers: { 'Content-Type': 'application/json' } })
  }) as typeof fetch
  await api.updateGameRetention('game', true)
  assert.deepEqual(body, { permanent: true })
})
