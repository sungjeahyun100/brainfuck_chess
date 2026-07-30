<template>
  <div class="app" :class="{ 'app-with-env-banner': showEnvBanner }">
    <div
      v-if="showEnvBanner"
      class="env-banner"
      :class="`env-banner-${appEnv}`"
    >
      {{ envBannerLabel }}
    </div>

    <GameScreen
      v-if="gameState"
      :state="gameState"
      :local-player="localPlayer"
      :room-id="currentRoom?.id ?? null"
      :bot-player="playMode === 'bot' ? botPlayer : null"
      :bot-difficulty="botDifficulty"
      @state-update="onGameStateUpdate"
      @restart="restartToLobby"
    />

    <LobbyHome
      v-else-if="view === 'home'"
      @navigate="navigate"
    />
    <DeckLibrary
      v-else-if="view === 'deck-library'"
      @back="navigate('home')"
      @edit="openDeckEditor"
    />
    <DeckEditor
      v-else-if="view === 'deck-editor'"
      :deck-id="editingDeckId"
      @back="navigate('deck-library')"
      @saved="navigate('deck-library')"
      @test-piece="openPieceLabFromDeckEditor"
    />
    <PieceLab
      v-else-if="view === 'piece-lab'"
      :initial-piece-type="pieceLabInitial.pieceType"
      :initial-board-size="pieceLabInitial.boardSize"
      @back="closePieceLab"
    />
    <CustomPieceWorkshop
      v-else-if="view === 'custom-piece-workshop'"
      @back="navigate('home')"
    />
    <DeckSelect
      v-else-if="view === 'single-select'"
      mode="single"
      @back="navigate('home')"
      @deck-building="navigate('deck-library')"
      @start-single="startSingleGame"
    />
    <DeckSelect
      v-else-if="view === 'bot-select'"
      mode="bot"
      @back="navigate('home')"
      @deck-building="navigate('deck-library')"
      @start-bot="startBotGame"
    />
    <MultiplayerLobby
      v-else
      @back="navigate('home')"
      @deck-building="navigate('deck-library')"
      @game-started="startMultiplayerGame"
    />

    <p v-if="lobbyError && !gameState" class="global-error error">{{ lobbyError }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import type { BotDifficulty, GameState } from './types/game'
import type {
  AppView,
  BotDeckSelection,
  LobbyPlayer,
  SingleDeckSelection,
} from './types/deck'
import { api, type MultiplayerRoom } from './api/gameApi'
import GameScreen from './components/GameScreen.vue'
import LobbyHome from './views/LobbyHome.vue'
import DeckLibrary from './views/DeckLibrary.vue'
import DeckEditor from './views/DeckEditor.vue'
import DeckSelect from './views/DeckSelect.vue'
import MultiplayerLobby from './views/MultiplayerLobby.vue'
import PieceLab from './views/PieceLab.vue'
import CustomPieceWorkshop from './views/CustomPieceWorkshop.vue'
import { appEnv, envBannerLabel, showEnvBanner } from './config'
import { useSavedDecks } from './composables/useSavedDecks'
import { serializeNeutralDeck } from './composables/useDeckSerialization'
import { validateSavedDeck } from './composables/useDeckValidation'

const savedDecks = useSavedDecks()
const view = ref<AppView>('home')
const editingDeckId = ref<string | null>(null)
const pieceLabReturnView = ref<AppView>('home')
const pieceLabInitial = ref<{ pieceType: string | null; boardSize: number | null }>({
  pieceType: null,
  boardSize: null,
})
const gameState = ref<GameState | null>(null)
const currentRoom = ref<MultiplayerRoom | null>(null)
const localPlayer = ref<LobbyPlayer | null>(null)
const playMode = ref<'single' | 'bot' | 'multiplayer'>('single')
const botDifficulty = ref<BotDifficulty>('normal')
const lobbyError = ref<string | null>(null)
const gamePollTimer = ref<number | null>(null)
const botPlayer = computed<LobbyPlayer>(() => localPlayer.value === 'white' ? 'black' : 'white')

function navigate(nextView: AppView) {
  stopGamePolling()
  lobbyError.value = null
  if (nextView !== 'deck-editor' && nextView !== 'piece-lab') {
    editingDeckId.value = null
  }
  if (nextView === 'piece-lab') {
    pieceLabReturnView.value = view.value
    pieceLabInitial.value = { pieceType: null, boardSize: null }
  }
  view.value = nextView
}

function openDeckEditor(deckId: string) {
  editingDeckId.value = deckId
  view.value = 'deck-editor'
}

function openPieceLabFromDeckEditor(payload: { pieceType: string; boardSize: number }) {
  stopGamePolling()
  lobbyError.value = null
  pieceLabReturnView.value = 'deck-editor'
  pieceLabInitial.value = {
    pieceType: payload.pieceType,
    boardSize: payload.boardSize,
  }
  view.value = 'piece-lab'
}

function closePieceLab() {
  lobbyError.value = null
  view.value = pieceLabReturnView.value === 'piece-lab' ? 'home' : pieceLabReturnView.value
}

function getValidDeck(deckId: string) {
  const deck = savedDecks.getDeck(deckId)
  if (!deck) {
    throw new Error('선택한 덱을 찾을 수 없습니다.')
  }
  const summary = validateSavedDeck(deck)
  if (!summary.valid) {
    throw new Error(summary.errors[0] ?? '유효하지 않은 덱입니다.')
  }
  return deck
}

function ensureSameBoardSize(whiteBoardSize: number, blackBoardSize: number) {
  if (whiteBoardSize !== blackBoardSize) {
    throw new Error('선택한 두 덱의 보드 크기가 다릅니다. 같은 보드 크기의 덱을 선택하세요.')
  }
}

async function startSingleGame(selection: SingleDeckSelection) {
  lobbyError.value = null
  playMode.value = 'single'
  try {
    const whiteDeck = getValidDeck(selection.whiteDeckId)
    const blackDeck = getValidDeck(selection.blackDeckId)
    ensureSameBoardSize(whiteDeck.boardSize, blackDeck.boardSize)

    const { state } = await api.createGame(
      whiteDeck.boardSize,
      serializeNeutralDeck(whiteDeck, 'white'),
      serializeNeutralDeck(blackDeck, 'black'),
    )
    localPlayer.value = null
    currentRoom.value = null
    gameState.value = state
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}

async function startBotGame(selection: BotDeckSelection) {
  lobbyError.value = null
  playMode.value = 'bot'
  botDifficulty.value = selection.difficulty
  try {
    const humanDeck = getValidDeck(selection.humanDeckId)
    const selectedBotDeck = getValidDeck(selection.botDeckId)
    ensureSameBoardSize(humanDeck.boardSize, selectedBotDeck.boardSize)

    const whiteDeck = selection.humanSide === 'white' ? humanDeck : selectedBotDeck
    const blackDeck = selection.humanSide === 'black' ? humanDeck : selectedBotDeck
    const { state } = await api.createGame(
      humanDeck.boardSize,
      serializeNeutralDeck(whiteDeck, 'white'),
      serializeNeutralDeck(blackDeck, 'black'),
    )
    localPlayer.value = selection.humanSide
    currentRoom.value = null
    gameState.value = state
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}

function startMultiplayerGame(payload: { state: GameState; room: MultiplayerRoom; localPlayer: LobbyPlayer }) {
  playMode.value = 'multiplayer'
  localPlayer.value = payload.localPlayer
  currentRoom.value = payload.room
  gameState.value = payload.state
  if (payload.room.game_id) {
    startGamePolling(payload.room.game_id)
  }
}

function onGameStateUpdate(state: GameState) {
  gameState.value = state
  if (state.phase === 'ended') {
    stopGamePolling()
  }
}

function restartToLobby() {
  stopGamePolling()
  gameState.value = null
  currentRoom.value = null
  localPlayer.value = null
  lobbyError.value = null
  view.value = 'home'
}

function stopGamePolling() {
  if (gamePollTimer.value !== null) {
    window.clearInterval(gamePollTimer.value)
    gamePollTimer.value = null
  }
}

function startGamePolling(gameId: string) {
  stopGamePolling()
  gamePollTimer.value = window.setInterval(async () => {
    try {
      gameState.value = await api.getGame(gameId)
    } catch {
      // Keep the last known state visible through transient sync failures.
    }
  }, 900)
}

onUnmounted(stopGamePolling)
</script>

<style>
:root {
  --bg: #0f1722;
  --panel: rgba(19, 26, 39, 0.92);
  --line: rgba(255, 255, 255, 0.1);
  --text: #eef2f7;
  --muted: #a8b1c2;
  --accent: #d9a441;
  --danger: #ff7d7d;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
  font-family: 'Segoe UI', sans-serif;
  background:
    radial-gradient(circle at top, rgba(217, 164, 65, 0.16), transparent 24%),
    linear-gradient(180deg, #101723 0%, #0b111a 100%);
  color: var(--text);
  min-height: 100vh;
}

button,
input,
select {
  font: inherit;
}

.app {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.app-with-env-banner {
  padding-top: 28px;
}

.env-banner {
  position: fixed;
  z-index: 1000;
  top: 0;
  left: 0;
  right: 0;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  color: #101723;
  font-size: 12px;
  font-weight: 900;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.env-banner-local { background: #74d4ff; }
.env-banner-test { background: #ffd45f; }

.lobby {
  width: min(1400px, 100%);
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
  flex: 1;
  padding: 32px 20px 40px;
}

.home-view {
  justify-content: center;
  min-height: 100vh;
}

.lobby-hero {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.lobby h1 {
  font-size: clamp(2.2rem, 4vw, 4rem);
  color: #f4dfb0;
}

.hero-en {
  font-size: 0.5em;
  color: var(--muted);
  font-weight: 400;
}

.eyebrow,
.limit-label,
.section-kicker,
.summary-title {
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--accent);
  font-size: 12px;
}

.card {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.24);
}

.home-actions {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 14px;
}

.home-action {
  min-height: 120px;
  padding: 24px;
  border: 1px solid rgba(217, 164, 65, 0.28);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.05);
  color: #f4dfb0;
  cursor: pointer;
  font-size: 1.3rem;
  font-weight: 800;
}

.home-action:hover {
  background: rgba(217, 164, 65, 0.14);
}

.page-bar {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr) max-content;
  gap: 16px;
  align-items: center;
}

.btn-secondary,
.btn-start,
.palette-piece,
.placement-square,
.tool-button,
.preset-card {
  border: none;
  cursor: pointer;
}

.btn-secondary {
  padding: 12px 18px;
  border-radius: 8px;
  background: #243142;
  color: var(--text);
}

.btn-secondary.danger {
  color: var(--danger);
}

.btn-secondary:disabled,
.btn-start:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-start {
  align-self: flex-start;
  padding: 14px 26px;
  border-radius: 8px;
  background: linear-gradient(135deg, #f0c15f, #c68a1b);
  color: #221a0d;
  font-weight: 800;
}

.empty-state {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 22px;
  color: var(--muted);
}

.empty-state h2 {
  color: var(--text);
}

.deck-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 16px;
}

.deck-card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
}

.deck-card.invalid {
  border-color: rgba(255, 125, 125, 0.4);
}

.deck-card-main {
  display: flex;
  flex-direction: column;
  gap: 8px;
  cursor: pointer;
}

.deck-card-main h2,
.deck-card-main strong,
.room-state strong {
  color: #f4dfb0;
}

.deck-card-main span,
.summary-status,
.muted-note,
.room-state p,
.room-status {
  color: var(--muted);
}

.deck-card-actions,
.room-buttons,
.room-code-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.editor-topbar {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) minmax(180px, 260px);
  gap: 16px;
  align-items: end;
  padding: 18px;
}

.editor-topbar label,
.room-actions label,
.difficulty-select,
.bot-opponent {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.text-input,
.room-code-input,
.piece-search,
.difficulty-select select {
  width: 100%;
  padding: 11px 12px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: #0d1520;
  color: var(--text);
}

.limit-panel {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 14px;
  background: rgba(255, 255, 255, 0.04);
  border-radius: 8px;
}

.deck-score-panel {
  display: grid;
  grid-template-columns: max-content minmax(220px, 1fr);
  gap: 18px;
  align-items: center;
  padding: 18px;
  border-color: rgba(217, 164, 65, 0.35);
  background: linear-gradient(90deg, rgba(217, 164, 65, 0.13), rgba(255, 255, 255, 0.04));
}

.deck-score-copy {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 190px;
}

.deck-score-copy strong {
  color: #f4dfb0;
  font-size: 26px;
}

.deck-score-copy span:last-child {
  color: var(--muted);
  font-size: 13px;
}

.deck-score-meter {
  height: 18px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(5, 8, 13, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.09);
}

.deck-score-meter > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #d9a441, #f4dfb0);
  transition: width 0.18s ease;
}

.deck-score-meter.over > span {
  background: linear-gradient(90deg, #ff7d7d, #ffc1a1);
}

.preset-panel,
.deck-score-panel,
.piece-list-panel,
.board-panel,
.pocket-panel,
.multiplayer-panel,
.bot-panel,
.summary-card {
  padding: 18px;
}

.section-header,
.summary-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.section-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.section-title-row h2 {
  margin: 0;
}

.section-score-pill {
  flex: 0 0 auto;
  padding: 5px 9px;
  border-radius: 999px;
  background: rgba(217, 164, 65, 0.13);
  color: #f4dfb0;
  font-size: 12px;
  font-weight: 800;
  white-space: nowrap;
}

.preset-list {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
  margin-top: 14px;
}

.preset-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  text-align: left;
}

.preset-card strong {
  color: #f4dfb0;
}

.preset-card span {
  color: var(--muted);
  font-size: 13px;
}

.builder-grid {
  display: grid;
  grid-template-columns: minmax(260px, 0.95fr) minmax(460px, 1.5fr) minmax(260px, 0.95fr);
  gap: 16px;
}

.piece-list-panel,
.board-panel,
.pocket-panel {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.piece-catalog {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 540px;
  overflow-y: auto;
}

.catalog-section,
.piece-palette,
.pocket-summary {
  display: grid;
  gap: 10px;
}

.catalog-section-title {
  display: flex;
  justify-content: space-between;
  color: var(--muted);
  font-size: 12px;
  font-weight: 700;
  text-transform: uppercase;
}

.palette-piece {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 68px;
  padding: 12px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  text-align: left;
}

.palette-piece-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) max-content;
  gap: 8px;
  align-items: stretch;
}

.piece-test-button {
  min-width: 64px;
  border: none;
  border-radius: 8px;
  background: #243142;
  color: var(--text);
  cursor: pointer;
  font-weight: 800;
}

.piece-test-button:hover {
  background: rgba(217, 164, 65, 0.18);
  color: #f4dfb0;
}

.palette-piece.active {
  background: rgba(217, 164, 65, 0.18);
  outline: 1px solid rgba(217, 164, 65, 0.5);
}

.symbol {
  display: inline-flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  font-size: 26px;
  line-height: 1;
  flex: 0 0 34px;
}

.piece-icon {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.meta {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  flex: 1;
}

.meta small {
  color: var(--muted);
}

.piece-count {
  min-width: 28px;
  padding: 4px 8px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: #f4dfb0;
  font-size: 12px;
  font-weight: 800;
  text-align: center;
}

.placement-controls {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 10px;
}

.tool-button,
.selected-tool {
  min-height: 54px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
}

.tool-button {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0 14px;
}

.tool-button.active {
  background: rgba(255, 125, 125, 0.14);
  outline: 1px solid rgba(255, 125, 125, 0.45);
}

.selected-tool {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  padding: 0 14px;
}

.placement-board {
  display: grid;
  grid-template-columns: repeat(var(--board-size), 1fr);
  gap: 6px;
}

.placement-square {
  min-height: 74px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.placement-square.light { background: #f1dfbf; color: #232a38; }
.placement-square.dark { background: #b7844d; color: #fff8ef; }
.placement-square.occupied { outline: 2px solid rgba(217, 164, 65, 0.48); }
.placement-square.drop-ready { outline: 2px dashed rgba(244, 223, 176, 0.7); }

.square-label {
  position: absolute;
  top: 6px;
  left: 8px;
  font-size: 10px;
  opacity: 0.72;
}

.square-piece {
  display: inline-flex;
  width: min(70%, 46px);
  height: min(70%, 46px);
  align-items: center;
  justify-content: center;
  font-size: 34px;
}

.square-empty {
  font-size: 22px;
  opacity: 0.35;
}

.pocket-drop-zone {
  min-height: 64px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px;
  border: 1px dashed rgba(255, 255, 255, 0.18);
  border-radius: 8px;
  color: var(--muted);
  background: rgba(255, 255, 255, 0.03);
  text-align: center;
}

.pocket-drop-zone.ready {
  border-color: rgba(217, 164, 65, 0.65);
  background: rgba(217, 164, 65, 0.12);
  color: #f4dfb0;
}

.pocket-summary {
  grid-template-columns: 1fr;
}

.pocket-chip {
  min-height: 64px;
  display: grid;
  grid-template-columns: 42px minmax(90px, 0.8fr) minmax(110px, 1fr) 30px;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
}

.pocket-piece-symbol {
  width: 38px;
  height: 38px;
  flex-basis: 38px;
}

.pocket-piece-name {
  min-width: 0;
  color: var(--text);
  font-weight: 800;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pocket-quantity {
  display: grid;
  grid-template-columns: minmax(64px, 1fr) minmax(24px, max-content);
  gap: 8px;
  align-items: center;
}

.pocket-quantity-bar {
  height: 10px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(5, 8, 13, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.pocket-quantity-bar > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #d9a441, #f4dfb0);
  transition: width 0.18s ease;
}

.pocket-quantity strong {
  color: #f4dfb0;
  font-size: 13px;
  text-align: right;
}

.pocket-remove-button {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 50%;
  background: #243142;
  color: var(--text);
  cursor: pointer;
  font-size: 18px;
  font-weight: 800;
  line-height: 1;
}

.pocket-remove-button:hover {
  background: rgba(255, 125, 125, 0.18);
  color: #ffc1c1;
}

.validation-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--danger);
  font-size: 14px;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.bot-options,
.room-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.color-match {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  padding: 12px 14px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
}

.color-match label {
  display: flex;
  align-items: center;
  gap: 6px;
}

.room-actions,
.room-state {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.room-state {
  padding: 14px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
}

.room-code-input {
  min-width: 180px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.error {
  color: var(--danger);
}

.global-error {
  width: min(1400px, calc(100% - 40px));
  margin: -24px auto 24px;
}

@media (max-width: 1100px) {
  .builder-grid,
  .summary-grid,
  .room-grid,
  .bot-options,
  .editor-topbar {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 700px) {
  .lobby {
    padding: 20px 14px 28px;
  }

  .page-bar {
    grid-template-columns: 1fr;
  }

  .placement-square {
    min-height: 52px;
  }
}
</style>
