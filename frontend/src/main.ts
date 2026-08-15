import { createApp } from 'vue'
import App from './App.vue'
import { api } from './api/gameApi'
import { applyPieceMetadata } from './composables/useDeckValidation'

async function bootstrap(): Promise<void> {
  applyPieceMetadata(await api.getPieceCatalog())
  createApp(App).mount('#app')
}

void bootstrap().catch((error: unknown) => {
  const root = document.querySelector<HTMLElement>('#app')
  if (root) {
    root.textContent = error instanceof Error
      ? `기물 정보를 불러오지 못했습니다: ${error.message}`
      : '기물 정보를 불러오지 못했습니다.'
  }
})
