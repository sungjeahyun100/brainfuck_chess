import assert from 'node:assert/strict'
import test from 'node:test'

import { api, mergeGameSync } from './gameApi.ts'
import type { GameSyncResponse } from './gameApi.ts'
import type { GameState } from '../types/game.ts'
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

const syncCurrent = {
  id: 'game', board: { size: 8, squares: {} }, pieces: {}, players: {},
  piece_definitions: { rook: { id: 'rook' } }, custom_piece_manifest: [],
  player_info: {}, current_player: 'white', turn_number: 1, phase: 'playing',
  history: [{ turn_number: 1, player_id: 'white', action: { type: 'move' } }],
  record_notation: [{ type: 'move' }], clock: {}, catalog_revision: 4, state_revision: 8,
} as unknown as GameState

function syncResponse(overrides: Partial<GameSyncResponse> = {}): GameSyncResponse {
  return {
    catalog_revision: 4,
    state_revision: 9,
    dynamic: {
      id: 'game', board: { size: 8, squares: {} }, pieces: {}, players: {},
      current_player: 'black', turn_number: 2, phase: 'playing', clock: {} as GameState['clock'],
    },
    latest_ply: 2,
    new_history: [{ turn_number: 2, player_id: 'black', action: { type: 'move' } } as never],
    new_record_notation: [{ type: 'move' } as never],
    resync_required: false,
    ...overrides,
  }
}

test('game sync reuses the catalog and appends only new history', () => {
  const merged = mergeGameSync(syncCurrent, syncResponse())
  assert.equal(merged.piece_definitions, syncCurrent.piece_definitions)
  assert.equal(merged.history.length, 2)
  assert.equal(merged.record_notation?.length, 2)
  assert.equal(merged.current_player, 'black')
})

test('game sync ignores an out-of-order or duplicate response revision', () => {
  assert.equal(
    mergeGameSync(syncCurrent, syncResponse({ state_revision: 8 })),
    syncCurrent,
  )
})

test('game sync can recover from a revision mismatch with a full catalog and history', () => {
  const catalog = {
    piece_definitions: { custom: { id: 'custom' } },
    custom_piece_manifest: [],
    player_info: {},
  } as unknown as NonNullable<GameSyncResponse['catalog']>
  const merged = mergeGameSync(syncCurrent, syncResponse({
    catalog_revision: 5,
    catalog,
    resync_required: true,
  }))
  assert.equal(merged.catalog_revision, 5)
  assert.equal(merged.piece_definitions, catalog.piece_definitions)
  assert.equal(merged.history.length, 1)
})

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

