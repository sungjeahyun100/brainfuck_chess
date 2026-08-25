<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div><p class="eyebrow">Game Replay</p><h1>기보 / 리플레이</h1></div>
    </div>
    <section class="card replay-import">
      <label for="replay-code">기보 또는 게임 코드를 입력하세요</label>
      <textarea id="replay-code" v-model="code" rows="10" placeholder="DC-G2-..." />
      <p class="replay-note">직접 공유받은 기보 코드는 계정 공개 설정과 관계없이 재생되며, 대국 당시의 닉네임과 공개 ID가 포함될 수 있습니다.</p>
      <button class="btn-start" :disabled="loading || !code.trim()" @click="load">{{ loading ? '검증 중…' : '불러오기' }}</button>
      <p v-if="error" class="error">{{ error }}</p>
    </section>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { decodeReplayCode, type ReplayDecodeError } from '../replayCodec'
import type { GameRecord } from '../types/gameRecord'

const emit = defineEmits<{ back: []; loaded: [record: GameRecord] }>()
const code = ref('')
const loading = ref(false)
const error = ref<string | null>(null)
const messages: Record<ReplayDecodeError, string> = {
  empty: '기보 코드를 입력해 주세요.', too_large: '코드가 허용된 최대 길이를 초과했습니다.',
  invalid_format: 'DC-G2 형식의 기보 코드가 아닙니다.', unsupported_version: '지원하지 않는 기보 버전입니다.',
  invalid_payload: '기보 데이터가 손상되었거나 안전하게 압축 해제할 수 없습니다.', invalid_schema: '기보의 데이터 구조가 올바르지 않습니다.',
}
async function load() {
  loading.value = true; error.value = null
  const decoded = await decodeReplayCode(code.value)
  loading.value = false
  if (!decoded.ok) { error.value = messages[decoded.error]; return }
  emit('loaded', decoded.value)
}
</script>

<style scoped>
.replay-import { max-width: 850px; margin: 24px auto; display: grid; gap: 14px; }
textarea { width: 100%; resize: vertical; padding: 12px; border-radius: 8px; font-family: ui-monospace, monospace; }
.replay-note { margin: 0; color: #a8b1c2; font-size: 12px; line-height: 1.5; }
.record-row { display: flex; justify-content: space-between; gap: 12px; padding: 10px; text-align: left; border: 1px solid rgba(255,255,255,.1); border-radius: 7px; background: rgba(255,255,255,.04); color: inherit; }.record-row small { color: #a8b1c2; }
</style>
