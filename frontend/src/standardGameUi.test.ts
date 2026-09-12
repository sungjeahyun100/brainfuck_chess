import * as dropHelpers from './dropSacrifices.ts'
import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { compileScript, parse } from '@vue/compiler-sfc'
import ts from 'typescript'
import * as vue from 'vue'
import { renderToString } from '@vue/server-renderer'
import * as helpers from './standardGameUi.ts'
import * as policy from './gameControlPolicy.ts'
import * as notation from './replayNotation.ts'
import type { GameState, Piece, SummonOptions } from './types/game.ts'

function fixture(): GameState {
  const piece = (id: string, owner = 'white', type_id = 'knight', square: unknown = null): Piece => ({ id, owner, type_id, current_square: square, in_pocket: id.endsWith('pocket'), captured: false, layer: 'ground', state: {} }) as Piece
  return {
    id: 'g6', ruleset: 'standard', current_player: 'white', turn_number: 1, phase: 'playing', history: [],
    board: { size: 8, squares: { '3_0': 'wk', '4_0': 'wq', '3_7': 'bk' }, air_squares: {} },
    pieces: Object.fromEntries([piece('wk','white','king',{file:3,rank:0}), piece('bk','black','king',{file:3,rank:7}), piece('wq','white','queen',{file:4,rank:0}), ...['w1','w2','w3','w4','wpocket'].map(id => piece(id)), ...['b1','b2','b3','bpocket'].map(id => piece(id,'black')), piece('we','white','guhang'), piece('be','black','bomber')].map(p=>[p.id,p])),
    players: { white: { deck: { hand_pieces: ['w1','w2','w3','w4'], pocket_pieces: ['wpocket'], extra_deck_pieces: ['we'] } }, black: { deck: { hand_pieces: ['b1','b2','b3'], pocket_pieces: ['bpocket'], extra_deck_pieces: ['be'] } } },
    piece_definitions: { knight: { name:'나이트',score:3, abilities:[] }, king:{name:'킹',score:0,is_king:true,abilities:[]}, queen:{name:'퀸',score:9,abilities:[]},guhang:{name:'구행',score:25,abilities:[]},bomber:{name:'폭격기',score:13,abilities:[]} },
    hand_counts: {white:4,black:3}, clock: { server_now_ms:0, mode:'unlimited', time_control:{type:'unlimited'}, white_elapsed_ms:0,black_elapsed_ms:0 }, player_info:{},
  } as unknown as GameState
}
function compile(name: string, modules: Record<string, unknown>, inlineTemplate = false): any {
  const {descriptor} = parse(readFileSync(new URL(`./components/${name}.vue`, import.meta.url),'utf8'))
  const code = ts.transpileModule(compileScript(descriptor,{id:name,inlineTemplate}).content.replaceAll('import.meta.env.DEV','false'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2020}}).outputText
  const exports: any = {}
  new Function('require','exports',code)((id:string)=>{ assert.ok(id in modules, id); return modules[id] }, exports)
  return exports.default
}
const modules: Record<string, unknown> = {
  vue: {...vue,onUnmounted(){},onMounted(){}}, '../standardGameUi':helpers, '../replayNotation':notation,
  '../pieceAssets':{renderedPieceAsset:()=>undefined,pieceAsset:()=>undefined}, '../gameControlPolicy':policy,
  '../moveOptionUi':{pendingForcedLandingPieceId:()=>null,moveOptionTargets:()=>({legalTargets:[],movable:[],captures:[]})},
  '../composables/useActionTimeline':{},'../timeControls':{timeControlLabel:()=>'',CLOCK_URGENCY_THRESHOLDS_MS:{}},'../replayCodec':{},'../botDebugMetrics':{}, './Board.vue':{},'./ExtraSummonPanel.vue':{},'./StandardReservePanel.vue':{}, '../dropSacrifices': dropHelpers, './DropSacrificePicker.vue': {},
}
function setup(t: any, api: Record<string, unknown> = {}, state=fixture()) {
  const scope=vue.effectScope();t.after(()=>scope.stop())
  const old=Object.getOwnPropertyDescriptor(globalThis,'window')
  Object.defineProperty(globalThis,'window',{configurable:true,value:{setInterval:()=>1,clearInterval(){}}})
  t.after(()=>{if(old)Object.defineProperty(globalThis,'window',old);else Reflect.deleteProperty(globalThis,'window')})
  const props=vue.reactive({state,playMode:'single',localPlayer:null})
  const updates:GameState[]=[]
  const component=compile('GameScreen',{...modules,'../api/gameApi':{api}})
  const ui=scope.run(()=>component.setup(props,{expose(){},emit(name:string,value:GameState){if(name==='stateUpdate')updates.push(value)}}))
  return {props,ui,updates}
}

