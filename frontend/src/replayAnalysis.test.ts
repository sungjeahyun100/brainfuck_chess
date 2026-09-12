import * as dropHelpers from './dropSacrifices.ts'
import { STANDARD_RULES_VERSION } from './gameRulesVersions.ts'
import assert from 'node:assert/strict'
import test from 'node:test'
import { actionIdentity, analysisPosition, reconcileOptimisticNode } from './replayAnalysis.ts'
import type { AnalysisNode, AnalysisTree } from './types/gameRecord.ts'
import type { GameState, TurnAction } from './types/game.ts'

const action = (piece: string): TurnAction => ({
  type: 'drop', player_id: 'white', piece_id: piece, to: { file: 0, rank: 0 },
})
const state = { current_player: 'white' } as GameState
const node = (id: string, parent: string | null, pending = false): AnalysisNode => ({
  id, parent_node_id: parent, pending, action: action(id), state_after: state,
  state_hash: id, created_at_ms: 1,
})
const tree = (nodes: AnalysisNode[]): AnalysisTree => ({
  id: 'tree', game_id: 'game', name: 'Variation 1', base_ply: 10, version: 2,
  created_at_ms: 1, updated_at_ms: 1, nodes,
})

test('four optimistic plies retain their ordered action suffix', () => {
  const nodes = [node('local-a', null, true), node('local-b', 'local-a', true), node('local-c', 'local-b', true), node('local-d', 'local-c', true)]
  assert.deepEqual(analysisPosition({ ...tree(nodes), id: 'local-tree' }, nodes[3], 0).pending_actions, nodes.map(item => item.action))
})

test('acknowledging a parent reparents queued children without moving the cursor', () => {
  const parent = node('local-b', 'server-a', true)
  const child = node('local-c', 'local-b', true)
  const value = tree([node('server-a', null), parent, child])
  const replacement = reconcileOptimisticNode(value, parent.id, node('server-b', 'server-a'))
  assert.equal(replacement?.id, 'server-b')
  assert.equal(child.parent_node_id, 'server-b')
  assert.deepEqual(analysisPosition(value, child, 0), {
    base_ply: 10, tree_id: 'tree', node_id: 'server-b', pending_actions: [child.action],
  })
})

test('returning to a persisted parent creates a sibling branch position', () => {
  const a = node('a', null), b = node('b', 'a'), c = node('c', 'b'), d = node('local-d', 'b', true)
  const value = tree([a, b, c, d])
  assert.deepEqual(analysisPosition(value, d, 0), {
    base_ply: 10, tree_id: 'tree', node_id: 'b', pending_actions: [d.action],
  })
})

test('canonical action matching ignores JSON property insertion order', () => {
  const left = action('piece')
  const right = { to: { rank: 0, file: 0 }, piece_id: 'piece', player_id: 'white', type: 'drop' } as TurnAction
  assert.equal(actionIdentity(left), actionIdentity(right))
})

// Exercise the actual ReplayPage setup, including the Standard commit boundary.
import { readFileSync } from 'node:fs'
import { compileScript, parse } from '@vue/compiler-sfc'
import ts from 'typescript'
import * as vue from 'vue'
import * as replayState from './replayState.ts'
import * as replayNotation from './replayNotation.ts'
import * as replayAnalysis from './replayAnalysis.ts'
import * as standardGameUi from './standardGameUi.ts'
import type { GameRecord } from './types/gameRecord.ts'

const { descriptor: replayDescriptor } = parse(readFileSync(new URL('./views/ReplayPage.vue', import.meta.url), 'utf8'))
const replayCompiled = ts.transpileModule(compileScript(replayDescriptor, { id: 'replay-page-test' }).content.replaceAll('import.meta.env.DEV', 'false'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
}).outputText

