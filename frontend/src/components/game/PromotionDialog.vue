<template>
  <div class="promotion-overlay">
    <div class="promotion-box">
      <h3>기물 승격</h3>
      <p>Pawn이 도착할 기물을 선택하세요.</p>
      <div class="promotion-choices">
        <button
          v-for="choice in request.options"
          :key="choice"
          class="promotion-choice"
          type="button"
          @click="$emit('choose', choice)"
        >
          <img
            v-if="pieceImage(choice, request.owner)"
            class="promotion-choice-image"
            :src="pieceImage(choice, request.owner)"
            :alt="pieceLabel(choice, definitions)"
          />
          <span v-else>{{ pieceSymbol(choice) }}</span>
          <small>{{ pieceLabel(choice, definitions) }}</small>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { GameState } from '../../types/game'
import type { PromotionRequest } from '../../domain/game'
import { pieceImage, pieceLabel, pieceSymbol } from '../../display/pieceDisplay'

defineProps<{
  request: PromotionRequest
  definitions: GameState['piece_definitions']
}>()

defineEmits<{
  choose: [pieceType: string]
  cancel: []
}>()
</script>

<style scoped>
.promotion-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.55);
  display: flex; align-items: center; justify-content: center; z-index: 60;
}
.promotion-box {
  background: white; padding: 24px 32px; border-radius: 12px; text-align: center;
  color: #1f2933; max-width: 320px;
}
.promotion-box h3 { margin: 0 0 4px; }
.promotion-box p { margin: 0 0 16px; color: #52606d; font-size: 14px; }
.promotion-choices { display: flex; gap: 12px; justify-content: center; }
.promotion-choice {
  display: flex; flex-direction: column; align-items: center; gap: 6px;
  background: #f0f4f8; border: 2px solid transparent; border-radius: 10px;
  padding: 10px 12px; cursor: pointer; font-size: 13px; color: #1f2933;
}
.promotion-choice:hover, .promotion-choice:focus-visible {
  border-color: #1976d2; background: #e3f2fd;
}
.promotion-choice-image { width: 48px; height: 48px; }
</style>