test('G6 Hand Drop uses server targets and commit; Pocket cannot start Standard Drop',async t=>{
  let calls=0; const state=fixture();const after=structuredClone(state); after.players.white.deck.hand_pieces=['w2','w3','w4'];after.board.squares['2_0']='w1';after.pieces.w1.current_square={file:2,rank:0};after.current_player='black'
  let commit!:(s:GameState)=>void
  const {ui,updates,props}=setup(t,{getLegalDrops:async()=>{calls++;return{drops:[{piece_id:'w1',to:{file:2,rank:0}}]}},submitAction:async(_id:string,a:unknown)=>{assert.deepEqual(a,{type:'drop',piece_id:'w1',to:{file:2,rank:0}});return new Promise<GameState>(r=>commit=r)}},state)
  await ui.onPocketClick('wpocket');assert.equal(calls,0)
  await ui.onHandClick('w1');assert.equal(ui.interactionMode.value,'HandDrop');assert.deepEqual(ui.dropSquares.value,[{file:2,rank:0}])
  const request=ui.onSquareClick({file:2,rank:0});assert.equal(updates.length,0);assert.equal(state.players.white.deck.hand_pieces!.length,4)
  commit(after);await request;props.state=updates[0];await vue.nextTick();assert.equal(ui.reserveActiveSide.value,'black');assert.equal(ui.interactionMode.value,'None');assert.equal(props.state.board.squares['2_0'],'w1')
})

test('G6 clock-only / reordered heartbeat preserves Hand; actual same-turn changes and stale responses clear it',async t=>{
  let resolve!:(v:unknown)=>void
  const {ui,props}=setup(t,{getLegalDrops:()=>new Promise(r=>resolve=r)})
  const pending=ui.onHandClick('w1');props.state={...props.state,clock:{...props.state.clock,server_now_ms:100},pieces:Object.fromEntries(Object.entries(props.state.pieces).reverse())};await vue.nextTick();assert.equal(ui.selectedPocketPieceId.value,'w1')
  props.state={...props.state,board:{...props.state.board,squares:{...props.state.board.squares,'5_0':'wq'}}};await vue.nextTick();resolve({drops:[{piece_id:'w1',to:{file:2,rank:0}}]});await pending;assert.equal(ui.selectedPocketPieceId.value,null);assert.deepEqual(ui.dropSquares.value,[])
})

test('G6 summon activation cancels Hand/Move/Ability drafts and excludes ordinary board clicks',async t=>{
  const {ui}=setup(t,{getLegalDrops:async()=>({drops:[]})})
  await ui.onHandClick('w1');ui.airdropOpen.value=true;ui.sacrificeSelectedIds.value=['wq'];ui.onSummonActive(true)
  assert.equal(ui.selectedPocketPieceId.value,null);assert.equal(ui.airdropOpen.value,false);assert.deepEqual(ui.sacrificeSelectedIds.value,[])
  assert.equal(ui.canUsePlayerControls.value,false);assert.equal(ui.interactionMode.value,'ExtraSummonSacrifice')
  ui.summonSelection.value={candidates:['wq'],selected:['wq'],stage:'target'};assert.equal(ui.interactionMode.value,'ExtraSummonTarget')
  ui.onSummonActive(false); await ui.onHandClick('w1'); ui.onSummonActive(false)
  assert.equal(ui.selectedPocketPieceId.value,'w1','inactive panel cleanup must not erase a new ordinary selection')
})

