<template>
  <main class="replay-page">
    <template v-if="replayResult.ok && state">
    <header class="replay-header">
      <button class="btn-secondary" @click="$emit('close')">로비로</button>
      <div><p class="eyebrow">{{ activeTree ? '분석 Variation' : '실제 대국 메인 라인' }}</p><h2>{{ record.display_name }}</h2></div>
      <div class="header-actions"><button v-if="canManage" class="btn-secondary" @click="toggleRetention">{{ record.retention_mode === 'permanent' ? '★ 영구 저장됨' : '☆ 영구 저장' }}</button><button class="btn-secondary" @click="copyCode">{{ copyStatus }}</button></div>
    </header>
    <div class="replay-layout">
      <p v-if="compatibilityWarning" class="compatibility-warning">이 게임 기록은 현재 게임 버전과 완전히 호환되지 않을 수 있습니다.</p>
      <section class="replay-board-area">
        <div class="replay-player"><div><strong>{{ record.players.black.nickname }}</strong><small v-if="record.players.black.public_id">@{{ record.players.black.public_id }}</small></div><b>{{ clockText('black') }}</b></div>
        <div class="replay-board-readonly" aria-label="읽기 전용 리플레이 보드">
          <Board ref="boardRef" :board="state.board" :pieces="state.pieces" :definitions="state.piece_definitions" :selected-piece-id="selectedPieceId" :movable-squares="movableSquares" :attack-squares="attackSquares" :threat-squares="[]" :drop-squares="dropSquares" :last-move="lastMove" orientation="white" :ability-mode="false" @square-click="onSquareClick" @piece-click="selectPiece" @piece-drag-start="selectPiece" @square-drop="onSquareDrop" />
        </div>
        <div class="replay-player"><div><strong>{{ record.players.white.nickname }}</strong><small v-if="record.players.white.public_id">@{{ record.players.white.public_id }}</small></div><b>{{ clockText('white') }}</b></div>
        <section class="replay-controls" aria-label="리플레이 재생 조작">
          <button @click="go(0)">|◀</button><button @click="go(ply - 1)">◀</button>
          <button @click="toggleAutoplay">{{ playing ? 'Ⅱ' : '▶' }}</button>
          <button @click="go(ply + 1)">▶</button><button @click="go(actionCount)">▶|</button>
          <strong>{{ activeTree ? `${activeTree.name} · ${activeNode ? '분석 수' : '시작점'} (ply ${activeTree.base_ply})` : `${ply} / ${actionCount}` }}</strong>
          <div class="annotation-controls">
            <button type="button" @click="boardRef?.clearAnnotations()">표시 지우기</button>
          </div>
        </section>
        <section v-if="canManage" class="analysis-tools" aria-label="분석 착수">
          <p>기물을 고르고 표시된 칸에 두면 현재 수순에서 분기합니다. 저장 중에도 계속 둘 수 있습니다.</p>
          <div v-if="pocketPieces.length" class="analysis-pocket"><small>{{ state.current_player === 'white' ? '백' : '흑' }} 포켓</small><button v-for="piece in pocketPieces" :key="piece.id" :class="{ active: selectedPieceId === piece.id }" @click="selectPiece(piece.id)">{{ state.piece_definitions[piece.type_id]?.name ?? piece.type_id }}</button></div>
          <div v-if="immediateAbilities.length" class="analysis-pocket"><small>즉시 능력</small><button v-for="action in immediateAbilities" :key="`${action.piece_id}:${action.ability_id}`" @click="playAnalysisAction(action)">{{ action.ability_id }}</button></div>
          <p v-if="analysisError" class="error">{{ analysisError }}</p>
        </section>
      </section>
      <aside class="replay-sidebar">
        <section><h3>덱</h3><div class="deck-summary" v-for="side in replaySides" :key="side">
          <strong>{{ side === 'white' ? '백' : '흑' }} · {{ record.decks[side].deck_name }}</strong>
          <small>{{ deploymentText(side) }}</small><small>{{ pocketText(side) }}</small>
          <button class="btn-secondary" :disabled="!canCopyDeck(side)" @click="copyDeck(side)">{{ deckCopyLabel(side) }}</button>
        </div></section>
        <section><h3>기보</h3><div class="notation-list">
          <button class="notation-row" :class="{ active: !activeTree && ply === 0 }" @click="go(0)">0. 시작 위치</button>
          <article v-for="tree in treesAtPly(0)" :key="tree.id" class="variation-branch">
            <div class="variation-heading"><button @click="openNode(tree, tree.nodes[0])">{{ tree.name }}</button><span><button title="이름 변경" @click="renameTree(tree)">✎</button><button title="분기 삭제" @click="removeTree(tree)">×</button></span></div>
            <div v-for="item in flattenedNodes(tree)" :key="item.node.id" class="variation-row" :style="{ marginLeft: `${item.depth * 14}px` }"><button class="variation-move" :class="{ active: activeNode?.id === item.node.id, pending: item.node.pending }" @click="openNode(tree,item.node)">{{ item.branch }} {{ item.label }}<small v-if="item.node.pending"> · 저장 중</small></button><button v-if="!item.node.pending" class="variation-delete" title="이 수와 하위 분기 삭제" @click="removeSubtree(tree,item.node)">×</button></div>
          </article>
          <div v-for="row in notationRows" :key="row.moveNumber" class="notation-full-move"><b>{{ row.moveNumber }}.</b><div class="notation-entries">
            <template v-for="entry in row.entries" :key="entry.ply">
              <button class="notation-row" :class="{ active: !activeTree && ply === entry.ply }" @click="go(entry.ply)"><span>{{ formatNotation(entry.notation) }}</span><small>{{ duration(entry.elapsed_ms) }}</small></button>
              <article v-for="tree in treesAtPly(entry.ply)" :key="tree.id" class="variation-branch">
                <div class="variation-heading"><button @click="openNode(tree, tree.nodes[0])">{{ tree.name }}</button><span><button title="이름 변경" @click="renameTree(tree)">✎</button><button title="분기 삭제" @click="removeTree(tree)">×</button></span></div>
                <div v-for="item in flattenedNodes(tree)" :key="item.node.id" class="variation-row" :style="{ marginLeft: `${item.depth * 14}px` }"><button class="variation-move" :class="{ active: activeNode?.id === item.node.id, pending: item.node.pending }" @click="openNode(tree,item.node)">{{ item.branch }} {{ item.label }}<small v-if="item.node.pending"> · 저장 중</small></button><button v-if="!item.node.pending" class="variation-delete" title="이 수와 하위 분기 삭제" @click="removeSubtree(tree,item.node)">×</button></div>
              </article>
            </template>
          </div></div>
        </div></section>
        <section class="game-info"><h3>게임 정보</h3><p>{{ timeControlLabel(record.time_control) }}</p><p>{{ record.ruleset_version }} · {{ record.chessembly_version }}</p><p v-if="record.result">{{ record.result.reason }}</p></section>
      </aside>
    </div>
    </template>
    <section v-else class="replay-error" role="alert">
      <p class="eyebrow">Read-only Replay</p>
      <h2>리플레이를 불러올 수 없습니다.</h2>
      <p>리플레이 데이터 복원 중 오류가 발생했습니다.</p>
      <button class="btn-secondary" @click="$emit('close')">로비로</button>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import Board from '../components/Board.vue'