function standardRecord(): GameRecord {
  const pieces = Object.fromEntries(['w1','w2','w3','w4','w5','b1','b2','b3','b4'].map(id => [id, { id, owner: id[0] === 'w' ? 'white' : 'black', type_id: 'knight', in_pocket: ['w5','b4'].includes(id), current_square: null, captured: false }]))
  const initial = { id: 'standard', ruleset: 'standard', board: { size: 8, squares: {} }, pieces, piece_definitions: { knight: { name: '나이트' } }, players: {
    white: { deck: { hand_pieces: ['w1','w2','w3','w4'], pocket_pieces: ['w5'] } },
    black: { deck: { hand_pieces: ['b1','b2','b3'], pocket_pieces: ['b4'] } },
  }, current_player: 'white', turn_number: 1, history: [] }
  return { ruleset_version: STANDARD_RULES_VERSION, game_id: 'standard', initial_state: initial, ended_at_ms: 1000, initial_clock: {}, actions: [],
    players: { white: {}, black: {} }, decks: { white: { deployments: [], pocket: [] }, black: { deployments: [], pocket: [] } },
    initial_draws: [ { player_id: 'white', timing: 'initial', piece_ids: ['w1','w2','w3'] }, { player_id: 'black', timing: 'initial', piece_ids: ['b1','b2','b3'] }, { player_id: 'white', timing: 'turn_start', piece_ids: ['w4'] } ],
  } as unknown as GameRecord
}

test('Standard Replay shows exact initial hands and waits for committed Draw before further analysis', async t => {
  const scope = vue.effectScope(); t.after(() => scope.stop())
  const record = standardRecord()
  const initial = replayState.buildReplayFrames(record)[0]
  const committed = structuredClone(initial)
  committed.players.black.deck.hand_pieces!.push('b4')
  committed.players.black.deck.pocket_pieces = []
  committed.pieces.b4.in_pocket = false
  committed.current_player = 'black'
  const resolved: AnalysisNode = { ...node('saved', null), action: action('w1'), state_after: committed, state_hash: 'exact-server-hash', draws: [{ player_id: 'black', timing: 'turn_start', piece_ids: ['b4'] }] }
  let save!: (tree: AnalysisTree) => void
  let writes = 0, previews = 0
  const modules: Record<string, unknown> = {
    vue: { ...vue, onMounted() {}, onUnmounted() {} }, '../components/Board.vue': {}, '../components/ExtraSummonPanel.vue': {}, '../components/StandardReservePanel.vue': {}, '../dropSacrifices': dropHelpers, '../components/DropSacrificePicker.vue': {}, '../replayCodec': {},
    '../replayNotation': replayNotation, '../replayState': replayState, '../replayAnalysis': replayAnalysis,
    '../moveOptionUi': {}, '../timeControls': {}, '../composables/useDeckCodeCodec': {}, '../replayDeckCode': {},
    '../api/gameApi': { api: {
      createAnalysis: async () => { writes++; return new Promise<AnalysisTree>(resolve => { save = resolve }) },
      getAnalysisOptions: async () => { previews++; return { moves: [], drops: [action('w1')], ability_actions: [], previews: [{ action: action('w1'), state_delta: [], draw_pending: true }] } },
    } },
  }
  const exports: any = {}
  new Function('require', 'exports', replayCompiled)((id: string) => { assert.ok(id in modules, id); return modules[id] }, exports)
  const ui = scope.run(() => exports.default.setup({ record }, { expose() {}, emit() {} }))
  ui.canManage.value = true
  assert.deepEqual(ui.pocketPieces.value.map((p: { id: string }) => p.id), ['w1','w2','w3','w4'])
  assert.equal(ui.currentDraws.value.length, 3)
  assert.match(ui.resolvedPiece('b1'), /나이트.*b1/)
  await ui.selectPiece('w1')
  assert.equal(ui.actionPreviews.value[0].draw_pending, true)
  assert.equal(ui.actionPreviews.value[0].state_hash, undefined)
  const pending = ui.playAnalysisAction(action('w1'))
  assert.equal(ui.committing.value, true)
  assert.equal(ui.activeNode.value, null)
  assert.equal(ui.state.value.players.black.deck.hand_pieces.length, 3)
  await ui.selectPiece('w2'); await ui.playAnalysisAction(action('w2'))
  assert.equal(writes, 1); assert.equal(previews, 1)
  save({ ...tree([resolved]), game_id: record.game_id })
  await pending
  assert.equal(ui.committing.value, false)
  assert.equal(ui.activeNode.value.state_hash, 'exact-server-hash')
  assert.equal(ui.state.value.players.black.deck.hand_pieces.length, 4)
  assert.deepEqual(ui.currentDraws.value[0].piece_ids, ['b4'])
  assert.match(ui.drawSummary(ui.currentDraws.value), /흑 드로우 1기/)
})

