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
      <article v-for="record in records" :key="record.game_id" class="record-row">
        <span><strong>{{ record.players.white.nickname }} vs {{ record.players.black.nickname }}</strong><small>{{ resultLabel(record) }} · {{ timeControlLabel(record.time_control) }} · {{ new Date(record.started_at_ms).toLocaleString() }}</small><small>분석 라인 {{ record.analysis_count }}개 · <b :class="{ expiry: daysLeft(record) <= 3 }">{{ retentionLabel(record) }}</b></small></span>
        <span class="record-actions"><button :disabled="openingId === record.game_id" @click="open(record.game_id)">{{ openingId === record.game_id ? '불러오는 중…' : '복기' }}</button><button class="btn-secondary" @click="toggleRetention(record)">{{ record.retention_mode === 'permanent' ? '★ 영구 저장' : '☆ 영구 저장' }}</button></span>
      </article>
    </section>
  </main>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api } from '../api/gameApi'
import type { GameRecord, GameRecordSummary } from '../types/gameRecord'
import { timeControlLabel } from '../timeControls'

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
function daysLeft(record: GameRecordSummary) { return record.expires_at_ms ? Math.max(0, Math.ceil((record.expires_at_ms - Date.now()) / 86_400_000)) : Infinity }
function originalExpiryPassed(record: GameRecordSummary) { return !!record.ended_at_ms && record.ended_at_ms + 30 * 86_400_000 <= Date.now() }
function retentionLabel(record: GameRecordSummary) { return record.retention_mode === 'permanent' ? '영구 저장됨' : `${daysLeft(record)}일 후 자동 삭제` }
async function toggleRetention(record: GameRecordSummary) {
  const makePermanent = record.retention_mode !== 'permanent'
  if (!makePermanent && originalExpiryPassed(record) && !window.confirm('이 대국은 기본 보존 기간 30일이 이미 지났습니다.\n영구 저장을 해제하면 삭제됩니다.')) return
  try {
    const updated = await api.updateGameRetention(record.game_id, makePermanent)
    record.retention_mode = updated.retention_mode ?? (makePermanent ? 'permanent' : 'auto')
    record.expires_at_ms = updated.expires_at_ms
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause); if (!makePermanent && originalExpiryPassed(record)) records.value = records.value.filter(item => item.game_id !== record.game_id) }
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
.record-actions { display:flex; gap:8px; }.expiry { color:#f0ad4e; }
</style>
