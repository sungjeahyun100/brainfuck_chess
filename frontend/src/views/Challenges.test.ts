import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { compileScript, parse } from '@vue/compiler-sfc'
import ts from 'typescript'
import * as vue from 'vue'
import * as maps from '../boardMaps.ts'
import * as rulesets from '../deckRulesets.ts'
import * as validation from '../composables/useDeckValidation.ts'
import * as serialization from '../composables/useDeckSerialization.ts'
import { createNewSavedDeck } from '../composables/localDeckRepository.ts'

const source = readFileSync(new URL('./Challenges.vue', import.meta.url), 'utf8')
const { descriptor } = parse(source)
const compiled = ts.transpileModule(compileScript(descriptor, { id: 'challenge-test' }).content, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
}).outputText
validation.applyPieceMetadata(Object.fromEntries(validation.pieceCatalog.map(p => [p.id, {
  score: p.id === 'pawn' ? 1 : p.id === 'king' ? 0 : 3,
  deployment_zone: p.id === 'pawn' ? 'front' : 'back',
}])))

test('Challenge component selects matching formats, shows reasons and submits player format', async t => {
  const legacy = createNewSavedDeck('standard-12x12')
  const standard = { ...createNewSavedDeck('standard-12x12'), ...validation.createPresetDeck(12, 'classic', 'standard'), ruleset: 'standard', id: 'standard' }
  const decks = vue.ref([legacy, standard])
  const calls: any[] = []; const events: any[] = []
  const modules: Record<string, unknown> = {
    vue, '../deckRulesets': rulesets, '../boardMaps': maps,
    '../composables/useDeckValidation': validation,
    '../composables/useDeckSerialization': serialization,
    '../composables/useSavedDecks': { useSavedDecks: () => ({ decks, loading: vue.ref(false), error: vue.ref(null) }) },
    '../api/gameApi': { api: { createChallengeGame: async (...args: any[]) => { calls.push(args); return { state: { ruleset: args[1].ruleset } } }, listChallenges: async () => [] } },
  }
  const exports: any = {}
  new Function('require', 'exports', compiled)((id: string) => { assert.ok(id in modules, id); return modules[id] }, exports)
  const scope = vue.effectScope(); t.after(() => scope.stop())
  const state = scope.run(() => exports.default.setup({}, { expose() {}, emit: (...args: any[]) => events.push(args) }))
  const challenge = { id: 'tempest_horde', name: '템페스트 호드', description: '', ruleset: 'legacy', board_size: 12, map_id: 'standard-12x12', bot_difficulty: 'normal', time_control: 'unlimited', cleared: true }
  state.selectChallenge(challenge)
  assert.equal(state.selectedDeckId.value, legacy.id)
  assert.match(state.availability(standard).reason, /Legacy/)
  await state.start()
  assert.equal(calls[0][0], 'tempest_horde')
  assert.equal(calls[0][1].map_id, legacy.mapId)
  assert.equal(calls[0][1].board_size, legacy.boardSize)
  assert.equal(calls[0][1].ruleset, 'legacy')
  state.selectChallenge({ ...challenge, ruleset: 'standard', id: 'fixture' })
  assert.equal(state.selectedDeckId.value, standard.id)
  assert.match(state.availability(legacy).reason, /Standard/)
  assert.equal(state.availability({ ...standard, mapId: 'central-high-ground-12x12' }).valid, false)
  assert.equal(state.availability({ ...standard, boardSize: 10 }).valid, false)
  assert.equal(state.availability({ ...standard, starting: [] }).valid, false)
  await state.start()
  assert.equal(calls[1][1].ruleset, 'standard')
  assert.equal(events.length, 2)
  state.selectedDeckId.value = legacy.id
  await state.start()
  assert.equal(calls.length, 2)
  for (const fragment of ['formatLabel(challenge.ruleset)', 'boardMapLabel(challenge.map_id)', 'challenge.cleared', ':disabled="!availability(deck).valid"', 'availability(deck).reason']) assert.ok(source.includes(fragment), fragment)
})
