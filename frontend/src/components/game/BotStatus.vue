<template>
  <div class="bot-status" :class="{ thinking: botThinking || botReplaying, failed: Boolean(botError) }" aria-live="polite">
    <div>
      <strong>{{ botStatusTitle }}</strong>
      <small v-if="botReplayMessage">{{ botReplayMessage }}</small>
      <small v-if="lastBotStats">
        최근 탐색 {{ lastBotStats.searched_nodes.toLocaleString() }}노드 · 깊이 {{ lastBotStats.depth_reached }} · {{ lastBotStats.elapsed_ms }}ms
      </small>
      <small v-else-if="!botReplayMessage">{{ playerName(botPlayer) }} 봇 · {{ botDifficultyLabel }}</small>
    </div>
    <button v-if="botError && !botThinking && !botReplaying" @click="$emit('retry')">다시 시도</button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { BotTurnStats, PlayerId } from '../../types/game'

const props = defineProps<{
  botPlayer: PlayerId
  botDifficultyLabel: string
  botThinking: boolean
  botReplaying: boolean
  botError: string | null
  botReplayMessage: string | null
  lastBotStats: BotTurnStats | null
}>()

defineEmits<{
  retry: []
}>()

const botStatusTitle = computed(() => {
  if (props.botThinking && !props.botReplaying) return '봇이 수를 계산하고 있습니다...'
  if (props.botReplaying) return '봇이 수를 두고 있습니다'
  if (props.botError) return '봇 턴 실행 실패'
  return '봇 대전'
})

function playerName(player: PlayerId): string {
  return player === 'white' ? 'White' : 'Black'
}
</script>

<style scoped>
.bot-status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 16px;
  border: 1px solid rgba(217, 164, 65, 0.28);
  border-radius: 10px;
  background: rgba(217, 164, 65, 0.08);
  color: #f4dfb0;
}
.bot-status > div { display: flex; flex-direction: column; gap: 3px; }
.bot-status small { color: #a8b1c2; }
.bot-status.thinking { animation: bot-pulse 1.3s ease-in-out infinite alternate; }
.bot-status.failed { border-color: rgba(255, 125, 125, 0.55); }
.bot-status button {
  padding: 7px 12px;
  border: none;
  border-radius: 6px;
  background: #d9a441;
  color: #221a0d;
  cursor: pointer;
  font-weight: 700;
}
@keyframes bot-pulse {
  from { background: rgba(217, 164, 65, 0.06); }
  to { background: rgba(217, 164, 65, 0.16); }
}
</style>
