<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="selectedChallenge ? selectedChallenge = null : $emit('back')">
        {{ selectedChallenge ? '목록으로' : '로비로' }}
      </button>
      <div>
        <p class="eyebrow">Single Player Challenge</p>
        <h1>{{ selectedChallenge?.name ?? '챌린지' }}</h1>
      </div>
      <span></span>
    </div>

    <p v-if="loading" class="card challenge-notice">Challenge 목록을 불러오는 중입니다.</p>
    <p v-else-if="error" class="card challenge-notice error">{{ error }}</p>

    <section v-else-if="!selectedChallenge" class="challenge-grid">
      <article v-for="challenge in challenges" :key="challenge.id" class="card challenge-card">
        <div>
          <p class="summary-title">{{ challenge.cleared ? '✓ 클리어' : '미클리어' }}</p>
          <h2>{{ challenge.name }}</h2>
          <p>{{ challenge.description }}</p>
          <strong>{{ challenge.board_size }}×{{ challenge.board_size }} · {{ difficultyLabel(challenge.bot_difficulty) }}</strong>
        </div>
        <button class="btn-start" @click="selectChallenge(challenge)">도전하기</button>
      </article>
    </section>

    <template v-else>
      <section class="card challenge-detail">
        <div>
          <p class="summary-title">{{ selectedChallenge.board_size }}×{{ selectedChallenge.board_size }} Challenge</p>
          <p>{{ selectedChallenge.description }}</p>
        </div>
        <strong>상대: {{ difficultyLabel(selectedChallenge.bot_difficulty) }} 봇</strong>
      </section>

      <section v-if="decks.length === 0" class="card empty-state">
        <h2>먼저 덱을 만들어 주세요.</h2>
        <p>저장된 덱이 없으면 Challenge를 시작할 수 없습니다.</p>
        <button class="btn-start" @click="$emit('deck-building')">덱 빌딩으로 이동</button>
      </section>
      <section v-else class="deck-grid">
        <article
          v-for="deck in decks"
          :key="deck.id"
          class="card deck-card challenge-deck"
          :class="{ invalid: !availability(deck).valid, selected: selectedDeckId === deck.id }"
        >
          <div class="deck-card-main">
            <p class="summary-title">{{ availability(deck).valid ? '사용 가능' : '사용 불가' }}</p>
            <h2>{{ deck.name }}</h2>
            <p>{{ deck.boardSize }}×{{ deck.boardSize }}</p>
            <span>{{ availability(deck).reason }}</span>
          </div>
          <button class="btn-secondary" :disabled="!availability(deck).valid" @click="selectedDeckId = deck.id">
            {{ selectedDeckId === deck.id ? '선택됨' : '선택' }}
          </button>
        </article>
      </section>
      <p v-if="startError" class="error">{{ startError }}</p>
      <button v-if="decks.length" class="btn-start challenge-start" :disabled="!selectedDeckId || starting" @click="start">
        {{ starting ? '시작 중…' : 'Challenge 시작' }}
      </button>
    </template>
  </main>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api, type ChallengeSummary } from '../api/gameApi'
import type { BotDifficulty, GameState } from '../types/game'
import type { SavedDeck } from '../types/deck'
import { useSavedDecks } from '../composables/useSavedDecks'
import { validateSavedDeck } from '../composables/useDeckValidation'
import { serializeNeutralDeck } from '../composables/useDeckSerialization'

const emit = defineEmits<{
  back: []
  'deck-building': []
  started: [payload: { state: GameState; challenge: ChallengeSummary }]
}>()
const savedDecks = useSavedDecks()
const challenges = ref<ChallengeSummary[]>([])
const decks = ref<SavedDeck[]>([])
const selectedChallenge = ref<ChallengeSummary | null>(null)
const selectedDeckId = ref('')
const loading = ref(true)
const starting = ref(false)
const error = ref<string | null>(null)
const startError = ref<string | null>(null)

function difficultyLabel(value: BotDifficulty) {
  return ({ easy: 'Easy', normal: 'Normal', hard: 'Hard' } as const)[value]
}

function availability(deck: SavedDeck): { valid: boolean; reason: string } {
  const challenge = selectedChallenge.value
  if (!challenge) return { valid: false, reason: '' }
  const summary = validateSavedDeck(deck)
  if (!summary.valid) return { valid: false, reason: summary.errors[0] ?? '유효하지 않은 덱입니다.' }
  if (deck.mapId !== challenge.map_id) {
    return { valid: false, reason: `이 Challenge는 ${challenge.board_size}×${challenge.board_size} 일반전 덱이 필요합니다.` }
  }
  return { valid: true, reason: `${summary.totalScore} / ${summary.scoreLimit}점` }
}

function selectChallenge(challenge: ChallengeSummary) {
  selectedChallenge.value = challenge
  selectedDeckId.value = decks.value.find(deck => availability(deck).valid)?.id ?? ''
  startError.value = null
}

async function start() {
  const challenge = selectedChallenge.value
  const deck = decks.value.find(item => item.id === selectedDeckId.value)
  if (!challenge || !deck || !availability(deck).valid) return
  starting.value = true
  startError.value = null
  try {
    const { state } = await api.createChallengeGame(challenge.id, serializeNeutralDeck(deck, 'white'))
    emit('started', { state, challenge })
  } catch (cause) {
    startError.value = cause instanceof Error ? cause.message : String(cause)
  } finally {
    starting.value = false
  }
}

onMounted(async () => {
  decks.value = savedDecks.loadDecks()
  try { challenges.value = await api.listChallenges() }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { loading.value = false }
})
</script>

<style scoped>
.challenge-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; }
.challenge-card { min-height: 230px; padding: 22px; display: flex; flex-direction: column; justify-content: space-between; gap: 20px; }
.challenge-card > div, .challenge-detail > div { display: grid; gap: 10px; }
.challenge-card h2 { color: #f4dfb0; }
.challenge-card p:not(.summary-title), .challenge-deck span { color: var(--muted); line-height: 1.55; }
.challenge-detail { padding: 20px; display: flex; align-items: center; justify-content: space-between; gap: 20px; }
.challenge-deck { padding: 18px; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.challenge-deck.selected { border-color: rgba(217, 164, 65, .7); }
.challenge-notice { padding: 18px; }
.challenge-start { align-self: flex-end; }
</style>
