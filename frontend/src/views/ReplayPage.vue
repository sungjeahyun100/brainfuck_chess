<template>
  <main class="replay-page">
    <header class="replay-header">
      <button class="btn-secondary" @click="$emit('close')">로비로</button>
      <div><p class="eyebrow">Read-only Replay</p><h2>{{ record.display_name }}</h2></div>
      <button class="btn-secondary" @click="copyCode">{{ copyStatus }}</button>
    </header>
    <div class="replay-layout">
      <p v-if="compatibilityWarning" class="compatibility-warning">이 게임 기록은 현재 게임 버전과 완전히 호환되지 않을 수 있습니다.</p>
      <section class="replay-board-area">
        <div class="replay-player"><div><strong>{{ record.players.black.nickname }}</strong><small v-if="record.players.black.public_id">@{{ record.players.black.public_id }}</small></div><b>{{ clockText('black') }}</b></div>
        <div class="replay-board-readonly" aria-label="읽기 전용 리플레이 보드">
          <Board :board="state.board" :pieces="state.pieces" :definitions="state.piece_definitions" :selected-piece-id="null" :movable-squares="[]" :attack-squares="[]" :threat-squares="[]" :drop-squares="[]" :last-move="lastMove" orientation="white" :ability-mode="false" />
        </div>
        <div class="replay-player"><div><strong>{{ record.players.white.nickname }}</strong><small v-if="record.players.white.public_id">@{{ record.players.white.public_id }}</small></div><b>{{ clockText('white') }}</b></div>
      </section>
      <aside class="replay-sidebar">
        <section><h3>덱</h3><div class="deck-summary" v-for="side in replaySides" :key="side">
          <strong>{{ side === 'white' ? '백' : '흑' }} · {{ record.decks[side].deck_name }}</strong>
          <small>{{ deploymentText(side) }}</small><small>{{ pocketText(side) }}</small>
          <button class="btn-secondary" :disabled="!canCopyDeck(side)" @click="copyDeck(side)">{{ deckCopyStatus[side] }}</button>
        </div></section>
        <section><h3>기보</h3><div class="notation-list">
          <button class="notation-row" :class="{ active: ply === 0 }" @click="go(0)">0. 시작 위치</button>
          <div v-for="row in notationRows" :key="row.moveNumber" class="notation-full-move"><b>{{ row.moveNumber }}.</b><div class="notation-entries">
            <button v-for="entry in row.entries" :key="entry.ply" class="notation-row" :class="{ active: ply === entry.ply }" @click="go(entry.ply)">
              <span>{{ formatNotation(entry.notation) }}</span><small>{{ duration(entry.elapsed_ms) }}</small>
            </button>
          </div></div>
        </div></section>
        <section class="replay-controls">
          <button @click="go(0)">|◀</button><button @click="go(ply - 1)">◀</button>
          <button @click="toggleAutoplay">{{ playing ? 'Ⅱ' : '▶' }}</button>
          <button @click="go(ply + 1)">▶</button><button @click="go(record.actions.length)">▶|</button>
          <strong>{{ ply }} / {{ record.actions.length }}</strong>
        </section>
        <section class="game-info"><h3>게임 정보</h3><p>{{ timeControlLabel(record.time_control) }}</p><p>{{ record.ruleset_version }} · {{ record.chessembly_version }}</p><p v-if="record.result">{{ record.result.reason }}</p></section>
      </aside>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import Board from '../components/Board.vue'
import { encodeReplayCode } from '../replayCodec'
import { formatNotation, groupNotation, squareName } from '../replayNotation'
import { buildReplayFrames } from '../replayState'
import { timeControlLabel } from '../timeControls'
import type { PlayerId } from '../types/game'
import type { GameRecord } from '../types/gameRecord'
import { encodeDeckCode } from '../composables/useDeckCodeCodec'
import { frozenDeckCodeSource } from '../replayDeckCode'

