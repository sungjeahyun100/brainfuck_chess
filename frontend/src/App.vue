<template>
  <div class="app">
    <div v-if="!gameState" class="lobby">
      <div class="lobby-hero">
        <p class="eyebrow">Deck Builder Lobby</p>
        <h1>Brainfuck Chess</h1>
        <p class="subtitle">보드 크기, 시작 배치, 포켓 기물을 한 화면에서 설정합니다.</p>
      </div>

      <div class="lobby-topbar card">
        <div class="board-select">
          <label for="board-size">보드 크기</label>
          <select id="board-size" v-model.number="selectedSize">
            <option v-for="n in [8, 9, 10, 11, 12]" :key="n" :value="n">
              {{ n }} × {{ n }} (최대 {{ scoreLimit(n) }}점)
            </option>
          </select>
        </div>

        <div class="limit-panel">
          <span class="limit-label">점수 상한</span>
          <strong>{{ scoreLimit(selectedSize) }}점</strong>
        </div>

        <button class="btn-secondary" @click="resetDecks">추천 덱으로 초기화</button>
      </div>

      <div class="player-tabs">
        <button
          v-for="player in players"
          :key="player"
          class="player-tab"
          :class="[`player-tab-${player}`, { active: activePlayer === player }]"
          @click="activePlayer = player"
        >
          {{ playerLabel(player) }} Deck
        </button>
      </div>

      <div class="summary-grid">
        <div class="card summary-card" :class="{ invalid: !whiteSummary.valid }">
          <p class="summary-title">White</p>
          <strong>{{ whiteSummary.totalScore }} / {{ scoreLimit(selectedSize) }}점</strong>
          <span>시작 {{ whiteDeck.starting.length }}개 / 포켓 {{ totalPocketCount(whiteDeck) }}개</span>
          <p class="summary-status">{{ whiteSummary.valid ? '시작 가능' : whiteSummary.errors[0] }}</p>
        </div>

        <div class="card summary-card" :class="{ invalid: !blackSummary.valid }">
          <p class="summary-title">Black</p>
          <strong>{{ blackSummary.totalScore }} / {{ scoreLimit(selectedSize) }}점</strong>
          <span>시작 {{ blackDeck.starting.length }}개 / 포켓 {{ totalPocketCount(blackDeck) }}개</span>
          <p class="summary-status">{{ blackSummary.valid ? '시작 가능' : blackSummary.errors[0] }}</p>
        </div>
      </div>

      <div class="builder-grid">
        <section class="card tool-panel">
          <div class="section-header">
            <div>
              <p class="section-kicker">현재 편집</p>
              <h2>{{ playerLabel(activePlayer) }} 기본 진영</h2>
            </div>
            <p class="section-description">기물을 고른 뒤 기본 진영 칸을 클릭해 배치합니다.</p>
          </div>

          <div class="piece-palette">
            <button
              v-for="piece in pieceCatalog"
              :key="piece.id"
              class="palette-piece"
              :class="{ active: placementTool === piece.id }"
              @click="placementTool = piece.id"
            >
              <span class="symbol">{{ displayPieceSymbol(piece.id, activePlayer) }}</span>
              <span class="meta">
                <strong>{{ piece.name }}</strong>
                <small>{{ piece.score === 0 ? '점수 제외' : `${piece.score}점` }}</small>
              </span>
            </button>

            <button
              class="palette-piece erase"
              :class="{ active: placementTool === 'erase' }"
              @click="placementTool = 'erase'"
            >
              <span class="symbol">✕</span>
              <span class="meta">
                <strong>지우개</strong>
                <small>선택한 칸의 시작 기물 제거</small>
              </span>
            </button>
          </div>

          <div class="active-counts">
            <div v-for="piece in pieceCatalog" :key="piece.id" class="count-chip">
              <span>{{ piece.name }}</span>
              <strong>{{ pieceCount(activeDeck, piece.id) }}</strong>
            </div>
          </div>
        </section>

        <section class="card board-panel">
          <div class="section-header">
            <div>
              <p class="section-kicker">기본 진영 배치</p>
              <h2>{{ playerLabel(activePlayer) }} Frontline</h2>
            </div>
            <p class="section-description">King은 시작 기물에 정확히 한 개만 있어야 합니다.</p>
          </div>

          <div class="placement-board" :style="{ '--board-size': selectedSize }">
            <button
              v-for="square in activeBaseZoneSquares"
              :key="`${square.file}_${square.rank}`"
              class="placement-square"
              :class="squareClass(square.file, square.rank)"
              @click="onPlacementSquareClick(square.file, square.rank)"
            >
              <span class="square-label">{{ fileLabel(square.file) }}{{ square.rank + 1 }}</span>
              <span v-if="pieceAt(activePlayer, square.file, square.rank)" class="square-piece">
                {{ displayPieceSymbol(pieceAt(activePlayer, square.file, square.rank)!, activePlayer) }}
              </span>
              <span v-else class="square-empty">+</span>
            </button>
          </div>
        </section>

        <section class="card pocket-panel">
          <div class="section-header">
            <div>
              <p class="section-kicker">포켓 기물</p>
              <h2>{{ playerLabel(activePlayer) }} Pocket</h2>
            </div>
            <p class="section-description">포켓에는 King을 넣을 수 없습니다.</p>
          </div>

          <div class="pocket-list">
            <div v-for="piece in pocketCatalog" :key="piece.id" class="pocket-row">
              <div class="pocket-piece-info">
                <span class="symbol">{{ displayPieceSymbol(piece.id, activePlayer) }}</span>
                <div class="meta meta-column">
                  <strong>{{ piece.name }}</strong>
                  <small>{{ piece.score }}점</small>
                </div>
              </div>

              <div class="stepper">
                <button @click="changePocketCount(piece.id, -1)">-</button>
                <span>{{ activeDeck.pocket[piece.id] }}</span>
                <button @click="changePocketCount(piece.id, 1)">+</button>
              </div>
            </div>
          </div>

          <div v-if="activeSummary.errors.length > 0" class="validation-list">
            <p v-for="message in activeSummary.errors" :key="message">{{ message }}</p>
          </div>
        </section>
      </div>

      <button class="btn-start" :disabled="!lobbyReady" @click="startGame">게임 시작</button>
      <p v-if="lobbyError" class="error">{{ lobbyError }}</p>
    </div>

    <GameScreen
      v-else
      :state="gameState"
      @state-update="gameState = $event"
      @restart="gameState = null"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { GameState, Square } from './types/game'
