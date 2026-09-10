import * as rulesets from '../deckRulesets.ts'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileScript, parse } from '@vue/compiler-sfc'
import ts from 'typescript'
import * as vue from 'vue'
import * as boardMaps from '../boardMaps.ts'
import * as validation from '../composables/useDeckValidation.ts'
import * as deckCode from '../composables/useDeckCode.ts'
import * as deckCodec from '../composables/useDeckCodeCodec.ts'
import { createNewSavedDeck } from '../composables/localDeckRepository.ts'
import type { SavedDeck } from '../types/deck.ts'

// Exercise the real component setup and handlers without requiring a browser or backend.
const { descriptor } = parse(readFileSync(new URL('./DeckEditor.vue', import.meta.url), 'utf8'))
const compiled = ts.transpileModule(compileScript(descriptor, { id: 'deck-editor-test' }).content, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
}).outputText

async function editor(t: any, persist: (deck: SavedDeck) => Promise<SavedDeck>) {
  const original = { ...createNewSavedDeck('standard-12x12'), version: 1 }
  const events: string[] = []
  const scope = vue.effectScope()
  t.after(() => scope.stop())
  const modules: Record<string, unknown> = {
    vue,
    '../deckRulesets': rulesets,
    '../pieceAssets': { pieceAsset: () => undefined },
    '../api/customPieceApi': { customPieceApi: { list: async () => ({ items: [] }) } },
    '../composables/useDeckValidation': validation,
    '../composables/useSavedDecks': {
      createNewSavedDeck,
      deckStorageIdentity: vue.ref('account'),
      useSavedDecks: () => ({
        busy: vue.ref(false), error: vue.ref(null),
        getDeck: async () => original, saveDeck: persist,
      }),
    },
    '../composables/useDeckCodeCodec': deckCodec,
    '../composables/useDeckCode': deckCode,
    '../boardMaps': boardMaps,
  }
  const exports: any = {}
  new Function('require', 'exports', compiled)((id: string) => {
    assert.ok(id in modules, `Unexpected component dependency: ${id}`)
    return modules[id]
  }, exports)
  const state = scope.run(() => exports.default.setup({ deckId: original.id }, {
    expose() {}, emit: (event: string) => events.push(event),
  }))
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(state.deckLoaded.value, true)
  return { state, events }
}

test('switching between equal-size maps preserves all draft content in both directions', async t => {
  const { state } = await editor(t, async deck => deck)
  state.deck.value.name = '편집한 덱'
  state.deck.value.starting = [{ pieceType: 'knight', square: { file: 3, rank: 1 } }]
  state.deck.value.pocket = { rook: 2 }
  state.deck.value.customPieces = [{ id: 'custom', version: 2, contentHash: 'hash', exposedPieceKey: 'hero' }]
  const before = JSON.parse(JSON.stringify(state.deck.value))
  for (const mapId of ['central-high-ground-12x12', 'standard-12x12']) {
    state.deck.value.mapId = mapId
    state.changeMap()
    assert.deepEqual(JSON.parse(JSON.stringify(state.deck.value)), { ...before, mapId })
  }
})

test('changing board size still applies the classic preset for that size', async t => {
  const { state } = await editor(t, async deck => deck)
  state.deck.value.mapId = 'standard-8x8'
  state.changeMap()
  assert.equal(state.deck.value.boardSize, 8)
  const preset = validation.createPresetDeck(8)
  assert.deepEqual(state.deck.value.starting, preset.starting)
  assert.deepEqual(state.deck.value.pocket, preset.pocket)
})

test('selecting a piece and clicking the pocket adds one per click without changing the frontline', async t => {
  const { state } = await editor(t, async deck => deck)
  state.deck.value.pocket = {}
  const starting = JSON.parse(JSON.stringify(state.deck.value.starting))
  state.placementTool.value = 'knight'
  assert.match(state.pocketDropMessage.value, /여기를 눌러/)
  state.onPocketClick()
  state.onPocketClick()
  state.placementTool.value = 'rook'
  state.onPocketClick()
  assert.deepEqual(state.deck.value.pocket, { knight: 2, rook: 1 })
  assert.deepEqual(state.deck.value.starting, starting)
  assert.equal(state.placementTool.value, 'rook')
})

test('clicking the pocket with King or the eraser selected preserves its contents', async t => {
  const { state } = await editor(t, async deck => deck)
  state.deck.value.pocket = { knight: 2 }
  state.placementTool.value = 'king'
  assert.match(state.pocketDropMessage.value, /포켓에 넣을 수 없습니다/)
  state.onPocketClick()
  state.placementTool.value = state.eraseTool
  assert.equal(state.pocketTargetPiece.value, null)
  assert.match(state.pocketDropMessage.value, /기물을 선택한 뒤/)
  state.onPocketClick()
  assert.deepEqual(state.deck.value.pocket, { knight: 2 })
})

