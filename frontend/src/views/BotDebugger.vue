<template>
  <main class="lobby bot-debugger">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div>
        <p class="eyebrow">Bot Debugger</p>
        <h1>봇 재현 테스트</h1>
      </div>
      <button class="btn-start" :disabled="!canStart" @click="start">디버그 경기 시작</button>
    </div>

    <section class="card debugger-notice">
      <strong>실제 봇 경로 사용</strong>
      <p>일반 봇 플레이와 동일한 게임 생성 API와 봇 탐색 엔진을 사용합니다. 별도 모의 봇이나 단순화된 규칙은 사용하지 않습니다.</p>
      <p v-if="difficulty === 'easy'" class="debugger-warning">Easy는 실제 봇의 의도적인 후보 무작위 선택도 그대로 사용하므로 같은 설정에서도 착수가 달라질 수 있습니다.</p>
      <p v-else>Normal과 Hard는 Easy의 무작위 후보 선택을 사용하지 않습니다. 탐색 시간 제한에 따른 차이까지 비교할 수 있도록 실제 측정값을 함께 기록합니다.</p>
    </section>

    <section v-if="decks.length === 0" class="card empty-state">
      <h2>테스트할 저장 덱이 없습니다.</h2>
      <p>내 덱과 봇 덱을 먼저 만들어 주세요.</p>
      <button class="btn-start" @click="$emit('deck-building')">덱 빌딩으로 이동</button>
    </section>

    <template v-else>
      <section class="card debugger-options">
        <div class="color-match">
          <span class="limit-label">내 진영</span>
          <label><input v-model="humanSide" type="radio" value="white" /> White</label>
          <label><input v-model="humanSide" type="radio" value="black" /> Black</label>
        </div>
        <label class="difficulty-select">
          <span class="limit-label">실제 봇 난이도</span>
          <select v-model="difficulty">
            <option value="easy">Easy (무작위 선택 포함)</option>
            <option value="normal">Normal</option>
            <option value="hard">Hard</option>
          </select>
        </label>
        <label class="difficulty-select">
          <span class="limit-label">타임 컨트롤</span>
          <select v-model="timeControl">
            <option v-for="option in TIME_CONTROLS" :key="option.id" :value="option.id">{{ option.label }}</option>
          </select>
        </label>
      </section>

      <section class="debugger-decks">
        <article class="card debugger-deck">
          <p class="summary-title">내 덱</p>
          <select v-model="humanDeckId" class="text-input">
            <option v-for="deck in validDecks" :key="deck.id" :value="deck.id">{{ deck.name }} · {{ boardMapLabel(deck.mapId) }}</option>
          </select>
          <div v-if="humanDeck" class="deck-snapshot">
            <strong>{{ deckSummary(humanDeck) }}</strong>
            <small><b>시작 배치</b> {{ startingInfo(humanDeck) }}</small>
            <small><b>포켓</b> {{ pocketInfo(humanDeck) }}</small>
          </div>
        </article>

        <article class="card debugger-deck bot-deck">
          <p class="summary-title">봇 덱</p>
          <select v-model="botDeckId" class="text-input">
            <option v-for="deck in validDecks" :key="deck.id" :value="deck.id">{{ deck.name }} · {{ boardMapLabel(deck.mapId) }}</option>
          </select>
          <div v-if="botDeck" class="deck-snapshot">
            <strong>{{ deckSummary(botDeck) }}</strong>
            <small><b>시작 배치</b> {{ startingInfo(botDeck) }}</small>
            <small><b>포켓</b> {{ pocketInfo(botDeck) }}</small>
          </div>
        </article>
      </section>

      <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
      <div class="debugger-actions">
        <button class="btn-secondary" :disabled="!canStart" @click="copySetup">{{ copyStatus }}</button>
        <small>설정 JSON에는 두 덱의 현재 배치·포켓 스냅샷이 포함됩니다.</small>
      </div>
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { BotDifficulty, TimeControlId } from '../types/game'
import type { BotDeckSelection, LobbyPlayer, SavedDeck } from '../types/deck'
import { useSavedDecks } from '../composables/useSavedDecks'
import { totalPocketCount, validateSavedDeck } from '../composables/useDeckValidation'
import { boardMapLabel } from '../boardMaps'
import { TIME_CONTROLS } from '../timeControls'
import { squareName } from '../replayNotation'

const props = defineProps<{ initialSelection?: BotDeckSelection | null }>()
const emit = defineEmits<{
  back: []
  'deck-building': []
  start: [selection: BotDeckSelection]
}>()