import { api, type PlayerDeckRequest } from './api/gameApi'
import GameScreen from './components/GameScreen.vue'

type LobbyPlayer = 'white' | 'black'
type DeckPieceType = 'king' | 'queen' | 'rook' | 'bishop' | 'knight' | 'pawn'
type PocketPieceType = Exclude<DeckPieceType, 'king'>

interface LobbyPlacement {
  pieceType: DeckPieceType
  square: Square
}

interface LobbyDeck {
  starting: LobbyPlacement[]
  pocket: Record<PocketPieceType, number>
}

interface DeckSummary {
  totalScore: number
  valid: boolean
  errors: string[]
}

const players: LobbyPlayer[] = ['white', 'black']

const pieceCatalog: Array<{ id: DeckPieceType; name: string; score: number }> = [
  { id: 'king', name: 'King', score: 0 },
  { id: 'queen', name: 'Queen', score: 9 },
  { id: 'rook', name: 'Rook', score: 5 },
  { id: 'bishop', name: 'Bishop', score: 3 },
  { id: 'knight', name: 'Knight', score: 3 },
  { id: 'pawn', name: 'Pawn', score: 1 },
]

const pocketCatalog = pieceCatalog.filter(
  (piece): piece is { id: PocketPieceType; name: string; score: number } => piece.id !== 'king',
)

