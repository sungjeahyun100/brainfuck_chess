<template>
  <div class="game-over-overlay">
    <div class="game-over-box">
      <h2>Game Over</h2>
      <p v-if="state.result?.winner">
        {{ state.result.winner === 'white' ? '⬜ White' : '⬛ Black' }} wins!
        <br><small>({{ state.result.reason }})</small>
      </p>
      <p v-else>Draw</p>
      <button @click="$emit('restart')">New Game</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { GameState } from '../../types/game'

defineProps<{
  state: GameState
}>()

defineEmits<{
  restart: []
}>()
</script>

<style scoped>
.game-over-overlay {
  position: absolute; inset: 0; background: rgba(0,0,0,0.5);
  display: flex; align-items: center; justify-content: center; z-index: 50;
}
.game-over-box {
  background: white; padding: 32px 48px; border-radius: 12px; text-align: center;
  color: #1f2933;
}
.game-over-box button { margin-top: 16px; padding: 10px 24px; background: #1976d2; color: white; border: none; border-radius: 6px; cursor: pointer; font-size: 16px; }
</style>
