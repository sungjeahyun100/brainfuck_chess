<template>
  <div class="pocket">
    <h4>{{ playerIcon }} {{ playerName }} Pocket</h4>
    <div class="pocket-pieces">
      <div
        v-for="group in groups"
        :key="group.typeId"
        class="pocket-piece-row"
        :class="{ selected: selectedPocketPieceId ? group.pieceIds.includes(selectedPocketPieceId) : false }"
        draggable="true"
        @click="$emit('pieceClick', group.representativeId)"
        @dragstart="$emit('pieceDragStart', $event, group.representativeId)"
        @dragend="$emit('pieceDragEnd')"
      >
        <img
          v-if="pieceImage(group.typeId, player)"
          class="pocket-piece-image"
          :src="pieceImage(group.typeId, player)"
          :alt="`${player} ${group.typeId}`"
          draggable="false"
        />
        <span v-else class="pocket-piece-symbol">{{ pieceSymbol(group.typeId) }}</span>
        <span class="pocket-piece-meta">
          <strong>{{ group.name }}</strong>
          <span class="pocket-count-bar">
            <span :style="{ width: pocketGroupFillWidth(group.count, maxCount) }"></span>
          </span>
        </span>
        <span class="pocket-piece-count">{{ group.count }}</span>
      </div>
    </div>
    <div class="score-info" v-if="deck">
      <span>{{ deck.total_score }} / {{ deck.score_limit }} pts</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Deck, PlayerId } from '../../types/game'
import type { PocketGroup } from '../../domain/game'
import { pieceImage, pieceSymbol } from '../../display/pieceDisplay'

const props = defineProps<{
  player: PlayerId
  groups: PocketGroup[]
  deck?: Deck
  selectedPocketPieceId: string | null
  maxCount: number
}>()

defineEmits<{
  pieceClick: [pieceId: string]
  pieceDragStart: [event: DragEvent, pieceId: string]
  pieceDragEnd: []
}>()

const playerIcon = computed(() => props.player === 'white' ? '⬜' : '⬛')
const playerName = computed(() => props.player === 'white' ? 'White' : 'Black')

function pocketGroupFillWidth(count: number, maxCount: number): string {
  return `${Math.round((count / Math.max(1, maxCount)) * 100)}%`
}
</script>

<style scoped>
.pocket { width: 170px; min-width: 150px; display: flex; flex-direction: column; gap: 8px; }
.pocket h4 { margin: 0; font-size: 14px; }
.pocket-pieces { display: grid; gap: 6px; }
.pocket-piece-row {
  min-height: 48px;
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) 24px;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border: 2px solid #bbb;
  border-radius: 6px;
  cursor: pointer;
  background: #f9f9f9;
  color: #1f2933;
  user-select: none;
}
.pocket-piece-row[draggable="true"] { cursor: grab; }
.pocket-piece-row[draggable="true"]:active { cursor: grabbing; }
.pocket-piece-row.selected { border-color: #4a8fff; background: #e0eeff; }
.pocket-piece-image {
  display: block;
  width: 30px;
  height: 30px;
  object-fit: contain;
}
.pocket-piece-symbol {
  font-size: 22px;
  text-align: center;
}
.pocket-piece-meta {
  min-width: 0;
  display: grid;
  gap: 5px;
}
.pocket-piece-meta strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  line-height: 1;
}
.pocket-count-bar {
  height: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(31, 41, 51, 0.18);
}
.pocket-count-bar > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: #4a8fff;
}
.pocket-piece-count {
  color: #1f2933;
  font-size: 12px;
  font-weight: 800;
  text-align: right;
}
.score-info { font-size: 12px; color: #666; }

@media (max-width: 900px) {
  .pocket {
    order: 2;
    width: min(320px, 100%);
    flex: 1 1 220px;
  }
}
</style>