const gameState = ref<GameState | null>(null)
const selectedSize = ref(8)
const lobbyError = ref<string | null>(null)
const activePlayer = ref<LobbyPlayer>('white')
const placementTool = ref<DeckPieceType | 'erase'>('king')
const lobbyDecks = ref<Record<LobbyPlayer, LobbyDeck>>(createLobbyDecks(selectedSize.value))

function scoreLimit(n: number): number {
  return n * n - 25
}

function playerLabel(player: LobbyPlayer): string {
  return player === 'white' ? 'White' : 'Black'
}

function fileLabel(file: number): string {
  return String.fromCharCode(97 + file)
}

function emptyPocket(): Record<PocketPieceType, number> {
  return {
    queen: 0,
    rook: 0,
    bishop: 0,
    knight: 0,
    pawn: 0,
  }
}

function createStandardStarting(player: LobbyPlayer, boardSize: number): LobbyPlacement[] {
  const offset = Math.floor((boardSize - 8) / 2)
  const backRank = player === 'white' ? 0 : boardSize - 1
  const pawnRank = player === 'white' ? 1 : boardSize - 2
  const backline: DeckPieceType[] = ['rook', 'knight', 'bishop', 'queen', 'king', 'bishop', 'knight', 'rook']

  return [
    ...backline.map((pieceType, index) => ({
      pieceType,
      square: { file: offset + index, rank: backRank },
    })),
    ...Array.from({ length: 8 }, (_, index) => ({
      pieceType: 'pawn' as const,
      square: { file: offset + index, rank: pawnRank },
    })),
  ]
}

function createLobbyDeck(player: LobbyPlayer, boardSize: number): LobbyDeck {
  return {
    starting: createStandardStarting(player, boardSize),
    pocket: emptyPocket(),
  }
}

function createLobbyDecks(boardSize: number): Record<LobbyPlayer, LobbyDeck> {
  return {
    white: createLobbyDeck('white', boardSize),
    black: createLobbyDeck('black', boardSize),
  }
}

function resetDecks() {
  lobbyDecks.value = createLobbyDecks(selectedSize.value)
  activePlayer.value = 'white'
  placementTool.value = 'king'
  lobbyError.value = null
}

watch(selectedSize, () => {
  resetDecks()
})

const whiteDeck = computed(() => lobbyDecks.value.white)
const blackDeck = computed(() => lobbyDecks.value.black)
const activeDeck = computed(() => lobbyDecks.value[activePlayer.value])

function deckSummary(deck: LobbyDeck): DeckSummary {
  const totalScore = deck.starting
    .filter(piece => piece.pieceType !== 'king')
    .reduce((sum, piece) => sum + pieceCatalog.find(entry => entry.id === piece.pieceType)!.score, 0)
    + Object.entries(deck.pocket).reduce((sum, [pieceType, count]) => {
      const score = pieceCatalog.find(entry => entry.id === pieceType)!.score
      return sum + score * count
    }, 0)

  const kingCount = deck.starting.filter(piece => piece.pieceType === 'king').length
  const errors: string[] = []

  if (kingCount !== 1) {
    errors.push('King은 기본 진영에 정확히 1개 있어야 합니다.')
  }

  if (totalScore > scoreLimit(selectedSize.value)) {
    errors.push(`덱 점수 ${totalScore}점이 상한 ${scoreLimit(selectedSize.value)}점을 초과했습니다.`)
  }

  return {
    totalScore,
    valid: errors.length === 0,
    errors,
  }
}

const whiteSummary = computed(() => deckSummary(whiteDeck.value))
const blackSummary = computed(() => deckSummary(blackDeck.value))
const activeSummary = computed(() => deckSummary(activeDeck.value))
const lobbyReady = computed(() => whiteSummary.value.valid && blackSummary.value.valid)

const activeBaseZoneSquares = computed(() => {
  const ranks = activePlayer.value === 'white'
    ? [1, 0]
    : [selectedSize.value - 1, selectedSize.value - 2]

  return ranks.flatMap(rank => Array.from({ length: selectedSize.value }, (_, file) => ({ file, rank })))
})