test('Draw notation exposes only count; zero results stay silent', () => {
  assert.equal(replayNotation.formatDraw({ player_id: 'black', piece_ids: ['secret-knight'] }), '흑 드로우 1기')
  assert.equal(replayNotation.formatDraw({ player_id: 'white', piece_ids: [] }), '')
})

test('G5 action identity retains sacrifice order and public detail includes exact total', () => {
  const summon: TurnAction = { type: 'extra_summon', player_id: 'white', extra_piece_id: 'extra', sacrifice_piece_ids: ['q3','q1','q2'], target_square: { file: 4, rank: 0 } }
  assert.notEqual(actionIdentity(summon), actionIdentity({ ...summon, sacrifice_piece_ids: ['q1','q2','q3'] }))
  const state = standardRecord().initial_state
  state.pieces.extra = { ...state.pieces.w1, id: 'extra', type_id: 'guhang' }
  for (const id of summon.sacrifice_piece_ids) state.pieces[id] = { ...state.pieces.w1, id, type_id: 'queen', captured: true }
  state.piece_definitions.guhang = { ...state.piece_definitions.knight, name: '구행', score: 25 }
  state.piece_definitions.queen = { ...state.piece_definitions.knight, name: '퀸', score: 9 }
  assert.match(replayNotation.summonDetail(summon, state), /구행 \[25\].*e1.*합계 27점/)
})

const { descriptor: summonDescriptor } = parse(readFileSync(new URL('./components/ExtraSummonPanel.vue', import.meta.url), 'utf8'))
const summonCompiled = ts.transpileModule(compileScript(summonDescriptor, { id: 'summon-panel-test' }).content, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText

test('G5 actual summon panel keeps overpayment, target confirmation, cancellation and opponent exclusion', async t => {
  const scope = vue.effectScope(); t.after(() => scope.stop())
  const state = standardRecord().initial_state
  state.piece_definitions.knight.score = 9
  state.pieces.extra = { ...state.pieces.w1, id: 'extra', type_id: 'knight' }
  state.players.white.deck.extra_deck_pieces = ['extra']
  const selectedRequests: string[][] = [], submitted: TurnAction[] = []
  const props = vue.reactive({ state, viewer: 'white', enabled: true, loadOptions: async (id: string, ids: string[]) => {
    selectedRequests.push([...ids])
    return { sacrifice_piece_ids: ['w1','w2','w3'], policy: { sacrifice_zones: ['hand','board'] }, cost: 25, actions: ids.length === 3 ? [{ player_id: 'white', extra_piece_id: id, sacrifice_piece_ids: [...ids], target_square: { file: 4, rank: 0 } }] : [] }
  }, submit: async (action: TurnAction) => { submitted.push(action) } })
  const exports: any = {}
  const modules: Record<string, unknown> = { vue, '../replayNotation': replayNotation, '../standardGameUi': standardGameUi, '../pieceAssets': { renderedPieceAsset: () => undefined } }
  new Function('require','exports',summonCompiled)((id: string) => { assert.ok(id in modules, id); return modules[id] }, exports)
  const ui = scope.run(() => exports.default.setup(props, { expose() {}, emit() {} }))
  await ui.selectExtra('extra')
  assert.deepEqual(ui.options.value.sacrifice_piece_ids, ['w1','w2','w3'])
  await ui.toggle('w3'); await ui.toggle('w1'); await ui.toggle('w2')
  assert.equal(ui.score.value, 27)
  props.state = structuredClone(vue.toRaw(props.state)); await vue.nextTick()
  assert.equal(ui.extraId.value, 'extra', 'clock-only heartbeat must not reset selection')
  assert.deepEqual(ui.selected.value, ['w3','w1','w2'])
  await ui.confirm(); assert.equal(submitted.length, 0)
  ui.target.value = ui.options.value.actions[0]
  await ui.confirm(); assert.deepEqual((submitted[0] as Extract<TurnAction,{type:'extra_summon'}>).sacrifice_piece_ids, ['w3','w1','w2'])
  assert.equal(ui.extraId.value, null)
  await ui.selectExtra('extra'); ui.cancel(); assert.equal(ui.options.value, null)
  props.enabled = false; await ui.selectExtra('extra'); assert.equal(ui.extraId.value, null)
  assert.deepEqual(selectedRequests[3], ['w3','w1','w2'])
})
