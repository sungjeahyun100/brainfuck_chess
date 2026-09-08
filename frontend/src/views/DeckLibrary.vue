<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div>
        <p class="eyebrow">Deck Library</p>
        <h1>덱 빌딩</h1>
      </div>
      <button class="btn-start"  :disabled="loading || busy || !!error" @click="createDeck">덱 추가 +</button>
    </div>

    <p v-if="loading" role="status">덱을 불러오는 중…</p>
    <p v-if="error" class="error" role="alert">{{ error }} <button class="btn-secondary" @click="savedDecks.loadDecks">다시 불러오기</button></p>
    <section v-if="isAccount && localCount > 0" class="card">
      <p>이 브라우저에 저장된 덱 {{ localCount }}개를 계정으로 가져올 수 있습니다. 원본은 보관되며 같은 내용을 반복해서 가져와도 덮어쓰지 않습니다.</p>
      <button class="btn-secondary" :disabled="busy || loading || !!error" @click="importLocal">{{ busy ? '처리 중…' : '로컬 덱 가져오기' }}</button>
      <p v-if="importResult" role="status">{{ importResult }}</p>
    </section>
    <section v-if="!loading && !error && decks.length === 0" class="card empty-state">
      <h2>저장된 덱이 없습니다.</h2>
      <p>덱 추가 + 버튼으로 새 덱을 만든 뒤 싱글, 봇, 멀티플레이에서 선택할 수 있습니다.</p>
    </section>

    <section v-if="!loading && decks.length > 0" class="deck-grid">
      <article
        v-for="deck in decks"
        :key="deck.id"
        class="card deck-card"
        :class="{ invalid: !summary(deck).valid }"
      >
        <div class="deck-card-main" @click="$emit('edit', deck.id)">
          <p class="summary-title">{{ summary(deck).valid ? '사용 가능' : '수정 필요' }}</p>
          <h2>{{ deck.name }}</h2>
          <p>{{ boardMapLabel(deck.mapId) }}</p>
          <strong>{{ summary(deck).totalScore }} / {{ summary(deck).scoreLimit }}점</strong>
          <span>{{ deck.starting.length }} 시작 기물 · {{ totalPocketCount(deck) }} 포켓 기물</span>
          <p class="summary-status">{{ summary(deck).valid ? '유효한 덱입니다.' : summary(deck).errors[0] }}</p>
        </div>
        <div class="deck-card-actions">
          <button class="btn-secondary" @click="$emit('edit', deck.id)">편집</button>
          <button class="btn-secondary" :disabled="busy || loading" @click="duplicate(deck.id)">복제</button>
          <button class="btn-secondary danger" :disabled="busy || loading" @click="remove(deck.id)">삭제</button>
        </div>
      </article>
    </section>
  </main>
</template>

<script setup lang="ts">
import type { SavedDeck } from '../types/deck'
import { createNewSavedDeck, useSavedDecks } from '../composables/useSavedDecks'
import { totalPocketCount, validateSavedDeck } from '../composables/useDeckValidation'
import { boardMapLabel } from '../boardMaps'

const emit = defineEmits<{
  back: []
  edit: [deckId: string]
}>()

const savedDecks = useSavedDecks()
const { decks, loading, busy, error, isAccount, localCount, importResult } = savedDecks

function summary(deck: SavedDeck) {
  return validateSavedDeck(deck)
}

async function createDeck() {
  try {
    const deck = createNewSavedDeck()
    let suffix = decks.value.length + 1
    while (decks.value.some(item => item.name === `새 덱 ${suffix}`)) suffix += 1
    deck.name = `새 덱 ${suffix}`
    const saved = await savedDecks.saveDeck(deck)
    emit('edit', saved.id)
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}
async function duplicate(id: string) {
  try { await savedDecks.duplicateDeck(id); await savedDecks.loadDecks() }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}
async function remove(id: string) {
  try { await savedDecks.deleteDeck(id); await savedDecks.loadDecks() }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}
async function importLocal() {
  try { await savedDecks.importLocalDecks() }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}
</script>
