import assert from 'node:assert/strict'
import test from 'node:test'

import {
  classifyCustomPieceError,
  customPieceApi,
  CustomPieceApiError,
} from './customPieceApi.ts'
import type { CustomPieceInput, CustomPieceRecord, CustomPieceTestBoard } from '../types/customPiece.ts'
import type { TurnAction } from '../types/game.ts'

const draft: CustomPieceInput = {
  name: 'Hero',
  description: '',
  score: 4,
  image: { kind: 'built_in', asset_key: 'knight' },
  raw_script: '{"definitions":[]}',
  exposed_piece_key: 'hero',
}

const board: CustomPieceTestBoard = {
  board_size: 8,
  current_player: 'white',
  pieces: [{
    id: 'hero-1',
    piece_key: 'hero',
    owner: 'white',
    square: { file: 1, rank: 1 },
  }],
}

function mockJson(body: unknown, status = 200) {
  const calls: Array<{ url: string; init?: RequestInit }> = []
  globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init })
    return new Response(status === 204 ? null : JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    })
  }) as typeof fetch
  return calls
}

test('error categories keep conflict, image and execution failures distinct', () => {
  assert.equal(classifyCustomPieceError(409, 'version_conflict'), 'conflict')
  assert.equal(classifyCustomPieceError(422, 'unsafe_svg'), 'image')
  assert.equal(classifyCustomPieceError(422, 'execution_limit_exceeded'), 'execution_limit')
  assert.equal(classifyCustomPieceError(503, 'unavailable'), 'server')
})

test('CRUD requests include prototype principal and optimistic version', async () => {
  const calls = mockJson({ ...draft, id: 'piece-1', version: 2 })
  await customPieceApi.update('piece-1', draft, 1)
  assert.equal(calls[0]?.url, '/api/custom-pieces/piece-1')
  assert.equal(calls[0]?.init?.method, 'PUT')
  assert.equal((calls[0]?.init?.headers as Record<string, string>)['X-User-Id'], 'browser-prototype-user')
  assert.deepEqual(JSON.parse(String(calls[0]?.init?.body)), { ...draft, expected_version: 1 })

  mockJson({}, 204)
  await customPieceApi.delete('piece-1', 2)
})

test('uploaded SVG assets are resolved before the custom piece list is returned', async () => {
  const record: CustomPieceRecord = {
    ...draft,
    id: 'uploaded-piece',
    owner_id: 'alice',
    image: { kind: 'uploaded', asset_id: 'svg-asset' },
    internal_piece_keys: [],
    validation_status: 'valid',
    version: 3,
    content_hash: 'hash',
    created_at: 1,
    updated_at: 2,
    active: true,
  }
  const calls: string[] = []
  globalThis.fetch = (async (url: string | URL | Request) => {
    const path = String(url)
    calls.push(path)
    const body = path.endsWith('/image-asset')
      ? { asset_key: 'data:image/svg+xml;base64,PHN2Zy8+' }
      : { items: [record] }
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    })
  }) as typeof fetch

  const response = await customPieceApi.list()

  assert.deepEqual(calls, [
    '/api/custom-pieces',
    '/api/custom-pieces/uploaded-piece/versions/3/image-asset',
  ])
  assert.equal(
    response.items[0].resolved_image_asset_key,
    'data:image/svg+xml;base64,PHN2Zy8+',
  )
})

test('validation and option preview use separate server endpoints without mutating board input', async () => {
  const original = structuredClone(board)
  const validationCalls = mockJson({
    valid: true,
    diagnostics: [],
    exposed_piece_key: 'hero',
    internal_piece_keys: [],
    preview_definitions: [],
  })
  await customPieceApi.validate(draft)
  assert.equal(validationCalls[0]?.url, '/api/custom-pieces/validate')

  const optionCalls = mockJson({ state: {}, legal_moves: [], legal_drops: [], attacks: [] })
  await customPieceApi.testOptions(draft, board, 'hero-1')
  assert.equal(optionCalls[0]?.url, '/api/custom-pieces/test/options')
  assert.deepEqual(board, original)
})

test('test actions add the discriminator omitted by legal move responses', async () => {
  const calls = mockJson({ state: {}, legal_moves: [], legal_drops: [], attacks: [] })
  const legalMoveResponse = {
    player_id: 'white',
    piece_id: 'hero-1',
    from: { file: 1, rank: 1 },
    to: { file: 1, rank: 2 },
    move_option_id: 'normal',
    source_layer_ids: ['default'],
    effects: {
      global_state_updates: [],
      piece_state_updates: [],
      cooldown_updates: [],
    },
  } as unknown as TurnAction

  await customPieceApi.testAction(draft, board, legalMoveResponse)

  const request = JSON.parse(String(calls[0]?.init?.body))
  assert.equal(request.action.type, 'move')
  assert.equal(request.action.piece_id, 'hero-1')
})

test('structured API failures are mapped to a safe typed error', async () => {
  mockJson({ code: 'version_conflict', error: '최신 버전이 아닙니다.' }, 409)
  await assert.rejects(
    () => customPieceApi.update('piece-1', draft, 1),
    (error: unknown) => error instanceof CustomPieceApiError
      && error.kind === 'conflict'
      && error.status === 409,
  )
})