function pieceCount(deck: LobbyDeck, pieceType: DeckPieceType): number {
  return deck.starting.filter(piece => piece.pieceType === pieceType).length
}

function totalPocketCount(deck: LobbyDeck): number {
  return Object.values(deck.pocket).reduce((sum, count) => sum + count, 0)
}

function pieceAt(player: LobbyPlayer, file: number, rank: number): DeckPieceType | null {
  return lobbyDecks.value[player].starting.find(
    piece => piece.square.file === file && piece.square.rank === rank,
  )?.pieceType ?? null
}

function squareClass(file: number, rank: number): string[] {
  return [
    (file + rank) % 2 === 0 ? 'light' : 'dark',
    pieceAt(activePlayer.value, file, rank) ? 'occupied' : 'empty',
  ]
}

function onPlacementSquareClick(file: number, rank: number) {
  lobbyError.value = null
  const deck = lobbyDecks.value[activePlayer.value]
  const existing = pieceAt(activePlayer.value, file, rank)

  if (placementTool.value === 'erase') {
    deck.starting = deck.starting.filter(piece => piece.square.file !== file || piece.square.rank !== rank)
    return
  }

  if (existing === placementTool.value) {
    deck.starting = deck.starting.filter(piece => piece.square.file !== file || piece.square.rank !== rank)
    return
  }

  deck.starting = deck.starting.filter(piece => {
    if (piece.square.file === file && piece.square.rank === rank) {
      return false
    }

    if (placementTool.value === 'king' && piece.pieceType === 'king') {
      return false
    }

    return true
  })

  deck.starting.push({
    pieceType: placementTool.value,
    square: { file, rank },
  })
}

function changePocketCount(pieceType: PocketPieceType, delta: number) {
  const deck = lobbyDecks.value[activePlayer.value]
  deck.pocket[pieceType] = Math.max(0, deck.pocket[pieceType] + delta)
}

function displayPieceSymbol(pieceType: DeckPieceType, player: LobbyPlayer): string {
  const whiteSymbols: Record<DeckPieceType, string> = {
    king: '♔',
    queen: '♕',
    rook: '♖',
    bishop: '♗',
    knight: '♘',
    pawn: '♙',
  }

  const blackSymbols: Record<DeckPieceType, string> = {
    king: '♚',
    queen: '♛',
    rook: '♜',
    bishop: '♝',
    knight: '♞',
    pawn: '♟',
  }

  return player === 'white' ? whiteSymbols[pieceType] : blackSymbols[pieceType]
}

function serializeDeck(deck: LobbyDeck): PlayerDeckRequest {
  return {
    starting: deck.starting.map(piece => ({
      piece_type: piece.pieceType,
      square: piece.square,
    })),
    pocket: pocketCatalog.flatMap(piece => Array.from({ length: deck.pocket[piece.id] }, () => piece.id)),
  }
}