const props = defineProps<{ record: GameRecord }>()
defineEmits<{ close: [] }>()
const replaySides: PlayerId[] = ['white', 'black']
const ply = ref(0), playing = ref(false), copyStatus = ref('기보 복사')
const deckCopyStatus = ref<Record<PlayerId, string>>({ white: '덱 코드 복사', black: '덱 코드 복사' })
let timer: number | null = null
const frames = computed(() => buildReplayFrames(props.record))
const state = computed(() => frames.value[ply.value])
const activeClock = computed(() => ply.value === 0 ? props.record.initial_clock : props.record.actions[ply.value - 1].clock)
const notationRows = computed(() => groupNotation(props.record.actions))
const compatibilityWarning = computed(() => props.record.format_version !== 2 || props.record.ruleset_version !== 'deck-chess-1' || props.record.chessembly_version !== 'chessembly-1')
const lastMove = computed(() => { const action = ply.value ? props.record.actions[ply.value - 1].action : null; return action?.type === 'move' ? { from: action.from, to: action.to } : null })
function duration(ms: number) { return `${(Math.max(0, ms) / 1000).toFixed(1)}s` }
function deploymentText(side: PlayerId) { return props.record.decks[side].deployments.map(piece => `${piece.piece_name} ${squareName(piece.square)}`).join(', ') || '보드 배치 없음' }
function pocketText(side: PlayerId) { return props.record.decks[side].pocket.map(piece => `${piece.piece_name} x${piece.count}`).join(', ') || '포켓 없음' }
function canCopyDeck(side: PlayerId) { return frozenDeckCodeSource(props.record, side) !== null }
async function copyDeck(side: PlayerId) {
  const source = frozenDeckCodeSource(props.record, side); if (!source) return
  const code = encodeDeckCode(source)
  try { await navigator.clipboard.writeText(code); deckCopyStatus.value[side] = '복사 완료' } catch { deckCopyStatus.value[side] = '복사 실패' }
  window.setTimeout(() => { deckCopyStatus.value[side] = '덱 코드 복사' }, 1800)
}
function clockText(player: PlayerId) { const clock = activeClock.value; const ms = clock.mode === 'countdown' ? (player === 'white' ? clock.white_remaining_ms ?? 0 : clock.black_remaining_ms ?? 0) : (player === 'white' ? clock.white_elapsed_ms : clock.black_elapsed_ms); const seconds = Math.ceil(ms / 1000); return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}` }
function stop() { playing.value = false; if (timer !== null) window.clearInterval(timer); timer = null }
function go(next: number) { ply.value = Math.max(0, Math.min(props.record.actions.length, next)); if (ply.value === props.record.actions.length) stop() }
function toggleAutoplay() { if (playing.value) { stop(); return } playing.value = true; timer = window.setInterval(() => go(ply.value + 1), 900) }
async function copyCode() { try { await navigator.clipboard.writeText(await encodeReplayCode(props.record)); copyStatus.value = '복사 완료' } catch { copyStatus.value = '복사 실패' } window.setTimeout(() => { copyStatus.value = '기보 복사' }, 1800) }
function keydown(event: KeyboardEvent) { const target = event.target as HTMLElement | null; if (target?.matches('input, textarea, select, [contenteditable="true"]')) return; if (event.key === 'ArrowLeft') { event.preventDefault(); go(ply.value - 1) } else if (event.key === 'ArrowRight') { event.preventDefault(); go(ply.value + 1) } }
onMounted(() => { window.addEventListener('keydown', keydown) }); onUnmounted(() => { stop(); window.removeEventListener('keydown', keydown) })
</script>

<style scoped>
.replay-page { padding: 16px; }
.replay-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
.replay-layout { display: grid; grid-template-columns: minmax(0, 1fr) 330px; gap: 16px; align-items: start; }
.compatibility-warning { grid-column: 1/-1; padding: 10px; border: 1px solid #d9a441; border-radius: 8px; color: #f4dfb0; }
.replay-board-area { display: grid; gap: 10px; max-width: 920px; }
.replay-board-readonly { pointer-events: none; }
.replay-player { display: flex; justify-content: space-between; align-items: center; padding: 10px 14px; background: rgba(19,26,39,.92); border: 1px solid rgba(255,255,255,.1); border-radius: 8px; }
.replay-player div { display: grid; }.replay-player small { color: #a8b1c2; }.replay-player b { font: 700 1.5rem ui-monospace, monospace; }
.replay-sidebar { display: grid; gap: 14px; position: sticky; top: 16px; max-height: calc(100vh - 32px); overflow: auto; padding: 14px; background: rgba(19,26,39,.94); border-radius: 10px; }
.notation-list { display: grid; gap: 4px; margin-top: 8px; }.notation-row { display: flex; justify-content: space-between; text-align: left; padding: 8px; border: 0; border-radius: 6px; background: rgba(255,255,255,.05); color: inherit; }.notation-row.active { background: rgba(217,164,65,.25); outline: 1px solid #d9a441; }
.notation-full-move { display: grid; grid-template-columns: 2rem 1fr; gap: 4px; align-items: start; }.notation-full-move > b { padding-top: 8px; }.notation-entries { display: grid; gap: 4px; }.deck-summary { display: grid; gap: 4px; margin-top: 8px; padding: 8px; background: rgba(255,255,255,.04); border-radius: 6px; }.deck-summary small { color: #a8b1c2; }
.replay-controls { display: grid; grid-template-columns: repeat(5, 1fr); gap: 6px; }.replay-controls strong { grid-column: 1/-1; text-align: center; }
.game-info { display: grid; gap: 5px; color: #a8b1c2; }
@media (max-width: 1000px) { .replay-layout { grid-template-columns: 1fr; }.replay-sidebar { position: static; max-height: none; } }
</style>
