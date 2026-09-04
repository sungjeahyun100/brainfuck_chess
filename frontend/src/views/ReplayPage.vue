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
      </section>
      <aside class="replay-sidebar">
        <section><h3>덱</h3><div class="deck-summary" v-for="side in replaySides" :key="side">
          <strong>{{ side === 'white' ? '백' : '흑' }} · {{ record.decks[side].deck_name }}</strong>
          <small>{{ deploymentText(side) }}</small><small>{{ pocketText(side) }}</small>
          <button class="btn-secondary" :disabled="!canCopyDeck(side)" @click="copyDeck(side)">{{ deckCopyLabel(side) }}</button>
        </div></section>
        <section><h3>기보</h3><div class="notation-list">
          <button class="notation-row" :class="{ active: ply === 0 }" @click="go(0)">0. 시작 위치</button>
          <div v-for="row in notationRows" :key="row.moveNumber" class="notation-full-move"><b>{{ row.moveNumber }}.</b><div class="notation-entries">
            <button v-for="entry in row.entries" :key="entry.ply" class="notation-row" :class="{ active: ply === entry.ply }" @click="go(entry.ply)">
              <span>{{ formatNotation(entry.notation) }}</span><small>{{ duration(entry.elapsed_ms) }}</small>
            </button>
          </div></div>
        </div></section>
        <section v-if="canManage" class="analysis-panel"><div class="analysis-title"><h3>분석</h3><button v-if="activeTree" class="btn-secondary" @click="returnToActual">원본으로</button></div><p class="muted-note">원본이나 분석 노드에서 기물을 움직이면 분기가 저장됩니다. 화살표는 오른쪽 드래그로 그립니다.</p><p v-if="analysisError" class="error">{{ analysisError }}</p>
          <div v-if="pocketPieces.length" class="analysis-pocket"><small>{{ state.current_player === 'white' ? '백' : '흑' }} 포켓</small><button v-for="piece in pocketPieces" :key="piece.id" :class="{ active: selectedPieceId === piece.id }" @click="selectPiece(piece.id)">{{ state.piece_definitions[piece.type_id]?.name ?? piece.type_id }}</button></div>
          <div v-if="immediateAbilities.length" class="analysis-pocket"><small>즉시 능력</small><button v-for="action in immediateAbilities" :key="`${action.piece_id}:${action.ability_id}`" @click="playAnalysisAction(action)">{{ action.ability_id }}</button></div>
          <article v-for="tree in trees" :key="tree.id" class="analysis-tree" :class="{ active: activeTree?.id === tree.id }"><div><button @click="openTreeStart(tree)">{{ tree.name }}</button><span><button title="이름 변경" @click="renameTree(tree)">✎</button><button title="variation 삭제" @click="removeTree(tree)">×</button></span></div><div v-for="entry in flattenedNodes(tree)" :key="entry.node.id" class="analysis-node-row"><button class="analysis-node" :style="{ paddingLeft: `${10 + entry.depth * 14}px` }" :class="{ active: activeNode?.id === entry.node.id }" @click="openNode(tree, entry.node)">{{ entry.label }}<small v-if="entry.children > 1"> · {{ entry.children }}개 분기</small></button><button v-if="entry.depth > 0" title="이 노드와 하위 분기 삭제" @click="removeSubtree(tree, entry.node)">×</button></div></article>
        </section>
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
import { computed, onMounted, onUnmounted, ref } from 'vue'
import Board from '../components/Board.vue'
import { encodeReplayCode } from '../replayCodec'
import { formatNotation, groupNotation, squareName } from '../replayNotation'
import { buildReplayFramesResult } from '../replayState'
import { timeControlLabel } from '../timeControls'
import type { PlayerId, Square, TurnAction } from '../types/game'
import type { AnalysisNode, AnalysisTree, GameRecord } from '../types/gameRecord'
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
const deckCopyStatus = ref<Record<PlayerId, string>>({ white: '덱 코드 복사', black: '덱 코드 복사' })
let timer: number | null = null
const replayResult = computed(() => buildReplayFramesResult(props.record))
const actionCount = computed(() => replayResult.value.ok ? props.record.actions.length : 0)
const state = computed(() => activeNode.value?.state_after ?? (replayResult.value.ok ? replayResult.value.frames[activeTree.value?.base_ply ?? ply.value] ?? null : null))
const targetOf = (action: TurnAction) => action.to
const movableSquares = computed(() => legalActions.value.filter(action => action.type === 'move' || action.type === 'ability').map(targetOf).filter((square): square is Square => !!square))
const attackSquares = computed(() => movableSquares.value.filter(square => !!state.value?.board.squares[`${square.file}_${square.rank}`]))
const dropSquares = computed(() => legalActions.value.filter(action => action.type === 'drop').map(action => action.to))
const pocketPieces = computed(() => state.value?.players[state.value.current_player]?.deck.pocket_pieces.map(id => state.value?.pieces[id]).filter((piece): piece is NonNullable<typeof piece> => !!piece) ?? [])
const immediateAbilities = computed(() => legalActions.value.filter((action): action is Extract<TurnAction,{type:'ability'}> => action.type === 'ability' && !action.to))
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
function clearSelection() { selectedPieceId.value = null; legalActions.value = [] }
function go(next: number) { activeTree.value = null; activeNode.value = null; clearSelection(); ply.value = Math.max(0, Math.min(actionCount.value, next)); if (ply.value === actionCount.value) stop() }
function toggleAutoplay() { if (!replayResult.value.ok) return; if (playing.value) { stop(); return } playing.value = true; timer = window.setInterval(() => go(ply.value + 1), 900) }
async function copyCode() { try { await navigator.clipboard.writeText(await encodeReplayCode(props.record)); copyStatus.value = '복사 완료' } catch { copyStatus.value = '복사 실패' } window.setTimeout(() => { copyStatus.value = '기보 복사' }, 1800) }
function position() { return activeTree.value && activeNode.value ? { base_ply: activeTree.value.base_ply, tree_id: activeTree.value.id, node_id: activeNode.value.id } : { base_ply: activeTree.value?.base_ply ?? ply.value } }
async function selectPiece(pieceId: string) { if(!canManage.value)return;const piece=state.value?.pieces[pieceId]; if (!piece || piece.owner !== state.value?.current_player) { clearSelection(); return } analysisError.value=null; try { const options=await api.getAnalysisOptions(props.record.game_id,position(),pieceId); selectedPieceId.value=pieceId; legalActions.value=[...options.moves,...options.drops,...options.ability_actions] } catch(cause){ analysisError.value=cause instanceof Error?cause.message:String(cause);clearSelection() } }
function sameSquare(a:Square|undefined,b:Square){return !!a&&a.file===b.file&&a.rank===b.rank}
function actionChoiceLabel(action:TurnAction){if(action.type==='move')return action.promotion?`승격: ${action.promotion}`:`이동: ${action.move_option_id}`;if(action.type==='ability')return `능력: ${action.ability_id}`;return '포켓 배치'}
function chooseAction(actions:TurnAction[]){if(actions.length<=1)return actions[0];const answer=window.prompt(actions.map((action,index)=>`${index+1}. ${actionChoiceLabel(action)}`).join('\n'));const index=Number(answer)-1;return Number.isInteger(index)?actions[index]:undefined}
async function onSquareClick(square: Square) { if (!state.value) return; const pieceId=state.value.board.squares[`${square.file}_${square.rank}`]; if (!selectedPieceId.value) { if(pieceId) await selectPiece(pieceId); return } const action=chooseAction(legalActions.value.filter(candidate=>sameSquare(candidate.to,square))); if(!action){if(pieceId) await selectPiece(pieceId);else clearSelection();return} await playAnalysisAction(action) }
async function onSquareDrop(square: Square|null,pieceId:string){if(!square)return;await selectPiece(pieceId);await onSquareClick(square)}
async function playAnalysisAction(action: TurnAction) { analysisError.value=null; try { if(!activeTree.value){ const actual=props.record.actions[ply.value]?.action; if(actual&&JSON.stringify(actual)===JSON.stringify(action)){go(ply.value+1);return} const created=await api.createAnalysis(props.record.game_id,ply.value,action);trees.value.push(created);activeTree.value=created;activeNode.value=created.nodes[0]??null } else if(activeNode.value){ const updated=await api.appendAnalysis(props.record.game_id,activeTree.value,activeNode.value.id,action);replaceTree(updated);activeTree.value=updated;activeNode.value=updated.nodes[updated.nodes.length-1]??null } } catch(cause){analysisError.value=cause instanceof Error?cause.message:String(cause)} finally{clearSelection()} }
function replaceTree(tree:AnalysisTree){const index=trees.value.findIndex(item=>item.id===tree.id);if(index>=0)trees.value[index]=tree}
function openTreeStart(tree:AnalysisTree){go(tree.base_ply)}
function openNode(tree:AnalysisTree,node:AnalysisNode){activeTree.value=tree;activeNode.value=node;ply.value=tree.base_ply;clearSelection()}
function returnToActual(){const base=activeTree.value?.base_ply??ply.value;go(base)}
function flattenedNodes(tree:AnalysisTree){const children=(parent:string|null)=>tree.nodes.filter(node=>(node.parent_node_id??null)===parent);const result:Array<{node:AnalysisNode;depth:number;label:string;children:number}>=[];const visit=(node:AnalysisNode,depth:number)=>{const descendants=children(node.id);result.push({node,depth,label:`${depth===0?'└':'├'} ${node.action.type} ${node.action.to? squareName(node.action.to):''}`,children:descendants.length});descendants.forEach(child=>visit(child,depth+1))};children(null).forEach(node=>visit(node,0));return result}
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
.game-info { display: grid; gap: 5px; color: #a8b1c2; }
.analysis-panel { display:grid; gap:8px; }.analysis-title,.analysis-tree>div { display:flex;align-items:center;justify-content:space-between;gap:8px; }.analysis-tree { display:grid;gap:3px;padding:7px;border:1px solid rgba(255,255,255,.08);border-radius:7px; }.analysis-tree.active { border-color:#d9a441; }.analysis-tree button { text-align:left; }.analysis-tree>div>span { display:flex;gap:4px; }.analysis-node-row { display:grid!important;grid-template-columns:1fr auto;gap:3px!important; }.analysis-node { border:0;background:rgba(255,255,255,.04);color:inherit;padding-block:7px; }.analysis-node.active { background:rgba(217,164,65,.25); }.analysis-node small,.muted-note { color:#a8b1c2; }
.analysis-pocket { display:flex;flex-wrap:wrap;gap:5px;align-items:center; }.analysis-pocket button.active { outline:1px solid #d9a441; }
@media (max-width: 1000px) {
  .replay-layout { grid-template-columns: 1fr; }
  .replay-sidebar { position: static; max-height: none; }
}
</style>