const savedDecks = useSavedDecks()
const decks = ref<SavedDeck[]>([])
const humanDeckId = ref('')
const botDeckId = ref('')
const humanSide = ref<LobbyPlayer>('white')
const difficulty = ref<BotDifficulty>('normal')
const timeControl = ref<TimeControlId>('unlimited')
const copyStatus = ref('설정 JSON 복사')

const validDecks = computed(() => decks.value.filter(deck => validateSavedDeck(deck).valid))
const humanDeck = computed(() => validDecks.value.find(deck => deck.id === humanDeckId.value) ?? null)
const botDeck = computed(() => validDecks.value.find(deck => deck.id === botDeckId.value) ?? null)
const sameMap = computed(() => Boolean(humanDeck.value && botDeck.value && humanDeck.value.mapId === botDeck.value.mapId))
const errorMessage = computed(() => humanDeck.value && botDeck.value && !sameMap.value
  ? '같은 맵 전용 덱을 선택해야 동일한 규칙 상태를 만들 수 있습니다.'
  : null)
const canStart = computed(() => Boolean(humanDeck.value && botDeck.value && sameMap.value))

function selection(): BotDeckSelection {
  return {
    humanSide: humanSide.value,
    humanDeckId: humanDeckId.value,
    botDeckId: botDeckId.value,
    difficulty: difficulty.value,
    timeControl: timeControl.value,
  }
}

function start() {
  if (canStart.value) emit('start', selection())
}

async function copySetup() {
  if (!canStart.value) return
  try {
    await navigator.clipboard.writeText(JSON.stringify({
      selection: selection(),
      human_deck: humanDeck.value,
      bot_deck: botDeck.value,
    }, null, 2))
    copyStatus.value = '복사 완료'
  } catch {
    copyStatus.value = '복사 실패'
  }
  window.setTimeout(() => { copyStatus.value = '설정 JSON 복사' }, 1_800)
}

function deckSummary(deck: SavedDeck): string {
  const summary = validateSavedDeck(deck)
  return `${summary.totalScore} / ${summary.scoreLimit}점 · 시작 ${deck.starting.length} · 포켓 ${totalPocketCount(deck)}`
}

function startingInfo(deck: SavedDeck): string {
  return [...deck.starting]
    .sort((left, right) => left.square.rank - right.square.rank || left.square.file - right.square.file)
    .map(piece => `${piece.pieceType} ${squareName(piece.square)}`)
    .join(', ') || '없음'
}

function pocketInfo(deck: SavedDeck): string {
  return Object.entries(deck.pocket)
    .filter(([, count]) => count > 0)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([pieceType, count]) => `${pieceType} ×${count}`)
    .join(', ') || '없음'
}

onMounted(() => {
  decks.value = savedDecks.loadDecks()
  const initial = props.initialSelection
  const validIds = new Set(validDecks.value.map(deck => deck.id))
  humanDeckId.value = initial && validIds.has(initial.humanDeckId)
    ? initial.humanDeckId
    : validDecks.value[0]?.id ?? ''
  botDeckId.value = initial && validIds.has(initial.botDeckId)
    ? initial.botDeckId
    : validDecks.value.find(deck => deck.id !== humanDeckId.value)?.id ?? validDecks.value[0]?.id ?? ''
  if (initial) {
    humanSide.value = initial.humanSide
    difficulty.value = initial.difficulty
    timeControl.value = initial.timeControl
  }
})
</script>

<style scoped>
.bot-debugger { display: grid; gap: 16px; }
.debugger-notice { display: grid; gap: 6px; border-color: rgba(217, 164, 65, .45); }
.debugger-notice p { margin: 0; color: var(--muted); line-height: 1.5; }
.debugger-warning { color: #ffb56b !important; }
.debugger-options { display: flex; flex-wrap: wrap; gap: 20px; align-items: end; }
.debugger-decks { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.debugger-deck { display: grid; align-content: start; gap: 10px; }
.bot-deck { border-color: rgba(217, 164, 65, .48); }
.deck-snapshot { display: grid; gap: 8px; padding-top: 8px; border-top: 1px solid rgba(255,255,255,.1); }
.deck-snapshot small { color: var(--muted); line-height: 1.5; overflow-wrap: anywhere; }
.debugger-actions { display: flex; align-items: center; gap: 12px; color: var(--muted); }
@media (max-width: 760px) { .debugger-decks { grid-template-columns: 1fr; } }
</style>
