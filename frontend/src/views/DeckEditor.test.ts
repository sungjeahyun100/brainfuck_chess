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