import { encodeReplayCode } from '../replayCodec'
import { formatLiveAction, formatNotation, groupNotation, squareName } from '../replayNotation'
import { applyStateDelta, buildReplayFramesResult } from '../replayState'
import { analysisPosition, reconcileOptimisticNode } from '../replayAnalysis'
import { abilityActionTargetsSquare, abilitySelectionSquares, isImmediateAbilityAction, moveOptionTargets } from '../moveOptionUi'
import { timeControlLabel } from '../timeControls'
import type { PlayerId, Square, TurnAction } from '../types/game'
import type { AnalysisActionPreview, AnalysisNode, AnalysisTree, GameRecord } from '../types/gameRecord'
import { api } from '../api/gameApi'
import { encodeDeckCode } from '../composables/useDeckCodeCodec'
import { frozenDeckCodeSource } from '../replayDeckCode'

const props = defineProps<{ record: GameRecord }>()
const emit = defineEmits<{ close: [] }>()
const replaySides: PlayerId[] = ['white', 'black']
const ply = ref(0), playing = ref(false), copyStatus = ref('기보 복사')
const boardRef = ref<InstanceType<typeof Board> | null>(null)
const canManage = ref(false)
const trees = ref<AnalysisTree[]>([]), activeTree = ref<AnalysisTree | null>(null), activeNode = ref<AnalysisNode | null>(null)
const selectedPieceId = ref<string | null>(null), legalActions = ref<TurnAction[]>([]), analysisError = ref<string | null>(null)
const actionPreviews = ref<AnalysisActionPreview[]>([])
const deckCopyStatus = ref<Record<PlayerId, string>>({ white: '덱 코드 복사', black: '덱 코드 복사' })
let timer: number | null = null
let localSequence = 0
let persistenceQueue = Promise.resolve()
const replayResult = computed(() => buildReplayFramesResult(props.record))
const actionCount = computed(() => replayResult.value.ok ? props.record.actions.length : 0)
const state = computed(() => activeNode.value?.state_after ?? (replayResult.value.ok ? replayResult.value.frames[activeTree.value?.base_ply ?? ply.value] ?? null : null))
const targetGroups = computed(() => moveOptionTargets(
  legalActions.value.filter((action): action is Extract<TurnAction,{type:'move'}> => action.type === 'move'),
  legalActions.value.filter((action): action is Extract<TurnAction,{type:'ability'}> => action.type === 'ability'),
))
const abilitySelfTargets = computed(() => abilitySelectionSquares(
  legalActions.value.filter((action): action is Extract<TurnAction,{type:'ability'}> => action.type === 'ability'),
  selectedPieceId.value ? state.value?.pieces[selectedPieceId.value]?.current_square : undefined,
))
const movableSquares = computed(() => [...targetGroups.value.movable, ...abilitySelfTargets.value])
const attackSquares = computed(() => targetGroups.value.captures)
const dropSquares = computed(() => legalActions.value.filter(action => action.type === 'drop').map(action => action.to))
const pocketPieces = computed(() => state.value?.players[state.value.current_player]?.deck.pocket_pieces.map(id => state.value?.pieces[id]).filter((piece): piece is NonNullable<typeof piece> => !!piece) ?? [])
const immediateAbilities = computed(() => legalActions.value.filter((action): action is Extract<TurnAction,{type:'ability'}> => action.type === 'ability' && isImmediateAbilityAction(action)))
const deckCodeSources = computed(() => ({
  white: frozenDeckCodeSource(props.record, 'white'),
  black: frozenDeckCodeSource(props.record, 'black'),
}))
const activeClock = computed(() => ply.value === 0 ? props.record.initial_clock : props.record.actions[ply.value - 1].clock)
const notationRows = computed(() => groupNotation(props.record.actions))
const compatibilityWarning = computed(() => props.record.format_version !== 2 || props.record.ruleset_version !== 'deck-chess-1' || props.record.chessembly_version !== 'chessembly-1')
const lastMove = computed(() => { const action = activeNode.value?.action ?? (ply.value ? props.record.actions[ply.value - 1].action : null); return action?.type === 'move' ? { from: action.from, to: action.to } : null })
function duration(ms: number) { return `${(Math.max(0, ms) / 1000).toFixed(1)}s` }
function deploymentText(side: PlayerId) { return props.record.decks[side].deployments.map(piece => `${piece.piece_name} ${squareName(piece.square)}`).join(', ') || '보드 배치 없음' }
function pocketText(side: PlayerId) { return props.record.decks[side].pocket.map(piece => `${piece.piece_name} x${piece.count}`).join(', ') || '포켓 없음' }
function canCopyDeck(side: PlayerId) { return deckCodeSources.value[side] !== null }
function deckCopyLabel(side: PlayerId) { return canCopyDeck(side) ? deckCopyStatus.value[side] : '덱 코드 복사 불가' }
async function copyDeck(side: PlayerId) {
  const source = deckCodeSources.value[side]; if (!source) return
  try { await navigator.clipboard.writeText(encodeDeckCode(source)); deckCopyStatus.value[side] = '복사 완료' } catch { deckCopyStatus.value[side] = '복사 실패' }
  window.setTimeout(() => { deckCopyStatus.value[side] = '덱 코드 복사' }, 1800)
}
function clockText(player: PlayerId) { const clock = activeClock.value; const ms = clock.mode === 'countdown' ? (player === 'white' ? clock.white_remaining_ms ?? 0 : clock.black_remaining_ms ?? 0) : (player === 'white' ? clock.white_elapsed_ms : clock.black_elapsed_ms); const seconds = Math.ceil(ms / 1000); return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}` }
function stop() { playing.value = false; if (timer !== null) window.clearInterval(timer); timer = null }
function clearSelection() { selectedPieceId.value = null; legalActions.value = []; actionPreviews.value = [] }
function go(next: number) { activeTree.value = null; activeNode.value = null; clearSelection(); ply.value = Math.max(0, Math.min(actionCount.value, next)); if (ply.value === actionCount.value) stop() }
function toggleAutoplay() { if (!replayResult.value.ok) return; if (playing.value) { stop(); return } playing.value = true; timer = window.setInterval(() => go(ply.value + 1), 900) }
async function copyCode() { try { await navigator.clipboard.writeText(await encodeReplayCode(props.record)); copyStatus.value = '복사 완료' } catch { copyStatus.value = '복사 실패' } window.setTimeout(() => { copyStatus.value = '기보 복사' }, 1800) }
function position() {
  return analysisPosition(activeTree.value,activeNode.value,ply.value)
}
async function selectPiece(pieceId: string) {
  if(!canManage.value)return
  const piece=state.value?.pieces[pieceId]
  if (!piece || piece.owner !== state.value?.current_player) { clearSelection(); return }
  const started=performance.now();analysisError.value=null; selectedPieceId.value=pieceId; legalActions.value=[]; actionPreviews.value=[]
  try {
    const options=await api.getAnalysisOptions(props.record.game_id,position(),pieceId)
    if (selectedPieceId.value !== pieceId) return
    legalActions.value=[...options.moves,...options.drops,...options.ability_actions]
    actionPreviews.value=options.previews
    if(import.meta.env.DEV)console.debug('analysis_selection_timing',{networkAndLegalMs:performance.now()-started,actions:legalActions.value.length})
  } catch(cause){ analysisError.value=cause instanceof Error?cause.message:String(cause);clearSelection() }
}
function sameSquare(a:Square|undefined,b:Square){return !!a&&a.file===b.file&&a.rank===b.rank}
function actionChoiceLabel(action:TurnAction){if(action.type==='move')return action.promotion?`승격: ${action.promotion}`:`이동: ${action.move_option_id}`;if(action.type==='ability')return `능력: ${action.ability_id}`;return '포켓 배치'}
function chooseAction(actions:TurnAction[]){if(actions.length<=1)return actions[0];const answer=window.prompt(actions.map((action,index)=>`${index+1}. ${actionChoiceLabel(action)}`).join('\n'));const index=Number(answer)-1;return Number.isInteger(index)?actions[index]:undefined}
async function onSquareClick(square: Square) { if (!state.value) return; const pieceId=state.value.board.squares[`${square.file}_${square.rank}`]; if (!selectedPieceId.value) { if(pieceId) await selectPiece(pieceId); return } const actorSquare=state.value.pieces[selectedPieceId.value]?.current_square;const action=chooseAction(legalActions.value.filter(candidate=>sameSquare(candidate.to,square)||(candidate.type==='ability'&&abilityActionTargetsSquare(candidate,actorSquare,square)))); if(!action){if(pieceId) await selectPiece(pieceId);else clearSelection();return} await playAnalysisAction(action) }
async function onSquareDrop(square: Square|null,pieceId:string){if(!square)return;await selectPiece(pieceId);await onSquareClick(square)}
function sameAction(left:TurnAction,right:TurnAction){return JSON.stringify(left)===JSON.stringify(right)}
function previewFor(action:TurnAction){return actionPreviews.value.find(item=>sameAction(item.action,action))}
function enqueuePersistence(job:()=>Promise<void>, basePly:number){
  persistenceQueue=persistenceQueue.then(job).catch(async cause=>{
    analysisError.value=cause instanceof Error?cause.message:String(cause)
    try { trees.value=await api.listAnalysis(props.record.game_id) } catch { trees.value=[] }
    activeTree.value=null;activeNode.value=null;ply.value=basePly;clearSelection()
  })
}
function replaceLocalNode(tree:AnalysisTree,localId:string,persisted:AnalysisNode){
  const replacement=reconcileOptimisticNode(tree,localId,persisted)
  if(activeNode.value?.id===localId&&replacement)activeNode.value=replacement
}
async function playAnalysisAction(action: TurnAction) {
  const interactionStarted=performance.now()
  analysisError.value=null
  const preview=previewFor(action)
  if(!preview){analysisError.value='분석 수의 local preview를 확인할 수 없습니다.';clearSelection();return}
  if(!state.value){analysisError.value='현재 분석 상태를 확인할 수 없습니다.';clearSelection();return}
  const localState=applyStateDelta(state.value,preview.state_delta)
  if(!activeTree.value){
    const actual=props.record.actions[ply.value]?.action
    if(actual&&sameAction(actual,action)){go(ply.value+1);return}
    const basePly=ply.value,now=Date.now(),localId=`local-node-${++localSequence}`
    const localNode:AnalysisNode={id:localId,parent_node_id:null,action,state_after:localState,state_hash:preview.state_hash,created_at_ms:now,pending:true}
    const tree:AnalysisTree={id:`local-tree-${localSequence}`,game_id:props.record.game_id,name:`Variation ${trees.value.length+1}`,base_ply:basePly,version:0,created_at_ms:now,updated_at_ms:now,nodes:[localNode]}
    trees.value.push(tree);activeTree.value=tree;activeNode.value=localNode;const trackedTree=activeTree.value;clearSelection();void nextTick(()=>{if(import.meta.env.DEV)console.debug('analysis_optimistic_timing',{boardUpdateAndRenderMs:performance.now()-interactionStarted})})
    enqueuePersistence(async()=>{
      const writeStarted=performance.now()
      const created=await api.createAnalysis(props.record.game_id,basePly,action)
      trackedTree.id=created.id;trackedTree.name=created.name;trackedTree.version=created.version;trackedTree.created_at_ms=created.created_at_ms;trackedTree.updated_at_ms=created.updated_at_ms
      replaceLocalNode(trackedTree,localId,created.nodes[0])
      if(import.meta.env.DEV)console.debug('analysis_persistence_timing',{kind:'create',networkAndServerMs:performance.now()-writeStarted})
    },basePly)
    return
  }
  const tree=activeTree.value,parent=activeNode.value
  if(!parent)return
  const basePly=tree.base_ply,localId=`local-node-${++localSequence}`
  const localNode:AnalysisNode={id:localId,parent_node_id:parent.id,action,state_after:localState,state_hash:preview.state_hash,created_at_ms:Date.now(),pending:true}
  tree.nodes.push(localNode);activeNode.value=localNode;clearSelection();void nextTick(()=>{if(import.meta.env.DEV)console.debug('analysis_optimistic_timing',{boardUpdateAndRenderMs:performance.now()-interactionStarted})})
  enqueuePersistence(async()=>{
    const currentParent=tree.nodes.find(item=>item.id===localId)?.parent_node_id
    if(!currentParent||currentParent.startsWith('local-'))throw new Error('분석 부모 노드가 아직 저장되지 않았습니다.')
    const writeStarted=performance.now();const result=await api.appendAnalysis(props.record.game_id,tree,currentParent,action)
    tree.version=result.version;tree.updated_at_ms=result.updated_at_ms;replaceLocalNode(tree,localId,result.node)
    if(import.meta.env.DEV)console.debug('analysis_persistence_timing',{kind:'append',networkAndServerMs:performance.now()-writeStarted})
  },basePly)
}
function replaceTree(tree:AnalysisTree){const index=trees.value.findIndex(item=>item.id===tree.id);if(index>=0)trees.value[index]=tree}
function openNode(tree:AnalysisTree,node:AnalysisNode){activeTree.value=tree;activeNode.value=node;ply.value=tree.base_ply;clearSelection()}
function returnToActual(){const base=activeTree.value?.base_ply??ply.value;go(base)}
function treesAtPly(value:number){return trees.value.filter(tree=>tree.base_ply===value)}
function nodeLabel(tree:AnalysisTree,node:AnalysisNode){const parent=node.parent_node_id?tree.nodes.find(item=>item.id===node.parent_node_id):undefined;const before=parent?.state_after??(replayResult.value.ok?replayResult.value.frames[tree.base_ply]:undefined)??node.state_after;return formatLiveAction(node.action,before,before.turn_number)}
function flattenedNodes(tree:AnalysisTree){const children=(parent:string|null)=>tree.nodes.filter(node=>(node.parent_node_id??null)===parent);const result:Array<{node:AnalysisNode;depth:number;label:string;branch:string}>=[];const visit=(node:AnalysisNode,depth:number)=>{const siblings=children(node.parent_node_id??null);result.push({node,depth,label:nodeLabel(tree,node),branch:siblings.length>1?'├─':'└─'});children(node.id).forEach(child=>visit(child,depth+1))};children(null).forEach(node=>visit(node,0));return result}
async function renameTree(tree:AnalysisTree){const name=window.prompt('Variation 이름',tree.name)?.trim();if(!name||name===tree.name)return;try{const updated=await api.renameAnalysis(props.record.game_id,tree,name);replaceTree(updated);if(activeTree.value?.id===tree.id)activeTree.value=updated}catch(cause){analysisError.value=cause instanceof Error?cause.message:String(cause)}}
async function removeTree(tree:AnalysisTree){if(!window.confirm(`"${tree.name}" variation과 모든 하위 분기를 삭제할까요?`))return;try{await api.deleteAnalysis(props.record.game_id,tree.id);trees.value=trees.value.filter(item=>item.id!==tree.id);if(activeTree.value?.id===tree.id)returnToActual()}catch(cause){analysisError.value=cause instanceof Error?cause.message:String(cause)}}
async function removeSubtree(tree:AnalysisTree,node:AnalysisNode){if(!window.confirm('이 수와 모든 하위 분기를 삭제할까요?'))return;try{const updated=await api.deleteAnalysisSubtree(props.record.game_id,tree,node.id);replaceTree(updated);if(activeNode.value?.id===node.id){activeTree.value=updated;activeNode.value=updated.nodes.find(item=>item.id===node.parent_node_id)??null}}catch(cause){analysisError.value=cause instanceof Error?cause.message:String(cause)}}
async function toggleRetention(){const permanent=props.record.retention_mode!=='permanent';const originalExpiry=(props.record.ended_at_ms??Infinity)+30*86_400_000;if(!permanent&&originalExpiry<=Date.now()&&!window.confirm('이 대국은 기본 보존 기간 30일이 이미 지났습니다.\n영구 저장을 해제하면 삭제됩니다.'))return;try{const updated=await api.updateGameRetention(props.record.game_id,permanent);props.record.retention_mode=updated.retention_mode;props.record.expires_at_ms=updated.expires_at_ms}catch(cause){if(!permanent&&originalExpiry<=Date.now()){emit('close');return}analysisError.value=cause instanceof Error?cause.message:String(cause)}}
function keydown(event: KeyboardEvent) { const target = event.target as HTMLElement | null; if (target?.matches('input, textarea, select, [contenteditable="true"]')) return; if (event.key === 'ArrowLeft') { event.preventDefault(); go(ply.value - 1) } else if (event.key === 'ArrowRight') { event.preventDefault(); go(ply.value + 1) } }
onMounted(async () => { window.addEventListener('keydown', keydown); try { trees.value=await api.listAnalysis(props.record.game_id);canManage.value=true } catch { trees.value=[] } }); onUnmounted(() => { stop(); window.removeEventListener('keydown', keydown) })
</script>

<style scoped>
.replay-page { padding: 16px; }
.replay-error { display: grid; justify-items: start; gap: 12px; max-width: 620px; margin: 12vh auto 0; padding: 28px; border: 1px solid rgba(255,255,255,.12); border-radius: 10px; background: rgba(19,26,39,.94); }
.replay-error p { color: #a8b1c2; }
.replay-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
.header-actions { display:flex; gap:8px; }
.replay-layout { display: grid; grid-template-columns: minmax(0, 1fr) 330px; gap: 16px; align-items: start; }
.compatibility-warning { grid-column: 1/-1; padding: 10px; border: 1px solid #d9a441; border-radius: 8px; color: #f4dfb0; }
.replay-board-area { display: grid; gap: 10px; max-width: 920px; }
.replay-board-readonly { pointer-events: auto; }
.replay-player { display: flex; justify-content: space-between; align-items: center; padding: 10px 14px; background: rgba(19,26,39,.92); border: 1px solid rgba(255,255,255,.1); border-radius: 8px; }
.replay-player div { display: grid; }.replay-player small { color: #a8b1c2; }.replay-player b { font: 700 1.5rem ui-monospace, monospace; }
.replay-sidebar { display: grid; gap: 14px; position: sticky; top: 16px; max-height: calc(100vh - 32px); overflow: auto; padding: 14px; background: rgba(19,26,39,.94); border-radius: 10px; }
.notation-list { display: grid; gap: 4px; margin-top: 8px; }.notation-row { display: flex; justify-content: space-between; text-align: left; padding: 8px; border: 0; border-radius: 6px; background: rgba(255,255,255,.05); color: inherit; }.notation-row.active { background: rgba(217,164,65,.25); outline: 1px solid #d9a441; }
.notation-full-move { display: grid; grid-template-columns: 2rem 1fr; gap: 4px; align-items: start; }.notation-full-move > b { padding-top: 8px; }.notation-entries { display: grid; gap: 4px; }.deck-summary { display: grid; gap: 4px; margin-top: 8px; padding: 8px; background: rgba(255,255,255,.04); border-radius: 6px; }.deck-summary small { color: #a8b1c2; }
.replay-controls { width: 100%; display: grid; grid-template-columns: repeat(5, 1fr); gap: 6px; }.replay-controls strong { grid-column: 1/-1; text-align: center; }
.replay-controls button { min-height: 44px; }
.annotation-controls { grid-column: 1/-1; display: grid; }
.analysis-tools { display:grid;gap:8px;padding:10px 12px;border:1px solid rgba(217,164,65,.28);border-radius:8px;background:rgba(19,26,39,.82); }
.analysis-tools p { margin:0;color:#a8b1c2;font-size:12px; }
.game-info { display: grid; gap: 5px; color: #a8b1c2; }
.analysis-pocket { display:flex;flex-wrap:wrap;gap:5px;align-items:center; }.analysis-pocket button.active { outline:1px solid #d9a441; }
.variation-branch { display:grid;gap:3px;margin:2px 0 4px 8px;padding:5px 5px 5px 9px;border-left:2px solid rgba(217,164,65,.55);background:rgba(255,255,255,.025); }
.variation-heading { display:flex;justify-content:space-between;gap:5px; }.variation-heading>button { color:#d9a441;background:transparent;border:0;text-align:left; }.variation-heading span { display:flex;gap:3px; }.variation-heading span button { min-width:28px; }
.variation-row { display:grid;grid-template-columns:minmax(0,1fr) auto;gap:3px; }.variation-move { border:0;border-radius:5px;background:rgba(255,255,255,.05);color:inherit;padding:7px;text-align:left; }.variation-move.active { background:rgba(217,164,65,.25);outline:1px solid #d9a441; }.variation-move.pending { opacity:.75; }.variation-move small { color:#a8b1c2; }.variation-delete { min-width:28px; }
@media (max-width: 1000px) {
  .replay-layout { grid-template-columns: 1fr; }
  .replay-sidebar { position: static; max-height: none; }
}
</style>