test('G6 Legacy Pocket still starts and submits Drop with no Hand/Extra HUD',async t=>{
  const state=fixture();state.ruleset='legacy';let submitted=false
  const {ui}=setup(t,{getLegalDrops:async()=>({drops:[{piece_id:'wpocket',to:{file:2,rank:0}}]}),submitAction:async()=>{submitted=true;return state}},state)
  await ui.onPocketClick('wpocket');await ui.onSquareClick({file:2,rank:0});assert.equal(submitted,true);assert.equal(ui.isStandard.value,false)
})

test('G6 reserve DOM shows own names/scores, hides opponent identities and tolerates missing objects',async()=>{
  const component=compile('StandardReservePanel',modules,true)
  const state=fixture();state.piece_definitions.knight.name='OWN_KNIGHT'
  state.pieces.b1.type_id='hidden-custom';state.piece_definitions['hidden-custom']={...state.piece_definitions.knight,name:'SECRET_CUSTOM'}
  let html=await renderToString(vue.createSSRApp(component,{state,side:'white',reveal:true,enabled:true}))
  assert.match(html,/Hand: 4/);assert.match(html,/OWN_KNIGHT/);assert.match(html,/3점/);assert.doesNotMatch(html,/SECRET_CUSTOM|data-piece/)
  html=await renderToString(vue.createSSRApp(component,{state,side:'black',reveal:false}))
  assert.match(html,/Hand: 3/);assert.match(html,/Pocket: 비공개/);assert.doesNotMatch(html,/SECRET_CUSTOM|b1|bpocket|OWN_KNIGHT/)
  delete state.pieces.b1;delete state.pieces.bpocket
  await renderToString(vue.createSSRApp(component,{state,side:'black',reveal:true}))
  state.ruleset='legacy';html=await renderToString(vue.createSSRApp(component,{state,side:'white',reveal:true}));assert.doesNotMatch(html,/Hand|Pocket/)
})

test('G6 summon only toggles authoritative candidates; no King/opponent/Pocket; target and late cancel safe',async t=>{
  const scope=vue.effectScope();t.after(()=>scope.stop());const state=fixture();state.piece_definitions.knight.score=9
  const submitted:unknown[]=[];let queries=0
  const props=vue.reactive({state,viewer:'white',enabled:true,loadOptions:async(id:string,selected:string[]):Promise<SummonOptions>=>{queries++;return {cost:25,policy:{sacrifice_zones:['hand','board']},sacrifice_piece_ids:['w1','w2','wq'],actions:selected.length===3?[{player_id:'white',extra_piece_id:id,sacrifice_piece_ids:[...selected],target_square:{file:4,rank:0}}]:[]}},submit:async(a:unknown)=>{submitted.push(a)}})
  const ui=scope.run(()=>compile('ExtraSummonPanel',modules).setup(props,{expose(){},emit(){}}))
  await ui.selectExtra('be');assert.equal(queries,0)
  await ui.selectExtra('we');for(const id of ['wk','bk','wpocket','be'])await ui.toggle(id);assert.deepEqual(ui.selected.value,[]);assert.equal(queries,1)
  await ui.toggle('w1');await ui.toggle('w2');await ui.toggle('wq');assert.equal(ui.score.value,27);assert.equal(queries,4)
  ui.chooseTarget({file:4,rank:0});assert.equal(ui.stage.value,'target');await ui.confirm();assert.equal(submitted.length,1);assert.equal(ui.extraId.value,null)
  props.loadOptions=()=>new Promise(()=>{});void ui.selectExtra('we');ui.cancel();assert.equal(ui.busy.value,false);assert.equal(ui.extraId.value,null)
})

