<template>
  <section v-if="state.ruleset === 'standard'" class="extra-panel" aria-label="Extra Deck 특수 소환">
    <div v-for="side in sides" :key="side" class="extra-side">
      <strong>{{ side === 'white' ? '백' : '흑' }} Extra Deck · {{ extraPieces(side).length }}기</strong>
      <small>{{ canSelectSide(side) ? '기물을 선택하여 특수 소환' : '공개 정보 · 보기 전용' }}</small>
      <div class="extra-list">
        <button v-for="piece in extraPieces(side)" :key="piece.id" :disabled="!canSelectSide(side) || busy" :aria-pressed="extraId === piece.id" :class="{ selected: extraId === piece.id }" @click="selectExtra(piece.id)">
          <img v-if="asset(piece.id)" :src="asset(piece.id)" :alt="label(piece.id)" />
          <span v-else aria-hidden="true">♟</span>
          <span>{{ label(piece.id) }}<small>남은 {{ extraPieces(side).filter(p => p.type_id === piece.type_id).length }}기<span v-if="extraId === piece.id"> · ✓ 선택</span></small></span>
        </button>
        <small v-if="!extraPieces(side).length">남은 기물이 없습니다.</small>
      </div>
    </div>
    <small v-if="!enabled">{{ disabledReason || '현재 턴에는 소환할 수 없습니다.' }}</small>
    <div v-if="extraId" class="summon-flow">
      <strong>{{ label(extraId) }} 소환</strong>
      <p>1. 제물 선택 · {{ options?.policy.sacrifice_zones.join(' + ') }}<br>점선은 후보, ✓는 선택된 제물입니다.</p>
      <div class="candidates">
        <label v-for="id in options?.sacrifice_piece_ids ?? []" :key="id" class="candidate">
          <input type="checkbox" :checked="selected.includes(id)" :disabled="busy || !enabled" @change="toggle(id)" />
          {{ label(id) }} · {{ state.pieces[id]?.current_square ? `${squareName(state.pieces[id].current_square!)} Board` : 'Hand' }}
        </label>
      </div>
      <p>선택된 제물: {{ selected.map(label).join(', ') || '없음' }}</p>
      <strong aria-live="polite">합계 {{ score }} / 필요 {{ options?.cost ?? '…' }}</strong>
      <p v-if="options && score > options.cost">초과: {{ score - options.cost }}점 · 초과 점수는 반환되지 않습니다.</p>
      <p v-if="options && score < options.cost">제물 점수가 부족합니다.</p>
      <button :disabled="busy || !options?.actions.length" :aria-pressed="stage === 'target'" @click="stage = stage === 'target' ? 'sacrifice' : 'target'">{{ stage === 'target' ? '제물 선택으로 돌아가기' : '2. 소환 위치 선택' }}</button>
      <p v-if="stage === 'target'">보드의 표시된 위치 또는 아래 좌표를 선택하세요.</p>
      <div class="targets" aria-label="합법 소환 위치">
        <button v-for="action in options?.actions ?? []" :key="squareName(action.target_square)" :disabled="busy || !enabled" :aria-pressed="target === action" :class="{ selected: target === action }" @click="chooseTarget(action.target_square)">{{ squareName(action.target_square) }}</button>
      </div>
      <p v-if="target">선택 위치: {{ squareName(target.target_square) }}</p>
      <p v-if="options && score >= options.cost && !options.actions.length">현재 선택으로 소환 가능한 위치가 없습니다.</p>
      <small v-if="!target">소환을 확정하려면 충분한 제물과 위치를 선택하세요.</small>
      <div class="confirm-actions"><button :disabled="busy || !enabled || !target" @click="confirm">3. 소환 확정</button><button @click="cancel">소환 취소</button></div>
    </div>
    <p v-if="busy" aria-live="polite">소환 정보를 처리하는 중입니다.</p>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { ExtraSummonAction, GameState, PlayerId, Square, SummonOptions } from '../types/game'
