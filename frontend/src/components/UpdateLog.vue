<template>
  <dialog
    ref="dialog"
    class="update-log card"
    aria-labelledby="update-log-title"
    aria-describedby="update-log-description"
    @cancel.prevent="dismiss"
  >
    <header class="update-log-header">
      <p class="eyebrow">Deck Chess Updates</p>
      <h2 id="update-log-title">업데이트 로그</h2>
      <p id="update-log-description">{{ automatic ? '새로운 변경 내용을 확인해 보세요.' : '덱체스의 변경 내용을 모아 보세요.' }}</p>
    </header>

    <div class="update-log-entries" tabindex="0" aria-label="업데이트 내역">
      <article v-for="(entry, index) in updateLog" :key="entry.id" class="update-log-entry">
        <div class="update-log-meta">
          <time :datetime="entry.date">{{ entry.date }}</time>
          <span v-if="index === 0" class="update-log-badge">최신 업데이트</span>
        </div>
        <h3>{{ entry.title }}</h3>
        <ul>
          <li v-for="change in entry.changes" :key="change">{{ change }}</li>
        </ul>
      </article>
      <p v-if="updateLog.length === 0">아직 등록된 업데이트가 없습니다.</p>
    </div>

    <footer class="update-log-footer">
      <p v-if="storageUnavailable" role="status">확인 여부를 브라우저에 저장할 수 없어 다음 방문 때 다시 표시될 수 있습니다.</p>
      <p v-else>확인한 업데이트는 이 브라우저에서 다시 자동으로 표시하지 않습니다. 로비에서 언제든 다시 볼 수 있습니다.</p>
      <button class="btn-start" autofocus @click="dismiss">{{ storageUnavailable ? '닫기' : '확인' }}</button>
    </footer>
  </dialog>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { acknowledgeUpdate, readUpdateStatus, updateLog } from '../updateLog'

const dialog = ref<HTMLDialogElement | null>(null)
const automatic = ref(false)
const storageUnavailable = ref(false)
const latest = updateLog[0]

function open() {
  automatic.value = false
  dialog.value?.showModal()
}

function dismiss() {
  if (latest && !storageUnavailable.value && acknowledgeUpdate(latest.id) === 'unavailable') {
    // Keep the notice visible until the user closes it without persistent storage.
    storageUnavailable.value = true
    return
  }
  dialog.value?.close()
}

onMounted(() => {
  if (!latest) return
  const status = readUpdateStatus(latest.id)
  storageUnavailable.value = status === 'unavailable'
  if (status !== 'read') {
    automatic.value = true
    dialog.value?.showModal()
  }
})

defineExpose({ open })
</script>

<style scoped>
.update-log {
  margin: auto;
  padding: 0;
  width: min(640px, calc(100% - 32px));
  max-height: calc(100dvh - 48px);
  color: var(--text);
  background: #131a27;
}
.update-log[open] { display: flex; flex-direction: column; }
.update-log::backdrop { background: rgba(5, 8, 13, 0.8); }
.update-log-header { padding: 24px 24px 18px; border-bottom: 1px solid var(--line); }
.update-log-header h2 { margin: 6px 0 10px; color: #f4dfb0; }
.update-log-header > p:last-child, .update-log-footer p { color: var(--muted); line-height: 1.6; }
.update-log-entries { overflow-y: auto; min-height: 0; padding: 0 24px; overscroll-behavior: contain; }
.update-log-entry { padding: 24px 0; }
.update-log-entry + .update-log-entry { border-top: 1px solid var(--line); }
.update-log-meta { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; color: var(--muted); font-size: 13px; }
.update-log-badge { border-radius: 4px; padding: 3px 8px; background: rgba(217, 164, 65, 0.14); color: #f4dfb0; }
.update-log-entry h3 { margin: 12px 0; font-size: 18px; }
.update-log-entry ul { padding-left: 20px; line-height: 1.7; overflow-wrap: anywhere; }
.update-log-entry li + li { margin-top: 8px; }
.update-log-footer { padding: 18px 24px 24px; border-top: 1px solid var(--line); }
.update-log-footer p { font-size: 13px; margin-bottom: 16px; }
.update-log-footer button { display: block; margin-left: auto; }
</style>
