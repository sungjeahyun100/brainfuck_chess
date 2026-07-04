<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div>
        <p class="eyebrow">{{ mode === 'bot' ? 'Bot Match' : 'Single Play' }}</p>
        <h1>{{ mode === 'bot' ? '봇 플레이 덱 선택' : '싱글플레이 덱 선택' }}</h1>
      </div>
      <button class="btn-start" :disabled="!canStart" @click="start">
        {{ mode === 'bot' ? '봇 대전 시작' : '게임 시작' }}
      </button>
    </div>

    <section v-if="decks.length === 0" class="card empty-state">
      <h2>먼저 덱을 만들어 주세요.</h2>
      <p>저장된 덱이 없으면 게임을 시작할 수 없습니다.</p>
      <button class="btn-start" @click="$emit('deck-building')">덱 빌딩으로 이동</button>
    </section>

    <template v-else>
      <section v-if="mode === 'bot'" class="card bot-panel">
        <div class="bot-options">
          <div class="color-match">
            <span class="limit-label">내 진영</span>
            <label><input v-model="humanSide" type="radio" value="white" /> White</label>
            <label><input v-model="humanSide" type="radio" value="black" /> Black</label>
          </div>
          <label class="difficulty-select">
            <span class="limit-label">난이도</span>
            <select v-model="difficulty">
              <option value="easy">Easy</option>
              <option value="normal">Normal</option>
              <option value="hard">Hard</option>
            </select>
          </label>
        </div>
      </section>

      <section class="summary-grid">
        <div class="card summary-card">
          <p class="summary-title">{{ mode === 'bot' ? '내 덱' : 'White 덱' }}</p>
          <select v-model="primaryDeckId" class="text-input">
            <option value="">선택 안 함</option>
            <option v-for="deck in validDecks" :key="deck.id" :value="deck.id">
              {{ deck.name }} · {{ deck.boardSize }}x{{ deck.boardSize }}
            </option>
          </select>
          <p v-if="primaryDeck">{{ deckInfo(primaryDeck) }}</p>
        </div>

        <div class="card summary-card">
          <p class="summary-title">{{ mode === 'bot' ? '봇 덱' : 'Black 덱' }}</p>
          <select v-model="secondaryDeckId" class="text-input">
            <option value="">선택 안 함</option>
            <option v-for="deck in validDecks" :key="deck.id" :value="deck.id">
              {{ deck.name }} · {{ deck.boardSize }}x{{ deck.boardSize }}
            </option>
          </select>
          <p v-if="secondaryDeck">{{ deckInfo(secondaryDeck) }}</p>
        </div>
      </section>

      <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
      <p v-if="invalidDecks.length > 0" class="muted-note">
        유효하지 않은 덱 {{ invalidDecks.length }}개는 선택 목록에서 제외되었습니다.
      </p>
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { BotDifficulty } from '../types/game'
import type { BotDeckSelection, DeckSelectMode, LobbyPlayer, SavedDeck, SingleDeckSelection } from '../types/deck'
import { useSavedDecks } from '../composables/useSavedDecks'
import { totalPocketCount, validateSavedDeck } from '../composables/useDeckValidation'

const props = defineProps<{
  mode: DeckSelectMode
}>()

const emit = defineEmits<{
  back: []
  'deck-building': []
  'start-single': [selection: SingleDeckSelection]
  'start-bot': [selection: BotDeckSelection]
}>()

const savedDecks = useSavedDecks()
const decks = ref<SavedDeck[]>([])
const primaryDeckId = ref('')
const secondaryDeckId = ref('')
const humanSide = ref<LobbyPlayer>('white')
const difficulty = ref<BotDifficulty>('normal')

const validDecks = computed(() => decks.value.filter(deck => validateSavedDeck(deck).valid))
const invalidDecks = computed(() => decks.value.filter(deck => !validateSavedDeck(deck).valid))
const primaryDeck = computed(() => decks.value.find(deck => deck.id === primaryDeckId.value) ?? null)
const secondaryDeck = computed(() => decks.value.find(deck => deck.id === secondaryDeckId.value) ?? null)
const sameBoardSize = computed(() => Boolean(primaryDeck.value && secondaryDeck.value && primaryDeck.value.boardSize === secondaryDeck.value.boardSize))
const errorMessage = computed(() => {
  if (!primaryDeck.value || !secondaryDeck.value) return null
  if (!sameBoardSize.value) return '선택한 두 덱의 보드 크기가 다릅니다. 같은 보드 크기의 덱을 선택하세요.'
  return null
})
const canStart = computed(() => Boolean(primaryDeck.value && secondaryDeck.value && sameBoardSize.value))

function refresh() {
  decks.value = savedDecks.loadDecks()
  primaryDeckId.value = validDecks.value[0]?.id ?? ''
  secondaryDeckId.value = validDecks.value.find(deck => deck.id !== primaryDeckId.value)?.id ?? validDecks.value[0]?.id ?? ''
}

function deckInfo(deck: SavedDeck): string {
  const summary = validateSavedDeck(deck)
  return `${summary.totalScore} / ${summary.scoreLimit}점 · 시작 ${deck.starting.length} · 포켓 ${totalPocketCount(deck)}`
}

function start() {
  if (!canStart.value) return

  if (props.mode === 'bot') {
    emit('start-bot', {
      humanSide: humanSide.value,
      humanDeckId: primaryDeckId.value,
      botDeckId: secondaryDeckId.value,
      difficulty: difficulty.value,
    })
    return
  }

  emit('start-single', {
    whiteDeckId: primaryDeckId.value,
    blackDeckId: secondaryDeckId.value,
  })
}

watch(() => props.mode, refresh)
onMounted(refresh)
</script>
