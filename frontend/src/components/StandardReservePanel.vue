<template>
  <section v-if="state.ruleset === 'standard'" class="reserve-panel" :class="`reserve-${presentation}`" :aria-label="`${side === 'white' ? '백' : '흑'} ${presentation === 'deck' ? '덱' : 'Hand / Pocket'}`">
    <header v-if="presentation !== 'deck'"><strong>{{ side === 'white' ? '백' : '흑' }} Hand: {{ handCount }}</strong><slot name="clock"><small>{{ reveal ? '손패 · 일반 착수' : '상대 손패 · 비공개' }}</small></slot></header>
    <button v-if="presentation !== 'hand'" type="button" class="deck-card" :class="{ required: drawRequired }" :disabled="!drawEnabled" @click="$emit('draw')" :aria-label="`덱 ${state.deck_counts?.[side] ?? pocket.length}장`">
      <span aria-hidden="true">▧</span><strong>{{ presentation === 'deck' ? deckLabel : 'DECK' }}</strong><span>{{ state.deck_counts?.[side] ?? (revealPocket ? pocket.length : '비공개') }}장</span>
      <small>{{ drawing && drawRequired ? '드로우 중…' : drawRequired ? '↓ 1장 뽑기' : '대기' }}</small>
    </button>
    <div v-if="reveal && presentation !== 'deck'" class="reserve-list" aria-label="손패 기물">
      <button v-for="piece in hand" :key="piece.id" type="button" :disabled="!enabled || (summoning && !candidates.includes(piece.id))"
        :aria-pressed="selectedId === piece.id || selectedSacrifices.includes(piece.id)"
        :class="{ selected: selectedId === piece.id || selectedSacrifices.includes(piece.id), candidate: summoning && candidates.includes(piece.id) }"
        @click="$emit('select', piece.id)">
        <img v-if="asset(piece)" :src="asset(piece)" :alt="name(piece)" /><span v-else aria-hidden="true">♟</span>
        <span>{{ name(piece) }}<small>{{ state.piece_definitions[piece.type_id]?.score ?? 0 }}점<span v-if="selectedId === piece.id || selectedSacrifices.includes(piece.id)"> · ✓ 선택</span><span v-else-if="summoning && candidates.includes(piece.id)"> · 제물 후보</span></small></span>
      </button>
      <small v-if="!hand.length">손패가 비어 있습니다.</small>
    </div>
    <div v-if="presentation === 'hand' && !reveal" class="reserve-list concealed-hand" :aria-label="`상대 손패 ${handCount}장 · 비공개`">
      <span v-for="index in handCount" :key="index" class="card-back" aria-hidden="true"><span>◇</span></span>
      <small v-if="!handCount">손패가 비어 있습니다.</small>
    </div>
    <small v-if="reveal && !enabled && presentation === 'full'">{{ disabledReason || '현재 턴에는 조작할 수 없습니다.' }}</small>
    <details v-if="revealPocket && presentation !== 'hand'"><summary>Pocket: {{ pocket.length }} · 드로우 및 능력용</summary>
      <p>일반 착수는 Hand에서 선택합니다. Pocket은 기존 능력에서 사용합니다.</p>
      <div class="reserve-list"><span v-for="piece in pocket" :key="piece.id" class="pocket-item"><img v-if="asset(piece)" :src="asset(piece)" :alt="name(piece)" /><span>{{ name(piece) }} · {{ state.piece_definitions[piece.type_id]?.score ?? 0 }}점</span></span><small v-if="!pocket.length">Pocket이 비어 있습니다.</small></div>
    </details>
    <small v-else-if="presentation === 'full'">Pocket: 비공개</small>

  </section>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import type { GameState, Piece, PlayerId } from '../types/game'