test('analysis options add discriminators omitted by server action arrays', async () => {
  const move = {
    player_id: 'white', piece_id: 'rook', from: { file: 0, rank: 0 }, to: { file: 0, rank: 1 },
    move_option_id: 'normal', source_layer_ids: [],
    effects: { global_state_updates: [], piece_state_updates: [], cooldown_updates: [] },
  }
  const drop = { player_id: 'white', piece_id: 'reserve', to: { file: 1, rank: 1 } }
  const ability = { player_id: 'white', piece_id: 'mortar', ability_id: 'barrage', deployments: [] }
  globalThis.fetch = (async () => new Response(JSON.stringify({
    moves: [move],
    drops: [drop],
    ability_actions: [ability],
    previews: [{ action: { type: 'move', ...move }, state_delta: [], state_hash: 'hash' }],
  }), { status: 200, headers: { 'Content-Type': 'application/json' } })) as typeof fetch

  const result = await api.getAnalysisOptions('game', { base_ply: 0 }, 'rook')

  assert.equal(result.moves[0]?.type, 'move')
  assert.equal(result.drops[0]?.type, 'drop')
  assert.equal(result.ability_actions[0]?.type, 'ability')
  assert.deepEqual(result.moves[0], result.previews[0]?.action)
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


test('ruleset sync preserves both formats across catalog omission and rejects unknown values', () => {
  for (const ruleset of ['legacy', 'standard'] as const) {
    const sync = syncResponse()
    sync.dynamic.ruleset = ruleset
    const first = mergeGameSync(null, { ...sync, catalog: { piece_definitions: {}, custom_piece_manifest: [], player_info: {} } as never })
    assert.equal(first.ruleset, ruleset)
    assert.equal(mergeGameSync(first, { ...sync, state_revision: first.state_revision! + 1 }).ruleset, ruleset)
  }
  assert.equal(mergeGameSync(syncCurrent, syncResponse()).ruleset, 'legacy')
  const bad = syncResponse()
  bad.dynamic.ruleset = 'future' as never
  assert.throws(() => mergeGameSync(syncCurrent, bad), /지원하지 않는 덱 룰/)
})

test('game and room payloads carry deck and top-level ruleset independently of map size', async t => {
  const calls: any[] = []
  t.mock.method(globalThis, 'fetch', async (_url: unknown, init: RequestInit) => { calls.push(JSON.parse(String(init.body))); return Response.json({}) })
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'sessionStorage')
  Object.defineProperty(globalThis, 'sessionStorage', { configurable: true, value: { getItem: () => 'client' } })
  t.after(() => { if (previous) Object.defineProperty(globalThis, 'sessionStorage', previous); else delete (globalThis as any).sessionStorage })
  for (const ruleset of ['legacy', 'standard'] as const) {
    const deck = { ruleset, starting: [], pocket: [] }
    await api.createGame(10, deck, deck, 'standard-10x10', 'unlimited')
    const game = calls.at(-1)
    assert.equal(game.ruleset, ruleset)
    assert.equal(game.white_deck.ruleset, ruleset)
    assert.equal(game.black_deck.ruleset, ruleset)
    assert.equal(game.map_id, 'standard-10x10')
    assert.equal(game.board_size, 10)
    await api.createRoom(10, 'white', deck, 'standard-10x10', 'unlimited')
    assert.equal(calls.at(-1).ruleset, ruleset)
    assert.equal(calls.at(-1).deck.ruleset, ruleset)
    await api.selectRoomDeck('room', deck)
    assert.equal(calls.at(-1).deck.ruleset, ruleset)
    await api.joinRoom('room', deck)
    assert.equal(calls.at(-1).deck.ruleset, ruleset)
  }
})

test('Standard sync replaces reserve identities and keeps own hand plus opponent counts without catalog', () => {
  const own = { id: 'white', deck: { player_id: 'white', starting_pieces: [], pocket_pieces: [], hand_pieces: ['own-hand'], score_limit: 39, total_score: 1 }, captured_pieces: [] }
  const opponent = { id: 'black', deck: { player_id: 'black', starting_pieces: [], pocket_pieces: [], score_limit: 39, total_score: 1 }, captured_pieces: [] }
  const previous = { ...syncCurrent, ruleset: 'standard', players: { white: own, black: { ...opponent, deck: { ...opponent.deck, hand_pieces: ['previously-visible'] } } }, pieces: { 'previously-visible': { id: 'previously-visible' } } } as unknown as GameState
  const sync = syncResponse()
  sync.dynamic = { ...sync.dynamic, ruleset: 'standard', players: { white: own, black: opponent }, pieces: { 'own-hand': { id: 'own-hand', type_id: 'knight', owner: 'white' } } as unknown as GameState['pieces'], hand_counts: { white: 1, black: 2 } }
  const merged = mergeGameSync(previous, sync)
  assert.deepEqual(merged.players.white.deck.hand_pieces, ['own-hand'])
  assert.equal(merged.hand_counts?.black, 2)
  assert.equal(merged.players.black.deck.hand_pieces, undefined)
  assert.ok(!JSON.stringify(merged).includes('previously-visible'))
})

test('live requests carry the tab capability and Bot creation explicitly fixes the bot side', async (t) => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'sessionStorage')
  Object.defineProperty(globalThis, 'sessionStorage', { configurable: true, value: { getItem: () => 'private-tab-capability' } })
  t.after(() => { if (previous) Object.defineProperty(globalThis, 'sessionStorage', previous); else Reflect.deleteProperty(globalThis, 'sessionStorage') })
  const calls: RequestInit[] = []
  globalThis.fetch = (async (_url: unknown, init?: RequestInit) => {
    calls.push(init!)
    return new Response('{}', { status: 200 })
  }) as typeof fetch
  const deck = { ruleset: 'standard' as const, starting: [], pocket: [] }
  await api.createGame(8, deck, deck, 'standard-8x8', 'unlimited', { localSide: 'white', guestNickname: 'Bot', botPlayerId: 'black' })
  await api.getGame('game')
  assert.equal(JSON.parse(String(calls[0].body)).bot_player_id, 'black')
  for (const call of calls) assert.equal(new Headers(call.headers).get('x-game-client-id'), 'private-tab-capability')
})

// Empty custom manifests are omitted by authoritative sparse serialization.
{
  const catalog = { piece_definitions: {}, player_info: {} } as never
  const first = mergeGameSync(null, syncResponse({ catalog }))
  assert.deepEqual(first.custom_piece_manifest, [])
  const oldCatalog = { ...syncCurrent, custom_piece_manifest: [{ package_id: 'formerly-visible-reserve' }] } as unknown as GameState
  const replacement = mergeGameSync(oldCatalog, syncResponse({ catalog }))
  assert.deepEqual(replacement.custom_piece_manifest, [])
  const noManifest = { ...syncCurrent }; delete (noManifest as Partial<GameState>).custom_piece_manifest
  assert.deepEqual(mergeGameSync(noManifest, syncResponse()).custom_piece_manifest, [])
}