test('pocket drops still use the dragged piece and reject King regardless of the selected tool', async t => {
  const { state } = await editor(t, async deck => deck)
  state.deck.value.pocket = {}
  state.placementTool.value = state.eraseTool
  state.onPieceDragStart({}, 'rook')
  assert.equal(state.pocketTargetPiece.value, 'rook')
  state.onPocketDrop({})
  assert.deepEqual(state.deck.value.pocket, { rook: 1 })
  assert.equal(state.draggedPiece.value, null)
  state.placementTool.value = 'knight'
  state.onPieceDragStart({}, 'king')
  state.onPocketDrop({})
  assert.deepEqual(state.deck.value.pocket, { rook: 1 })
  assert.equal(state.draggedPiece.value, null)
})

test('successful saves keep the editor loaded, show feedback, and advance the version for repeated saves', async t => {
  const versions: (number | undefined)[] = []
  const { state, events } = await editor(t, async deck => {
    versions.push(deck.version)
    return { ...deck, version: deck.version! + 1, updatedAt: 12345 }
  })
  await state.save()
  assert.equal(state.saveNotice.value, '덱을 저장했습니다.')
  assert.equal(state.deckLoaded.value, true)
  assert.equal(state.deck.value.updatedAt, 12345)
  state.deck.value.name = '다시 편집'
  assert.equal(state.saveNotice.value, null)
  await state.save()
  assert.deepEqual(versions, [1, 2])
  assert.equal(state.deck.value.version, 3)
  assert.deepEqual(events, ['saved', 'saved'])
  const app = parse(readFileSync(new URL('../App.vue', import.meta.url), 'utf8')).descriptor
  const editorTag = app.template!.content.match(/<DeckEditor\b[^>]*\/>/)![0]
  assert.doesNotMatch(editorTag, /@saved=/)
})

test('a failed save preserves the draft and shows an error without a success notice', async t => {
  const { state, events } = await editor(t, async () => { throw new Error('저장 실패') })
  const before = JSON.parse(JSON.stringify(state.deck.value))
  await state.save()
  assert.equal(state.saveError.value, '저장 실패')
  assert.equal(state.saveNotice.value, null)
  assert.deepEqual(JSON.parse(JSON.stringify(state.deck.value)), before)
  assert.deepEqual(events, [])
})

test('saving does not overwrite edits made while the request is pending', async t => {
  let finish!: (deck: SavedDeck) => void
  let snapshot!: SavedDeck
  const { state } = await editor(t, deck => {
    snapshot = deck
    return new Promise(resolve => { finish = resolve })
  })
  const pending = state.save()
  state.deck.value.name = '저장 중 추가 편집'
  finish({ ...snapshot, version: 2 })
  await pending
  assert.equal(state.deck.value.name, '저장 중 추가 편집')
  assert.equal(state.deck.value.version, 2)
})


test('ruleset editor selection is independent of size/map and is preserved by clone and save', async t => {
  const persisted: SavedDeck[] = []
  const { state } = await editor(t, async deck => { persisted.push(deck); return { ...deck, version: 2 } })
  for (const ruleset of ['legacy', 'standard'] as const) {
    const before = JSON.parse(JSON.stringify(state.deck.value))
    state.deck.value.ruleset = ruleset
    await vue.nextTick()
    assert.deepEqual(JSON.parse(JSON.stringify(state.deck.value)), { ...before, ruleset })
    assert.equal(state.cloneSavedDeck(state.deck.value).ruleset, ruleset)
    for (const mapId of ['central-high-ground-12x12', 'standard-12x12', 'standard-8x8', 'standard-10x10']) {
      state.deck.value.mapId = mapId
      state.changeMap()
      assert.equal(state.deck.value.ruleset, ruleset)
      assert.equal(state.deck.value.boardSize, boardMaps.findBoardMap(mapId)!.boardSize)
    }
    await state.save()
    assert.equal(state.saveError.value, null)
    assert.equal(persisted.at(-1)!.ruleset, ruleset)
  }
  const { ruleset: _ruleset, ...old } = state.deck.value
  assert.equal(state.cloneSavedDeck(old).ruleset, 'legacy')
  assert.throws(() => state.cloneSavedDeck({ ...old, ruleset: 'future' }), /지원하지 않는 덱 룰/)
  state.deck.value.ruleset = 'standard'
  assert.equal(state.canCopyDeckCode.value, true)
  // Verify the real Vue template binds a separate labelled selector.
  assert.match(descriptor.template!.content, /<select v-model="deck.ruleset"/)
  assert.match(descriptor.template!.content, /<select v-model="deck.mapId"/)
})


