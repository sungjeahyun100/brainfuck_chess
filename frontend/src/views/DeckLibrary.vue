<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div>
        <p class="eyebrow">Deck Library</p>
        <h1>덱 빌딩</h1>
      </div>
      <button class="btn-start" @click="createDeck">덱 추가 +</button>
    </div>

    <section v-if="decks.length === 0" class="card empty-state">
      <h2>저장된 덱이 없습니다.</h2>
      <p>덱 추가 + 버튼으로 새 덱을 만든 뒤 싱글, 봇, 멀티플레이에서 선택할 수 있습니다.</p>
    </section>

    <section v-else class="deck-grid">
      <article
        v-for="deck in decks"
        :key="deck.id"
        class="card deck-card"
        :class="{ invalid: !summary(deck).valid }"
      >
        <div class="deck-card-main" @click="$emit('edit', deck.id)">
          <p class="summary-title">{{ summary(deck).valid ? '사용 가능' : '수정 필요' }}</p>
          <h2>{{ deck.name }}</h2>
          <p>{{ deck.boardSize }} x {{ deck.boardSize }}</p>
          <strong>{{ summary(deck).totalScore }} / {{ summary(deck).scoreLimit }}점</strong>
          <span>{{ deck.starting.length }} 시작 기물 · {{ totalPocketCount(deck) }} 포켓 기물</span>
          <p class="summary-status">{{ summary(deck).valid ? '유효한 덱입니다.' : summary(deck).errors[0] }}</p>
        </div>
        <div class="deck-card-actions">
          <button class="btn-secondary" @click="$emit('edit', deck.id)">편집</button>
          <button class="btn-secondary" @click="duplicate(deck.id)">복제</button>
          <button class="btn-secondary danger" @click="remove(deck.id)">삭제</button>
        </div>
      </article>
    </section>
  </main>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { SavedDeck } from '../types/deck'
import { createNewSavedDeck, useSavedDecks } from '../composables/useSavedDecks'
import { totalPocketCount, validateSavedDeck } from '../composables/useDeckValidation'

const emit = defineEmits<{
  back: []
  edit: [deckId: string]
}>()

const savedDecks = useSavedDecks()
const decks = ref<SavedDeck[]>([])

function refresh() {
  decks.value = savedDecks.loadDecks()
}

function summary(deck: SavedDeck) {
  return validateSavedDeck(deck)
}

function createDeck() {
  const deck = createNewSavedDeck()
  savedDecks.saveDeck(deck)
  emit('edit', deck.id)
}

function duplicate(id: string) {
  savedDecks.duplicateDeck(id)
  refresh()
}

function remove(id: string) {
  savedDecks.deleteDeck(id)
  refresh()
}

onMounted(refresh)
</script>
