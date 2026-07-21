import { createApp } from 'vue'
import App from './App.vue'
import { api } from './api/gameApi'
import { applyPieceScores } from './composables/useDeckValidation'

async function bootstrap(): Promise<void> {
  applyPieceScores(await api.getPieceScores())
  createApp(App).mount('#app')
}

void bootstrap().catch((error: unknown) => {
  const root = document.querySelector<HTMLElement>('#app')
  if (root) {
    root.textContent = error instanceof Error
      ? `기물 점수를 불러오지 못했습니다: ${error.message}`
      : '기물 점수를 불러오지 못했습니다.'
  }
})