test('Standard editor displays actual coordinates, restricts clicks/drags, and resets by size', async t => {
  const { state } = await editor(t, async deck => deck)
  validation.applyPieceMetadata(Object.fromEntries(validation.pieceCatalog.filter(p => !p.custom).map(p => [p.id, {
    score: p.id === 'pawn' ? 1 : 0, deployment_zone: p.id === 'pawn' ? 'front' : 'back',
  }])))
  state.deck.value.ruleset = 'standard'
  for (const size of [8, 9, 10, 11, 12]) {
    state.deck.value.mapId = `standard-${size}x${size}`
    state.changeMap()
    state.applyPreset('classic')
    assert.equal(state.deckSummary.value.valid, true)
    assert.deepEqual(state.deck.value.starting, validation.createPresetDeck(size, 'classic', 'standard').starting)
    const cells = state.placementZoneSections.value.flatMap((z: any) => z.squares)
    assert.equal(cells.length, 2 * size)
    assert.deepEqual(cells.slice(0, size), Array.from({ length: size }, (_, file) => ({file, rank: 1})))
    const front = validation.frontZoneSquares(size, 'white', 'standard')
    const back = validation.backZoneSquares(size, 'white', 'standard')
    state.placementTool.value = 'knight'
    for (const square of cells) {
      assert.equal(state.squareClass(square.file, square.rank).includes('restricted'), state.squareZone(square.file, square.rank) !== 'back')
    }
    const before = JSON.stringify(state.deck.value.starting)
    state.onPlacementSquareClick(front[0].file, front[0].rank)
    state.onPlacementSquareClick(0, 0)
    assert.equal(JSON.stringify(state.deck.value.starting), before)
    assert.match(state.placementError.value, /진영 밖/)
    state.onPieceDragStart({}, 'pawn')
    state.onPlacementDrop({}, back[0].file, back[0].rank)
    assert.equal(JSON.stringify(state.deck.value.starting), before)
    state.placementTool.value = 'guhang'
    const vacancy = back.find(s => state.pieceAt(s.file, s.rank) === null)!
    state.onPlacementSquareClick(vacancy.file, vacancy.rank)
    assert.equal(state.pieceAt(vacancy.file, vacancy.rank), null)
    assert.match(state.placementError.value, /Extra Deck 전용/)
    state.onPieceDragStart({}, 'bomber')
    state.onPlacementDrop({}, vacancy.file, vacancy.rank)
    assert.equal(state.pieceAt(vacancy.file, vacancy.rank), null)
    assert.match(state.placementError.value, /Extra Deck 전용/)
    state.placementTool.value = state.eraseTool
    state.onPlacementSquareClick(front[0].file, front[0].rank)
    assert.equal(state.deckSummary.value.valid, false)
    assert.equal(state.canSaveDeck.value, true)
    state.placementTool.value = 'pawn'
    state.onPlacementSquareClick(front[0].file, front[0].rank)
    assert.equal(state.deckSummary.value.valid, true)
  }
  const beforeMap = JSON.stringify(state.deck.value.starting)
  state.deck.value.mapId = 'central-high-ground-12x12'
  state.changeMap()
  assert.equal(JSON.stringify(state.deck.value.starting), beforeMap)
  assert.match(descriptor.template!.content, /squareRestriction\(square.file, square.rank\)/)
  assert.match(descriptor.template!.content, /:disabled="deck.ruleset === 'standard' && !squareZone/)
})

test('Extra editor adds instances, enforces capacity, preserves drafts and separates Main score', async t => {
  let saved: SavedDeck | undefined
  const { state } = await editor(t, async deck => { saved = deck; return deck })
  state.deck.value.ruleset = 'standard'
  state.applyPreset('classic')
  const main = state.deckSummary.value.totalScore
  assert.deepEqual(state.extraPieces.value, [])
  state.addExtra('guhang'); state.addExtra('bomber'); state.addExtra('bomber')
  state.addExtra('guhang'); state.addExtra('knight')
  assert.deepEqual(state.extraPieces.value, ['guhang', 'bomber', 'bomber'])
  assert.equal(state.deckSummary.value.totalScore, main)
  assert.equal(state.deckSummary.value.valid, true)
  const clone = state.cloneSavedDeck(state.deck.value)
  clone.extra.pop()
  assert.equal(state.extraPieces.value.length, 3)
  await state.save()
  assert.deepEqual(saved!.extra, ['guhang', 'bomber', 'bomber'])
  state.deck.value.pocket = { guhang: 1, bomber: 1 }
  const before = JSON.stringify(state.deck.value)
  state.deck.value.ruleset = 'legacy'
  assert.match(state.deckSummary.value.errors.join(' '), /Legacy.*Extra/)
  assert.equal(state.canSaveDeck.value, true)
  state.deck.value.ruleset = 'standard'
  assert.equal(JSON.stringify(state.deck.value), before)
  assert.equal(state.deckSummary.value.valid, false)
  state.changePocketCount('guhang', 1)
  assert.equal(state.deck.value.pocket.guhang, 1)
  state.changePocketCount('guhang', -1); state.changePocketCount('bomber', -1)
  assert.equal(state.deckSummary.value.valid, true)
  state.removeExtra(1)
  await state.save()
  assert.deepEqual(saved!.extra, ['guhang', 'bomber'])
  assert.match(descriptor.template!.content, /extraPieces.length >= MAX_EXTRA_DECK_PIECES/)
  assert.match(descriptor.template!.content, /Main Deck 점수 상한에 포함되지 않습니다/)
})

test('G7 real editor copies/imports DC4 atomically and DC3 explicitly restores Legacy', async t => {
  const { state } = await editor(t, async deck => deck)
  const codes: string[] = []
  const clipboard = Object.getOwnPropertyDescriptor(navigator, 'clipboard')
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: async (code: string) => { codes.push(code) } } })
  t.after(() => { if (clipboard) Object.defineProperty(navigator, 'clipboard', clipboard); else Reflect.deleteProperty(navigator, 'clipboard') })
  state.deck.value.ruleset = 'standard'
  state.resetToClassic()
  state.addExtra('guhang')
  state.addExtra('guhang')
  state.addExtra('bomber')
  state.deck.value.pocket.knight = 2
  await state.copyDeckCode()
  assert.match(codes[0], /^DC4\./u)
  const original = JSON.parse(JSON.stringify(state.deck.value))
  state.openImportDialog()
  state.importCode.value = codes[0]
  state.prepareImport()
  assert.ok(state.importCandidate.value)
  state.importCode.value = 'DC4.invalid'
  state.prepareImport()
  assert.equal(state.importCandidate.value, null)
  state.applyImportedDeck()
  assert.deepEqual(JSON.parse(JSON.stringify(state.deck.value)), original)
  const legacyCode = deckCodec.encodeDeckCode(createNewSavedDeck())
  for (const [code, ruleset, extra] of [[legacyCode, 'legacy', []], [codes[0], 'standard', ['bomber', 'guhang', 'guhang']]] as const) {
    state.openImportDialog(); state.importCode.value = code; state.prepareImport(); state.applyImportedDeck()
    assert.equal(state.deck.value.ruleset, ruleset)
    assert.deepEqual([...state.deck.value.extra], extra)
  }
  assert.equal(state.deck.value.boardSize, original.boardSize)
  assert.equal(state.deck.value.mapId, original.mapId)
  assert.equal(state.deck.value.pocket.knight, 2)
  assert.deepEqual([...state.deck.value.starting], [...original.starting].sort((a,b) => a.square.rank-b.square.rank || a.square.file-b.square.file))
})