import { squareName } from '../replayNotation'
import { renderedPieceAsset } from '../pieceAssets'
import { gameplayKey, gameActionError } from '../standardGameUi'
const props = defineProps<{
  state: GameState
  enabled: boolean
  disabledReason?: string
  loadOptions: (id: string, selected: string[]) => Promise<SummonOptions>
  submit: (action: ExtraSummonAction) => Promise<void>
}>()
const emit = defineEmits<{ targets: [squares: Square[]]; active: [active: boolean]; selection: [value: { candidates: string[]; selected: string[]; stage: 'sacrifice' | 'target' }] }>()
const sides: PlayerId[] = ['white', 'black']
const extraId = ref<string | null>(null), selected = ref<string[]>([]), options = ref<SummonOptions | null>(null)
const target = ref<SummonOptions['actions'][number] | null>(null), busy = ref(false), error = ref('')
const stage = ref<'sacrifice' | 'target'>('sacrifice')
let generation = 0
function extraPieces(side: PlayerId) { return (props.state.players[side]?.deck.extra_deck_pieces ?? []).flatMap(id => props.state.pieces[id] ? [props.state.pieces[id]] : []) }
function canSelectSide(side: PlayerId) { return props.enabled && side === props.state.current_player }
function label(id: string) { const p = props.state.pieces[id]; const d = props.state.piece_definitions[p?.type_id ?? '']; return `${d?.name ?? '기물'} [${d?.score ?? 0}]` }
function asset(id: string) { const p = props.state.pieces[id]; return p ? renderedPieceAsset(p, props.state.piece_definitions[p.type_id]) : undefined }
const score = computed(() => selected.value.reduce((sum, id) => sum + (props.state.piece_definitions[props.state.pieces[id]?.type_id ?? '']?.score ?? 0), 0))
watch([options, selected, stage, busy], () => {
  emit('selection', { candidates: options.value?.sacrifice_piece_ids ?? [], selected: [...selected.value], stage: stage.value })
  emit('targets', !busy.value && stage.value === 'target' ? (options.value?.actions ?? []).map(a => a.target_square) : [])
})
function cancel() { generation++; extraId.value = null; selected.value = []; options.value = null; target.value = null; busy.value = false; error.value = ''; stage.value = 'sacrifice'; emit('targets', []); emit('active', false) }
watch(() => gameplayKey(props.state), cancel)
watch(() => props.enabled, enabled => { if (!enabled && !busy.value) cancel() })
async function refresh() {
  if (!extraId.value) return
  const ticket = ++generation
  busy.value = true; error.value = ''; target.value = null; stage.value = 'sacrifice'; emit('targets', [])
  try { const result = await props.loadOptions(extraId.value, [...selected.value]); if (ticket !== generation) return; options.value = result }
  catch (cause) { if (ticket === generation) { options.value = null; error.value = gameActionError(cause) } }
  finally { if (ticket === generation) busy.value = false }
}
async function selectExtra(id: string) {
  if (!props.enabled || busy.value || !extraPieces(props.state.current_player).some(p => p.id === id)) return
  extraId.value = id; selected.value = []; emit('active', true); await refresh()
}
async function toggle(id: string) {
  if (busy.value || !props.enabled || !options.value?.sacrifice_piece_ids.includes(id)) return
  selected.value = selected.value.includes(id) ? selected.value.filter(p => p !== id) : [...selected.value, id]; await refresh()
}
function chooseTarget(square: Square) {
  if (!busy.value && props.enabled) { target.value = options.value?.actions.find(a => a.target_square.file === square.file && a.target_square.rank === square.rank) ?? null; if (target.value) stage.value = 'target' }
}
function chooseBoardSquare(square: Square) {
  if (stage.value === 'target') { chooseTarget(square); return }
  const ids = options.value?.sacrifice_piece_ids.filter(id => { const sq = props.state.pieces[id]?.current_square; return sq?.file === square.file && sq.rank === square.rank }) ?? []
  // A ground/air overlap remains individually selectable in the candidate list.
  if (ids.length === 1) void toggle(ids[0])
}
function chooseBoardPiece(id: string) {
  if (stage.value === 'sacrifice') { void toggle(id); return }
  const square = props.state.pieces[id]?.current_square; if (square) chooseTarget(square)
}
defineExpose({ chooseTarget, chooseBoardSquare, chooseBoardPiece, toggle, cancel })
async function confirm() {
  if (!target.value || !props.enabled || busy.value) return
  const action: ExtraSummonAction = { ...target.value, type: 'extra_summon', sacrifice_piece_ids: [...selected.value] }
  const ticket = ++generation
  busy.value = true; error.value = ''
  try { await props.submit(action); if (ticket === generation) cancel() }
  catch (cause) { if (ticket === generation) error.value = gameActionError(cause) }
  finally { if (ticket === generation) busy.value = false }
}
</script>
<style scoped>
.extra-panel { padding: 12px; border: 1px solid #46566d; border-radius: 10px; background:#151e2c; color:#e2e8f0; min-width:0; }
.extra-side + .extra-side, .summon-flow { border-top:1px solid #46566d; margin-top:10px; padding-top:10px; }
.extra-list { display:flex; overflow-x:auto; gap:6px; padding:6px 2px; }
.extra-list button { flex:0 0 auto; display:flex; align-items:center; gap:6px; }
.extra-panel button { padding: 7px; border:1px solid #53657d; border-radius:6px; background:#253247; color:inherit; cursor:pointer; }
.extra-panel button:disabled { opacity:.55; cursor:default; }
.extra-panel strong, .extra-panel small { display: block; }
.extra-panel small, .extra-panel p { font-size:12px; color:#b6c2d2; }
.extra-panel img { width:32px; height:32px; object-fit:contain; }
.extra-panel .selected { outline: 2px solid #eab308; }
.extra-panel button:focus-visible, input:focus-visible { outline:3px solid #8fc9ff; outline-offset:2px; }
.candidates { max-height:190px; overflow:auto; }
.candidate { display: flex; align-items: center; gap: 4px; margin: 6px 0; }
.targets { display: flex; flex-wrap: wrap; gap:5px; max-height: 120px; overflow: auto; margin:8px 0; }
.confirm-actions { display:flex; gap:8px; margin-top:8px; }
</style>