import { renderedPieceAsset } from '../pieceAssets'
const props = withDefaults(defineProps<{
  state: GameState; side: PlayerId; reveal: boolean; revealPocket?: boolean; enabled?: boolean; selectedId?: string | null
  presentation?: 'full' | 'hand' | 'deck'; deckLabel?: string;
  drawEnabled?: boolean; drawRequired?: boolean; drawing?: boolean;
  summoning?: boolean; candidates?: string[]; selectedSacrifices?: string[]; disabledReason?: string
}>(), { presentation: 'full', deckLabel: 'DECK', revealPocket: false, enabled: false, selectedId: null, summoning: false, candidates: () => [], selectedSacrifices: () => [] })
defineEmits<{ select: [id: string]; draw: [] }>()
const handCount = computed(() => props.state.hand_counts?.[props.side] ?? props.state.players[props.side]?.deck.hand_pieces?.length ?? 0)
// Hidden reserves never become component lists, even if a caller has a fuller view.
const hand = computed(() => props.reveal ? (props.state.players[props.side]?.deck.hand_pieces ?? []).flatMap(id => props.state.pieces[id] ? [props.state.pieces[id]] : []) : [])
const pocket = computed(() => props.revealPocket ? (props.state.players[props.side]?.deck.pocket_pieces ?? []).flatMap(id => props.state.pieces[id] ? [props.state.pieces[id]] : []) : [])
function name(piece: Piece) { return props.state.piece_definitions[piece.type_id]?.name ?? '기물' }
function asset(piece: Piece) { return renderedPieceAsset(piece, props.state.piece_definitions[piece.type_id]) }
</script>
<style scoped>
.reserve-panel { width: 100%; min-width: 0; box-sizing: border-box; border: 1px solid #46566d; border-radius: 10px; padding: 10px; background: #151e2c; color: #e2e8f0; }
header { display:flex; justify-content:space-between; flex-wrap:wrap; gap:6px; }
small, p { color:#b6c2d2; font-size:12px; }
p { margin:6px 0; }
.reserve-list { display:flex; gap:6px; overflow:auto; padding:6px 2px; max-height:220px; overflow-x:auto; scroll-snap-type:x proximity; }
button, .pocket-item { flex:0 0 100px; min-width:0; display:flex; flex-direction:column; justify-content:space-between; align-items:center; text-align:center; min-height:128px; scroll-snap-align:start; gap:6px; border:1px solid #53657d; border-radius:6px; background:#253247; color:inherit; padding:5px; font-size:12px; line-height:1.2; }
button { cursor:pointer; } button:disabled { opacity:.6; cursor:default; }
button small { display:block; } img { width:48px; height:48px; object-fit:contain; }
button.selected { border:2px solid #f4cf72; } button.candidate { border-style:dashed; }
button:focus-visible, summary:focus-visible { outline:3px solid #8fc9ff; outline-offset:2px; }
summary { cursor:pointer; margin-top:6px; font-size:13px; }
.deck-card { width:104px; min-height:116px; margin:10px auto; border:2px solid #53657d; background:repeating-linear-gradient(45deg,#253247,#253247 8px,#2c3e57 8px,#2c3e57 10px); box-shadow:3px 3px 0 #151e2c,5px 5px 0 #53657d; }
.deck-card.required { border-color:#f4cf72; opacity:1; animation:draw-pulse 1.8s ease-in-out infinite; }
.reserve-list button { animation:card-arrival .35s ease-out; }
@keyframes draw-pulse { 50% { box-shadow:0 0 14px #f4cf7260; } }
@keyframes card-arrival { from { opacity:0; transform:translateY(-12px); } to { opacity:1; transform:translateY(0); } }
@media (prefers-reduced-motion: reduce) { .deck-card.required, .reserve-list button { animation:none; } }

.reserve-hand { display:grid; grid-template-columns:125px minmax(0,1fr); align-items:center; gap:8px; padding:6px 8px; }
.reserve-hand header { flex-direction:column; align-items:flex-start; gap:4px; font-size:13px; }
.reserve-hand .reserve-list { padding:5px 1px 2px; gap:6px; }
.reserve-hand .reserve-list button { flex-basis:70px; min-height:64px; gap:2px; padding:4px; }
.card-back { flex:0 0 70px; height:76px; box-sizing:border-box; display:grid; place-items:center; border:1px solid #8095b3; border-radius:6px; background:repeating-linear-gradient(45deg,#253247,#253247 6px,#344965 6px,#344965 8px); box-shadow:inset 0 0 0 3px #182435,inset 0 0 0 4px #53657d; color:#c8d7ec; font-size:30px; }
.concealed-hand { flex-wrap:wrap; overflow:visible; max-height:none; }
.reserve-hand img { width:32px; height:32px; }
.reserve-hand button:disabled { opacity:.78; }
.reserve-deck { position:relative; padding:0; background:transparent; border:0; }
.reserve-deck .deck-card { width:100%; min-height:110px; margin:0; padding:7px 4px; }
.reserve-deck summary { font-size:11px; }
.reserve-deck details[open] { position:absolute; z-index:10; width:240px; padding:8px; background:#151e2c; border:1px solid #53657d; border-radius:8px; }
@media (max-width:600px), (max-height:700px) {
  .reserve-hand { grid-template-columns:88px minmax(0,1fr); padding:5px 7px; gap:5px; }
  .reserve-hand header { font-size:12px; }
  .reserve-hand .reserve-list button { flex-basis:58px; min-height:64px; font-size:11px; }
  .card-back { flex-basis:38px; height:64px; }
  .reserve-hand img { width:28px; height:28px; }
  .reserve-hand small { font-size:11px; }
  .reserve-deck .deck-card { min-height:88px; font-size:11px; }
}
</style>