test('G6 actual draw projection updates counts without client draw; forced landing has no inferred Draw',async t=>{
  const {ui,props}=setup(t)
  assert.equal(ui.viewState.value.hand_counts.white,4);assert.equal(ui.viewState.value.hand_counts.black,3)
  props.state={...props.state,history:[{turn_number:1,player_id:'white',action:{type:'drop',player_id:'white',piece_id:'w1',to:{file:2,rank:0}}}]};await vue.nextTick();assert.equal(ui.viewState.value.current_player,'white');assert.equal(ui.viewState.value.hand_counts.black,3)
  props.state={...props.state,current_player:'black',hand_counts:{white:3,black:4}};await vue.nextTick();assert.equal(ui.viewState.value.hand_counts.black,4)
  assert.doesNotMatch(readFileSync(new URL('./components/GameScreen.vue',import.meta.url),'utf8'),/Math\.random|crypto\.getRandomValues/)
})

test('G6 errors never echo arbitrary internal text',()=>{assert.doesNotMatch(helpers.gameActionError(new Error('secret-id / private stack')),/secret|stack/);assert.equal(helpers.gameActionError(new Error('제물 점수가 소환 비용보다 부족합니다.')),'제물 점수가 부족합니다.')})

test('G6 server refusal preserves valid summon choices; reordered clocks preserve target; gameplay changes cancel', async t => {
  const scope=vue.effectScope();t.after(()=>scope.stop());const state=fixture()
  const props=vue.reactive({state,viewer:'white',enabled:true,loadOptions:async(id:string,ids:string[])=>({cost:9,policy:{sacrifice_zones:['board']},sacrifice_piece_ids:['wq'],actions:ids.length?[{player_id:'white',extra_piece_id:id,sacrifice_piece_ids:ids,target_square:{file:4,rank:0}}]:[]}),submit:async()=>{throw new Error('private internal details')}})
  const ui=scope.run(()=>compile('ExtraSummonPanel',modules).setup(props,{expose(){},emit(){}}))
  await ui.selectExtra('we');await ui.toggle('w1');assert.deepEqual(ui.selected.value,[],'Board-only server candidates exclude Hand')
  await ui.toggle('wq');ui.chooseTarget({file:4,rank:0});const target=ui.target.value
  props.state={...props.state,clock:{...props.state.clock,server_now_ms:400},pieces:Object.fromEntries(Object.entries(props.state.pieces).reverse())};await vue.nextTick()
  assert.equal(ui.target.value,target);await ui.confirm();assert.deepEqual(ui.selected.value,['wq']);assert.equal(ui.target.value,target);assert.doesNotMatch(ui.error.value,/private internal/)
  props.state={...props.state,current_player:'black'};await vue.nextTick();assert.equal(ui.extraId.value,null);assert.equal(ui.target.value,null)
})

test('G6 late ability request cannot open an overlay or submit after summon starts',async t=>{
  const state=fixture();state.piece_definitions.queen.move_options=[{id:'airdrop',kind:'ability',name:'공수',enabled_when:[]}] as never
  let resolve!:(value:unknown)=>void;let submits=0
  const {ui}=setup(t,{getPieceOptions:async(_g:string,_p:string,ability:string)=>ability?new Promise(r=>resolve=r):{moves:[],ability_actions:[]},submitAction:async()=>{submits++}},state)
  await ui.selectBoardPiece('wq');const pending=ui.toggleAbilityMode('airdrop');ui.onSummonActive(true)
  resolve({moves:[],ability_actions:[{type:'ability',piece_id:'wq',ability_id:'airdrop',pocket_piece_id:'wpocket',to:{file:2,rank:0}}]});await pending
  assert.equal(ui.airdropOpen.value,false);assert.equal(ui.selectedPieceId.value,null);assert.equal(submits,0)
})

