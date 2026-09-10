<template>
  <section v-if="state.ruleset === 'standard'" class="reserve-panel" :aria-label="`${side === 'white' ? '백' : '흑'} Hand / Pocket`">
    <header><strong>{{ side === 'white' ? '백' : '흑' }} Hand: {{ state.hand_counts?.[side] ?? hand.length }}</strong><small>{{ reveal ? '손패 · 일반 착수' : '상대 손패 · 비공개' }}</small></header>
    <div v-if="reveal" class="reserve-list" aria-label="손패 기물">
      <button v-for="piece in hand" :key="piece.id" type="button" :disabled="!enabled || (summoning && !candidates.includes(piece.id))"
        :aria-pressed="selectedId === piece.id || selectedSacrifices.includes(piece.id)"
        :class="{ selected: selectedId === piece.id || selectedSacrifices.includes(piece.id), candidate: summoning && candidates.includes(piece.id) }"
        @click="$emit('select', piece.id)">
        <img v-if="asset(piece)" :src="asset(piece)" :alt="name(piece)" /><span v-else aria-hidden="true">♟</span>
        <span>{{ name(piece) }}<small>{{ state.piece_definitions[piece.type_id]?.score ?? 0 }}점<span v-if="selectedId === piece.id || selectedSacrifices.includes(piece.id)"> · ✓ 선택</span><span v-else-if="summoning && candidates.includes(piece.id)"> · 제물 후보</span></small></span>
      </button>
      <small v-if="!hand.length">손패가 비어 있습니다.</small>
    </div>
    <small v-if="reveal && !enabled">{{ disabledReason || '현재 턴에는 조작할 수 없습니다.' }}</small>
    <details v-if="reveal"><summary>Pocket: {{ pocket.length }} · 드로우 및 능력용</summary>
      <p>일반 착수는 Hand에서 선택합니다. Pocket은 기존 능력에서 사용합니다.</p>
      <div class="reserve-list"><span v-for="piece in pocket" :key="piece.id" class="pocket-item"><img v-if="asset(piece)" :src="asset(piece)" :alt="name(piece)" /><span>{{ name(piece) }} · {{ state.piece_definitions[piece.type_id]?.score ?? 0 }}점</span></span><small v-if="!pocket.length">Pocket이 비어 있습니다.</small></div>
    </details>
    <small v-else>Pocket: 비공개</small>
  </section>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import type { GameState, Piece, PlayerId } from '../types/game'
import { renderedPieceAsset } from '../pieceAssets'
const props = withDefaults(defineProps<{
  state: GameState; side: PlayerId; reveal: boolean; enabled?: boolean; selectedId?: string | null
  summoning?: boolean; candidates?: string[]; selectedSacrifices?: string[]; disabledReason?: string
}>(), { enabled: false, selectedId: null, summoning: false, candidates: () => [], selectedSacrifices: () => [] })
defineEmits<{ select: [id: string] }>()
// Hidden reserves never become component lists, even if a caller has a fuller view.
const hand = computed(() => props.reveal ? (props.state.players[props.side]?.deck.hand_pieces ?? []).flatMap(id => props.state.pieces[id] ? [props.state.pieces[id]] : []) : [])
const pocket = computed(() => props.reveal ? (props.state.players[props.side]?.deck.pocket_pieces ?? []).flatMap(id => props.state.pieces[id] ? [props.state.pieces[id]] : []) : [])
function name(piece: Piece) { return props.state.piece_definitions[piece.type_id]?.name ?? '기물' }
function asset(piece: Piece) { return renderedPieceAsset(piece, props.state.piece_definitions[piece.type_id]) }
</script>
<style scoped>
.reserve-panel { width: 100%; min-width: 0; box-sizing: border-box; border: 1px solid #46566d; border-radius: 10px; padding: 10px; background: #151e2c; color: #e2e8f0; }
header { display:flex; justify-content:space-between; flex-wrap:wrap; gap:6px; }
small, p { color:#b6c2d2; font-size:12px; }
p { margin:6px 0; }
.reserve-list { display:flex; gap:8px; overflow-x:auto; padding:6px 2px; max-height:150px; }
button, .pocket-item { flex:0 0 auto; display:flex; align-items:center; gap:6px; border:1px solid #53657d; border-radius:6px; background:#253247; color:inherit; padding:6px; }
button { cursor:pointer; } button:disabled { opacity:.6; cursor:default; }
button small { display:block; } img { width:32px; height:32px; object-fit:contain; }
button.selected { border:2px solid #f4cf72; } button.candidate { border-style:dashed; }
button:focus-visible, summary:focus-visible { outline:3px solid #8fc9ff; outline-offset:2px; }
summary { cursor:pointer; margin-top:6px; font-size:13px; }
</style>