test('G7 custom Extra copy collects unsaved references and keeps pinned hashes without mutating the draft', async t => {
  const { state } = await editor(t, async deck => deck)
  const ref = { id: 'g7-editor', version: 2, contentHash: 'frozen', exposedPieceKey: 'hero' }
  const id = 'custom:g7-editor:v2:hero'
  validation.pieceCatalog.push({ id, name: 'Custom Extra', score: 3, category: 'custom', canPocket: true, deploymentZone: 'back',
    custom: { ...ref, active: true, image: { kind: 'built_in', asset_key: 'knight' } } })
  t.after(() => validation.pieceCatalog.splice(validation.pieceCatalog.findIndex(piece => piece.id === id), 1))
  state.deck.value.ruleset = 'standard'
  state.deck.value.extra = [id, id]
  state.deck.value.customPieces = []
  assert.deepEqual(state.collectDeckCustomPieces(true), [ref])
  state.deck.value.customPieces = [{ ...ref, contentHash: 'original-hash' }]
  assert.equal(state.collectDeckCustomPieces(true)[0].contentHash, 'original-hash')
  const before = JSON.parse(JSON.stringify(state.deck.value))
  const catalogBefore = JSON.parse(JSON.stringify(validation.pieceCatalog))
  const malformed = deckCodec.encodeDeckCode({ ...state.cloneSavedDeck(state.deck.value), customPieces: [ref] })
  state.openImportDialog()
  state.importCode.value = malformed.replace(/^DC4\./u, 'DC3.')
  state.prepareImport(); state.applyImportedDeck()
  assert.deepEqual(JSON.parse(JSON.stringify(state.deck.value)), before)
  assert.deepEqual(JSON.parse(JSON.stringify(validation.pieceCatalog)), catalogBefore)
})