test('Extra DOM hides opponent deck identities, scores and counts for either viewer',async()=>{
  const state=fixture()
  const component=compile('ExtraSummonPanel',modules,true)
  for(const current_player of ['white','black']) {
    state.current_player=current_player
    for(const viewer of ['white','black',null]) {
      const html=await renderToString(vue.createSSRApp(component,{state,viewer,enabled:true,loadOptions:async()=>({}),submit:async()=>{}}))
      if(viewer==='white') { assert.match(html,/구행 \[25\]/); assert.doesNotMatch(html,/폭격기|흑 Extra Deck/) }
      if(viewer==='black') { assert.match(html,/폭격기 \[13\]/); assert.doesNotMatch(html,/구행|백 Extra Deck/) }
      if(viewer===null) assert.doesNotMatch(html,/구행|폭격기|Extra Deck ·/)
    }
  }
})

test('Shared local hands can be visible while opponent Pocket stays private',async()=>{
  const state=fixture();state.pieces.bpocket.type_id='bomber'
  const html=await renderToString(vue.createSSRApp(compile('StandardReservePanel',modules,true),{state,side:'black',reveal:true,revealPocket:false}))
  assert.match(html,/나이트/);assert.match(html,/Pocket: 비공개/);assert.doesNotMatch(html,/폭격기|13점/)
})

test('Deck viewer follows local turns but stays on human side during bot turns',async t=>{
  const {ui,props}=setup(t)
  assert.equal(ui.deckViewer.value,'white')
  props.state={...props.state,current_player:'black'};await vue.nextTick()
  assert.equal(ui.deckViewer.value,'black');assert.equal(ui.canRevealReserve('white'),true)
  Object.assign(props,{playMode:'bot',localPlayer:'white'});await vue.nextTick()
  assert.equal(ui.deckViewer.value,'white');assert.equal(ui.canRevealReserve('black'),false)
})

test('G6 Standard bot fallback renders only the authoritative final state and no hidden ID label',async t=>{
  const {ui,updates}=setup(t);const final=fixture();final.current_player='black'
  const action={type:'drop',player_id:'black',piece_id:'not-in-projection',to:{file:2,rank:5}}
  assert.match(ui.actionLabel(action),/Hand 착수/);assert.doesNotMatch(ui.actionLabel(action),/not-in-projection/)
  await ui.replayBotTurn([action],final,0);assert.equal(updates[0],final);assert.equal(ui.botReplayState.value,null)
})

test('G8-A bot ExtraSummon label and preview preserve public target without hidden ID fallback',async t=>{
  const {ui}=setup(t)
  const action={type:'extra_summon',player_id:'black',extra_piece_id:'not-in-projection',sacrifice_piece_ids:['hidden-used-id'],target_square:{file:4,rank:6}}
  assert.match(ui.actionLabel(action),/Extra 소환/)
  assert.doesNotMatch(ui.actionLabel(action),/not-in-projection|hidden-used-id/)
  ui.previewBotAction(action)
  assert.deepEqual(ui.botPreviewDropSquares.value,[{file:4,rank:6}])
})

test('manual Draw blocks ordinary controls and rapid clicks commit exactly one server intent', async t => {
  const state = fixture()
  state.global_state = { manual_draw_v1: 1, draw_required: 1 }
  let commits = 0, resolve!: (state: GameState) => void
  const { ui, props, updates } = setup(t, {
    getLegalDrops: async () => { throw new Error('must not query before draw') },
    submitAction: async (_id: string, action: unknown) => {
      commits++
      assert.deepEqual(action, { type: 'draw', turn_number: 1 })
      return new Promise<GameState>(r => { resolve = r })
    },
  }, state)
  assert.equal(ui.canDraw.value, true)
  assert.equal(ui.canUsePlayerControls.value, false)
  assert.equal(ui.canUseSummonControls.value, false)
  await ui.onHandClick('w1')
  assert.equal(ui.selectedPocketPieceId.value, null)
  const pending = ui.drawCard()
  await ui.drawCard()
  assert.equal(commits, 1)
  assert.equal(updates.length, 0)
  const after = structuredClone(state)
  after.global_state!.draw_required = 0
  after.players.white.deck.hand_pieces!.push('wpocket')
  after.players.white.deck.pocket_pieces = []
  resolve(after)
  await pending
  props.state = updates[0]
  await vue.nextTick()
  assert.equal(ui.canDraw.value, false)
  assert.equal(ui.canUsePlayerControls.value, true)
  assert.equal(ui.canUseSummonControls.value, true)
  await ui.drawCard()
  assert.equal(commits, 1)
  props.playMode = 'multiplayer'
  props.localPlayer = 'black'
  props.state.global_state!.draw_required = 1
  assert.equal(ui.canDraw.value, false)
  assert.equal(ui.canUsePlayerControls.value, false)
})

