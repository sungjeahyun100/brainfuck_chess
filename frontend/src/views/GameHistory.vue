<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div><p class="eyebrow">Game History</p><h1>게임 기록</h1></div>
    </div>
    <section class="card history-list">
      <p v-if="loading">기록을 불러오는 중…</p>
      <p v-else-if="error" class="error">{{ error }}</p>
      <p v-else-if="!records.length" class="muted-note">저장된 게임 기록이 없습니다.</p>
      <button v-for="record in records" :key="record.game_id" class="record-row" :disabled="openingId === record.game_id" @click="open(record.game_id)">
        <span><strong>{{ record.players.white.nickname }} vs {{ record.players.black.nickname }}</strong><small>{{ new Date(record.started_at_ms).toLocaleString() }} · {{ resultLabel(record) }}</small></span>
        <span>{{ openingId === record.game_id ? '불러오는 중…' : '리플레이' }}</span>
      </button>
    </section>
  </main>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api } from '../api/gameApi'
import type { GameRecord, GameRecordSummary } from '../types/gameRecord'

const emit = defineEmits<{ back: []; loaded: [record: GameRecord] }>()
const records = ref<GameRecordSummary[]>([])
const loading = ref(true)
const openingId = ref<string | null>(null)
const error = ref<string | null>(null)

function resultLabel(record: GameRecordSummary): string {
  if (!record.result) return '진행 중'
  if (!record.result.winner) return '무승부'
  return record.result.winner === record.owner_side ? '승리' : '패배'
}

async function open(gameId: string) {
  openingId.value = gameId; error.value = null
  try { emit('loaded', await api.getGameRecord(gameId)) }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { openingId.value = null }
}

onMounted(async () => {
  try { records.value = await api.listGameRecords() }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { loading.value = false }
})
</script>

<style scoped>
.history-list { max-width: 850px; margin: 24px auto; display: grid; gap: 10px; }
.record-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 14px; text-align: left; border: 1px solid rgba(255,255,255,.1); border-radius: 7px; background: rgba(255,255,255,.04); color: inherit; }
.record-row span:first-child { display: grid; gap: 5px; }.record-row small { color: #a8b1c2; }
</style>
