<template>
  <div class="header">
    <h2>덱체스 <small class="title-en">Deck Chess</small></h2>
    <div class="turn-info">
      <span class="player-badge" :class="`player-${state.current_player}`">
        {{ state.current_player === 'white' ? '⬜ White' : '⬛ Black' }}
      </span>
      <span v-if="localPlayer" class="local-badge" :class="{ waiting: !isMyTurn }">
        {{ isBotTurn ? '봇 턴' : isMyTurn ? '내 턴' : '상대 턴' }}
      </span>
      <span v-if="botPlayer" class="bot-badge">🤖 {{ botDifficultyLabel }}</span>
      <span class="turn-badge">Turn {{ state.turn_number }}</span>
      <span class="mode-badge" v-if="state.turn_state.mode !== 'undecided'">
        {{ state.turn_state.mode === 'move' ? '🏃 이동' : '🎯 포켓 기물 놓기' }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { BotDifficulty, GameState, PlayerId } from '../../types/game'

const props = defineProps<{
  state: GameState
  localPlayer?: PlayerId | null
  botPlayer?: PlayerId | null
  botDifficulty?: BotDifficulty
  isMyTurn: boolean
  isBotTurn: boolean
}>()

const botDifficultyLabel = computed(() => {
  const labels: Record<BotDifficulty, string> = {
    easy: 'Easy',
    normal: 'Normal',
    hard: 'Hard',
  }
  return labels[props.botDifficulty ?? 'normal']
})
</script>

<style scoped>
.header { display: flex; align-items: center; gap: 16px; }
.title-en { font-size: 0.55em; font-weight: 400; opacity: 0.65; margin-left: 6px; }
.player-badge { padding: 4px 10px; border-radius: 6px; font-weight: bold; }
.player-badge.player-white { background: #eee; color: #333; }
.player-badge.player-black { background: #333; color: #eee; }
.turn-badge, .mode-badge { padding: 4px 8px; background: #ddd; color: #1f2933; border-radius: 6px; }
.local-badge {
  padding: 4px 8px;
  background: #e8f5e9;
  color: #256029;
  border-radius: 6px;
  font-weight: 700;
}
.local-badge.waiting {
  background: #fff3cd;
  color: #7a5a00;
}
.bot-badge {
  padding: 4px 8px;
  background: #342a18;
  color: #f4dfb0;
  border: 1px solid rgba(217, 164, 65, 0.38);
  border-radius: 6px;
  font-weight: 700;
}

@media (max-width: 900px) {
  .header {
    flex-wrap: wrap;
  }
  .turn-info {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
}
</style>