test('draw reserve DOM exposes a highlighted deck, disabled cards and safe opponent count', async () => {
  const component = compile('StandardReservePanel', modules, true)
  const state = fixture()
  state.deck_counts = {white:17,black:12}
  const own = await renderToString(vue.createSSRApp(component,{state,side:'white',reveal:true,enabled:false,drawRequired:true,drawEnabled:true,disabledReason:'먼저 카드를 드로우해야 합니다.'}))
  assert.match(own,/덱 17장/)
  assert.match(own,/deck-card required/)
  assert.match(own,/1장 뽑기/)
  assert.match(own,/먼저 카드를 드로우해야 합니다/)
  const opponent = await renderToString(vue.createSSRApp(component,{state,side:'black',reveal:false}))
  assert.match(opponent,/덱 12장/)
  assert.doesNotMatch(opponent,/bpocket|b1/)
})

test('opponent hand renders count-only card backs without private piece metadata', async () => {
  const state = fixture()
  state.piece_definitions.knight.name = 'SECRET_KNIGHT'
  const component = compile('StandardReservePanel', modules, true)
  const render = () => renderToString(vue.createSSRApp(component, { state, side: 'black', reveal: false, presentation: 'hand' }))
  let html = await render()
  assert.equal((html.match(/class="card-back"/g) ?? []).length, 3)
  assert.match(html, /상대 손패 3장 · 비공개/)
  assert.doesNotMatch(html, /SECRET_KNIGHT|b1|bpocket|<img|<button/)
  state.hand_counts!.black = 0
  html = await render()
  assert.doesNotMatch(html, /class="card-back"/)
  assert.match(html, /손패가 비어 있습니다/)
})

test('hand sacrifice selection gates targets, sends exact bodies and cancels with position changes', async t => {
  const state=fixture();state.piece_definitions.knight.score=10
  const drops=[{type:'drop',player_id:'white',piece_id:'w1',sacrifice_piece_ids:['wq','body'],to:{file:4,rank:0}}]
  let intent: any
  const {ui,props}=setup(t,{getLegalDrops:async()=>({drops}),submitAction:async(_id:string,a:unknown)=>{intent=a;return state}},state)
  await ui.onHandClick('w1')
  assert.equal(ui.dropRequired.value,2);assert.deepEqual(ui.dropSquares.value,[])
  ui.setDropSacrifices(['wk']);assert.deepEqual(ui.dropSacrifices.value,[])
  ui.setDropSacrifices(['wq']);await ui.onSquareClick({file:4,rank:0});assert.equal(intent,undefined)
  ui.setDropSacrifices(['body','wq']);assert.deepEqual(ui.dropSquares.value,[{file:4,rank:0}])
  await ui.onSquareClick({file:4,rank:0})
  assert.deepEqual(intent.sacrifice_piece_ids,['body','wq'])
  assert.deepEqual(ui.dropSacrifices.value,[])
  await ui.onHandClick('w1');ui.setDropSacrifices(['wq'])
  props.state={...state,turn_number:2};await vue.nextTick()
  assert.deepEqual(ui.dropSacrifices.value,[])
})

test('hand sacrifice thresholds preserve Legacy and lower score drops',()=>{
  const state=fixture()
  for(const [score,count] of [[0,0],[4,0],[5,1],[9,1],[10,2],[20,2]]){
    state.piece_definitions.knight.score=score
    assert.equal(dropHelpers.requiredDropSacrifices(state,'w1'),count)
  }
  state.ruleset='legacy';assert.equal(dropHelpers.requiredDropSacrifices(state,'w1'),0)
})
