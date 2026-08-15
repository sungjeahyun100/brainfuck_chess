import assert from 'node:assert/strict'
import test from 'node:test'

import {
  advancedDraftFromInput,
  buildAdvancedCustomPieceInput,
  buildCustomPieceInput,
  customPieceDraftSnapshot,
  customPieceTemplate,
  newAdvancedCustomPieceDraft,
  newSimpleCustomPieceDraft,
  newCustomPieceScript,
  parseCustomPiecePackage,
  serializeCustomPiecePackage,
  simpleDraftFromInput,
  testPiecesFromServerState,
  validateAdvancedCustomPieceDraft,
  validateCustomPieceDraft,
} from './useCustomPieceDraft.ts'
import type { CustomPieceInput, SimpleCustomPieceDraft } from '../types/customPiece.ts'
import type { GameState } from '../types/game.ts'

const draft = (): SimpleCustomPieceDraft => ({
  name: 'Hero',
  description: '',
  score: 5,
  image: { kind: 'built_in', asset_key: 'knight' },
  movementCode: 'move(1, 0);',
  abilities: [],
})

test('required fields and score boundaries are validated locally', () => {
  assert.equal(validateCustomPieceDraft(draft()), '')
  assert.match(validateCustomPieceDraft({ ...draft(), name: '' }), /이름/)
  assert.match(validateCustomPieceDraft({ ...draft(), score: 0 }), /1–30/)
  assert.match(validateCustomPieceDraft({ ...draft(), score: 31 }), /1–30/)
  assert.match(validateCustomPieceDraft({ ...draft(), movementCode: '' }), /움직임 코드/)
})

test('a simple draft builds the existing package protocol with system-owned fields', () => {
  const source = draft()
  const input = buildCustomPieceInput(source)
  const script = JSON.parse(input.raw_script)
  assert.equal(script.format, 'brainfuck-chess-piece-set-v1')
  assert.equal(script.definitions[0].id, 'main')
  assert.equal(script.definitions[0].name, 'Hero')
  assert.equal(script.definitions[0].score, 5)
  assert.equal(script.definitions[0].chessembly_code, 'move(1, 0);')
  assert.equal(input.exposed_piece_key, 'main')
  assert.deepEqual(simpleDraftFromInput(input), source)
  assert.equal(JSON.stringify(input).includes('movementCode'), false)
  assert.notEqual(input.image, source.image)
})

test('new custom piece defaults only ask for user-facing fields', () => {
  const simple = newSimpleCustomPieceDraft()
  assert.equal(simple.name, '')
  assert.equal(simple.abilities.length, 0)
  assert.match(simple.movementCode, /move/)
  assert.equal(JSON.parse(newCustomPieceScript()).definitions[0].id, 'main')
  assert.equal(JSON.parse(newCustomPieceScript()).definitions[0].deployment_zone, 'back')
})

test('custom deployment zones round-trip and legacy definitions default to back', () => {
  const document = parseCustomPiecePackage(newCustomPieceScript())
  document.definitions[0].deployment_zone = 'front'
  assert.equal(
    parseCustomPiecePackage(serializeCustomPiecePackage(document)).definitions[0].deployment_zone,
    'front',
  )

  const legacy = JSON.parse(newCustomPieceScript())
  delete legacy.definitions[0].deployment_zone
  assert.equal(parseCustomPiecePackage(JSON.stringify(legacy)).definitions[0].deployment_zone, 'back')
})

test('the structured editor package round-trips without changing the server envelope', () => {
  const parsed = parseCustomPiecePackage(newCustomPieceScript())
  parsed.definitions[0].state_schema.push({ key: 'charged', default_value: false })
  parsed.definitions[0].move_layers.push({
    id: 'charged-move',
    chessembly_code: 'move(0, 2);',
    enabled_when: [{ key: 'charged', condition: { equals: true } }],
    on_commit: [{ key: 'charged', value: false }],
  })
  const roundTrip = parseCustomPiecePackage(serializeCustomPiecePackage(parsed))
  assert.deepEqual(roundTrip, parsed)
  assert.equal(roundTrip.format, 'brainfuck-chess-piece-set-v1')
})

test('advanced drafts preserve state layers, options and internal definitions', () => {
  const source = newAdvancedCustomPieceDraft(draft())
  const document = customPieceTemplate('windmill')
  const internal = document.definitions[0]
  internal.id = 'internal-form'
  document.definitions.push({ ...internal, id: 'main', name: 'Hero' })
  source.rawScript = serializeCustomPiecePackage(document)
  source.exposedPieceKey = 'main'
  const input = buildAdvancedCustomPieceInput(source)
  const parsed = parseCustomPiecePackage(input.raw_script)
  assert.equal(parsed.definitions.length, 2)
  assert.equal(parsed.definitions[1].move_layers.length, 2)
  assert.equal(parsed.definitions[1].move_options[0].layer_ids.length, 2)
  assert.equal(parsed.definitions[1].state_schema[0].key, 'mode')
  assert.equal(parsed.definitions[1].can_capture_on_drop, false)
  assert.deepEqual(advancedDraftFromInput(input), {
    ...source,
    rawScript: input.raw_script,
  })
})

test('advanced templates expose selectable movement and cooldown examples', () => {
  const windmill = customPieceTemplate('windmill').definitions[0]
  assert.equal(windmill.state_schema[0].key, 'mode')
  assert.equal(windmill.move_layers.length, 2)
  const cannon = customPieceTemplate('cannon-rook').definitions[0]
  assert.equal(cannon.move_options[1].kind, 'ability')
  assert.equal(cannon.move_options[1].cooldown?.turns, 3)
  const bouncing = customPieceTemplate('bouncing-bishop').definitions[0]
  assert.match(bouncing.move_layers[1].chessembly_code, /edge/)
})

test('advanced validation rejects missing exposed definitions', () => {
  const advanced = newAdvancedCustomPieceDraft(draft())
  advanced.exposedPieceKey = 'missing'
  assert.match(validateAdvancedCustomPieceDraft(advanced), /대표 기물/)
})

test('the structured editor rejects packages it cannot safely represent', () => {
  assert.throws(() => parseCustomPiecePackage('[]'), /객체/)
  assert.throws(() => parseCustomPiecePackage('{"format":"unknown","definitions":[{}]}'), /지원하지 않는/)
  assert.throws(() => parseCustomPiecePackage('{"format":"brainfuck-chess-piece-set-v1","definitions":[]}'), /하나 이상/)
})

test('draft snapshots make validation and dirty state invalidation explicit', () => {
  const original = draft()
  const validatedSnapshot = customPieceDraftSnapshot(original)
  assert.equal(customPieceDraftSnapshot(original), validatedSnapshot)
  original.movementCode = 'move(0, 2);'
  assert.notEqual(customPieceDraftSnapshot(original), validatedSnapshot)
})

test('an applied server state becomes the next test board including transitions and captures', () => {
  const state = {
    current_player: 'black',
    pieces: {
      hero: { id: 'hero', type_id: 'custom:test:hero-east', owner: 'white', current_square: { file: 4, rank: 3 }, in_pocket: false, captured: false, state: { direction: 'east' } },
      captured: { id: 'captured', type_id: 'pawn', owner: 'black', current_square: undefined, in_pocket: false, captured: true, state: {} },
    },
  } as GameState
  assert.deepEqual(testPiecesFromServerState(state), [{
    id: 'hero', piece_key: 'hero-east', owner: 'white', square: { file: 4, rank: 3 }, state: { direction: 'east' },
  }])
})