async function startGame() {
  if (!lobbyReady.value) {
    lobbyError.value = '양쪽 덱 구성이 모두 유효해야 게임을 시작할 수 있습니다.'
    return
  }

  lobbyError.value = null
  try {
    const { state } = await api.createGame(
      selectedSize.value,
      serializeDeck(whiteDeck.value),
      serializeDeck(blackDeck.value),
    )
    gameState.value = state
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<style>
:root {
  --bg: #0f1722;
  --panel: rgba(19, 26, 39, 0.9);
  --line: rgba(255, 255, 255, 0.08);
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

.app { min-height: 100vh; display: flex; flex-direction: column; }

.lobby {
  width: min(1400px, 100%);
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
  flex: 1;
  padding: 32px 20px 40px;
}

.lobby-hero {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.eyebrow,
.limit-label,
.section-kicker,
.summary-title,
.board-select label {
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--accent);
  font-size: 12px;
}

.lobby h1 {
  font-size: clamp(2.4rem, 4vw, 4rem);
  color: #f4dfb0;
}

.subtitle,
.section-description,
.summary-status,
.meta small,
.pocket-piece-info small {
  color: var(--muted);
}

.card {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 18px;
  backdrop-filter: blur(12px);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.28);
}

.lobby-topbar {
  display: grid;
  grid-template-columns: repeat(3, max-content);
  gap: 16px;
  align-items: end;
  padding: 18px;
}

.board-select {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.board-select select {
  padding: 10px 12px;
  font-size: 16px;
  border-radius: 10px;
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
  border-radius: 12px;
}

.btn-secondary,
.btn-start,
.player-tab,
.palette-piece,
.placement-square,
.stepper button {
  border: none;
  cursor: pointer;
}

.btn-secondary {
  padding: 12px 18px;
  border-radius: 12px;
  background: #243142;
  color: var(--text);
}

.player-tabs {
  display: flex;
  gap: 10px;
}

.player-tab {
  padding: 12px 18px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
}

.player-tab.active.player-tab-white {
  background: rgba(255, 255, 255, 0.92);
  color: #101723;
}

.player-tab.active.player-tab-black {
  background: #1f2a3c;
  color: #f4f7fb;
  outline: 1px solid rgba(255, 255, 255, 0.18);
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.summary-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 18px;
}

.summary-card.invalid {
  border-color: rgba(255, 125, 125, 0.32);
}

.builder-grid {
  display: grid;
  grid-template-columns: minmax(260px, 0.95fr) minmax(460px, 1.5fr) minmax(260px, 0.95fr);
  gap: 16px;
}

.tool-panel,
.board-panel,
.pocket-panel {
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.section-header {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.section-header h2 {
  font-size: 1.2rem;
}

.piece-palette,
.pocket-list {
  display: grid;
  gap: 10px;
}

.palette-piece,
.pocket-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
}

.palette-piece.active {
  background: rgba(217, 164, 65, 0.18);
  outline: 1px solid rgba(217, 164, 65, 0.5);
}

.palette-piece.erase.active {
  background: rgba(255, 125, 125, 0.14);
  outline-color: rgba(255, 125, 125, 0.45);
}

.symbol {
  font-size: 26px;
  line-height: 1;
}

.meta,
.pocket-piece-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.meta-column,
.meta {
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
}

.active-counts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.count-chip {
  display: flex;
  justify-content: space-between;
  padding: 10px 12px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.04);
}

.placement-board {
  display: grid;
  grid-template-columns: repeat(var(--board-size), 1fr);
  gap: 6px;
}

.placement-square {
  min-height: 74px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.placement-square.light { background: #f1dfbf; color: #232a38; }
.placement-square.dark { background: #b7844d; color: #fff8ef; }
.placement-square.occupied { outline: 2px solid rgba(217, 164, 65, 0.48); }

.square-label {
  position: absolute;
  top: 6px;
  left: 8px;
  font-size: 10px;
  opacity: 0.72;
}

.square-piece {
  font-size: clamp(24px, 2.5vw, 38px);
}

.square-empty {
  font-size: 22px;
  opacity: 0.35;
}

.stepper {
  display: flex;
  align-items: center;
  gap: 10px;
}

.stepper button {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: #243142;
  color: var(--text);
}

.validation-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--danger);
  font-size: 14px;
}

.btn-start {
  align-self: flex-start;
  padding: 14px 30px;
  font-size: 17px;
  background: linear-gradient(135deg, #f0c15f, #c68a1b);
  color: #221a0d;
  border-radius: 14px;
  font-weight: 700;
}

.btn-start:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.error {
  color: var(--danger);
}

@media (max-width: 1100px) {
  .builder-grid,
  .summary-grid {
    grid-template-columns: 1fr;
  }

  .lobby-topbar {
    grid-template-columns: 1fr;
    align-items: stretch;
  }
}

@media (max-width: 700px) {
  .lobby {
    padding: 20px 14px 28px;
  }

  .player-tabs {
    flex-direction: column;
  }

  .placement-square {
    min-height: 58px;
  }
}
</style>