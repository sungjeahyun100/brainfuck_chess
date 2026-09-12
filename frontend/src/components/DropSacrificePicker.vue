<template>
  <section class="drop-sacrifices" aria-label="손패 소환 제물 선택">
    <strong>소환하려면 아군 필드 기물 {{ required }}체를 희생하세요. (킹 제외)</strong>
    <p v-if="!candidates.length">소환 가능한 제물이 없습니다.</p>
    <div>
      <button v-for="id in candidates" :key="id" type="button" :aria-pressed="selected.includes(id)"
        :disabled="!enabled || (!selected.includes(id) && selected.length >= required)"
        @click="$emit('change', selected.includes(id) ? selected.filter(value => value !== id) : [...selected, id])">
        {{ state.piece_definitions[state.pieces[id]?.type_id]?.name ?? id }}
        ({{ squareLabel(id) }}) {{ selected.includes(id) ? '✓' : '' }}
      </button>
    </div>
    <p>{{ selected.length }} / {{ required }}체 선택 · 선택 후 표시된 칸을 누르면 희생과 소환이 확정됩니다.</p>
    <button type="button" :disabled="!enabled" @click="$emit('cancel')">취소</button>
  </section>
</template>
<script setup lang="ts">
import type { GameState } from '../types/game'
const props = defineProps<{ state: GameState; required: number; candidates: string[]; selected: string[]; enabled: boolean }>()
defineEmits<{ change: [ids: string[]]; cancel: [] }>()
function squareLabel(id: string) {
  const p = props.state.pieces[id], sq = p?.current_square
  return sq ? `${String.fromCharCode(97 + sq.file)}${sq.rank + 1}${p.layer === 'air' ? ' 공중' : ''}` : id
}
</script>
<style scoped>
.drop-sacrifices { padding: 12px; border: 1px solid #d7a84a; border-radius: 8px; }
.drop-sacrifices div { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
button { padding: 8px; cursor: pointer; }
button[aria-pressed="true"] { outline: 2px solid #d7a84a; }
button:disabled { opacity: .5; cursor: default; }
</style>
