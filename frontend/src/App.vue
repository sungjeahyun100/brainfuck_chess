<template>
  <div class="app" :class="{ 'app-with-env-banner': showEnvBanner }">
    <div
      v-if="showEnvBanner"
      class="env-banner"
      :class="`env-banner-${appEnv}`"
    >
      {{ envBannerLabel }}
    </div>

    <div v-if="!gameState" class="lobby">
      <div class="lobby-hero">
        <p class="eyebrow">Deck Builder Lobby</p>
        <h1>덱체스 <span class="hero-en">Deck Chess</span></h1>
        <p class="subtitle">덱 제한 점수 안에서 기물을 고르고, 시작 기물과 포켓 기물로 나만의 덱을 만드는 체스입니다.</p>
      </div>

      <div class="mode-tabs">
        <button
          class="mode-tab"
          :class="{ active: playMode === 'local' }"
          @click="setPlayMode('local')"
        >
          로컬 게임
        </button>
        <button
          class="mode-tab"
          :class="{ active: playMode === 'bot' }"
          @click="setPlayMode('bot')"
        >
          봇 대전
        </button>
        <button
          class="mode-tab"
          :class="{ active: playMode === 'multiplayer' }"
          @click="setPlayMode('multiplayer')"
        >
          멀티플레이
        </button>
      </div>

      <div v-if="playMode === 'bot'" class="card bot-panel">
        <div class="section-header">
          <div>
            <p class="section-kicker">Bot Match</p>
            <h2>봇 대전 설정</h2>
          </div>
          <p class="section-description">사람 진영과 봇 난이도를 선택합니다. 봇은 자기 차례의 모든 행동과 턴 종료를 자동으로 처리합니다.</p>
        </div>
        <div class="bot-options">
          <div class="color-match">
            <span class="limit-label">내 진영</span>
            <label><input v-model="botHumanSide" type="radio" value="white" /> White</label>
            <label><input v-model="botHumanSide" type="radio" value="black" /> Black</label>
          </div>
          <label class="difficulty-select">
            <span class="limit-label">난이도</span>
            <select v-model="botDifficulty">
              <option value="easy">Easy · 빠름 / 약간의 랜덤성</option>
              <option value="normal">Normal · 균형</option>
              <option value="hard">Hard · 더 깊은 탐색</option>
            </select>
          </label>
          <div class="bot-opponent">
            <span class="limit-label">봇 진영</span>
            <strong>{{ playerLabel(botPlayer) }}</strong>
          </div>
        </div>
      </div>

      <div class="lobby-topbar card">
        <div class="board-select">
          <label for="board-size">보드 크기</label>
          <select id="board-size" v-model.number="selectedSize" :disabled="Boolean(currentRoom)">
            <option v-for="n in [8, 9, 10, 11, 12]" :key="n" :value="n">
              {{ n }} × {{ n }} (최대 {{ scoreLimit(n) }}점)
            </option>
          </select>
        </div>

        <div class="limit-panel">
          <span class="limit-label">덱 제한 점수</span>
          <strong>{{ scoreLimit(selectedSize) }}점</strong>
        </div>

        <button class="btn-secondary" @click="resetDecks()">기본 배치로 초기화</button>
      </div>

      <div v-if="playMode === 'multiplayer'" class="card multiplayer-panel">
        <div class="section-header">
          <div>
            <p class="section-kicker">Room</p>
            <h2>멀티플레이 방</h2>
          </div>
          <p class="section-description">방장은 보드 크기를 정하고 한쪽 덱을 제출합니다. 참가자는 방 번호로 들어와 반대쪽 덱을 제출합니다.</p>
        </div>

        <div class="room-grid">
          <div class="room-actions">
            <div class="color-match">
              <span class="limit-label">색상 매칭</span>
              <label>
                <input v-model="hostSideMode" type="radio" value="white" :disabled="Boolean(currentRoom)" />
                White
              </label>
              <label>
                <input v-model="hostSideMode" type="radio" value="black" :disabled="Boolean(currentRoom)" />
                Black
              </label>
              <label>
                <input v-model="hostSideMode" type="radio" value="random" :disabled="Boolean(currentRoom)" />
                랜덤
              </label>
            </div>
            <div class="room-code-row">
              <input v-model.trim="roomCodeInput" class="room-code-input" maxlength="6" placeholder="입장할 방 번호" />
            </div>
            <div class="room-buttons">
              <button class="btn-secondary" :disabled="!activeSummary.valid" @click="createMultiplayerRoom">방 만들기</button>
              <button class="btn-secondary" :disabled="!canJoinRoom" @click="joinMultiplayerRoom">입장하고 시작</button>
              <button class="btn-secondary" :disabled="!currentRoom" @click="refreshRoom">새로고침</button>
            </div>
          </div>

          <div class="room-state">
            <span class="limit-label">현재 방</span>
            <strong>{{ currentRoom ? currentRoom.id : '없음' }}</strong>
            <p v-if="currentRoom">
              방장 {{ playerLabel(currentRoom.host_side) }} · 참가자 {{ playerLabel(currentRoom.guest_side) }} · {{ currentRoom.board_size }} × {{ currentRoom.board_size }}
            </p>
            <p v-else>내 덱을 구성한 뒤 방을 만들거나, 방 번호를 입력해 참가하세요. 랜덤 매칭은 방 생성 순간 색상을 배정합니다.</p>
          </div>
        </div>

        <p v-if="multiplayerStatus" class="room-status">{{ multiplayerStatus }}</p>
      </div>

      <div v-if="playMode !== 'multiplayer'" class="player-tabs">
        <button
          v-for="player in players"
          :key="player"
          class="player-tab"
          :class="[`player-tab-${player}`, { active: activePlayer === player }]"
          @click="activePlayer = player"
        >
          {{ playerLabel(player) }} Deck
          <span v-if="playMode === 'bot'">· {{ player === botHumanSide ? '나' : '봇' }}</span>
        </button>
      </div>

      <div v-if="playMode !== 'multiplayer'" class="summary-grid">
        <div class="card summary-card" :class="{ invalid: !whiteSummary.valid }">
          <p class="summary-title">White<span v-if="playMode === 'bot'"> · {{ botHumanSide === 'white' ? '나' : '봇' }}</span></p>
          <strong>덱 점수: {{ whiteSummary.totalScore }} / {{ scoreLimit(selectedSize) }}</strong>
          <span>시작 {{ whiteDeck.starting.length }}개 / 포켓 {{ totalPocketCount(whiteDeck) }}개</span>
          <p class="summary-status">{{ whiteSummary.valid ? '시작 가능' : whiteSummary.errors[0] }}</p>
        </div>

        <div class="card summary-card" :class="{ invalid: !blackSummary.valid }">
          <p class="summary-title">Black<span v-if="playMode === 'bot'"> · {{ botHumanSide === 'black' ? '나' : '봇' }}</span></p>
          <strong>덱 점수: {{ blackSummary.totalScore }} / {{ scoreLimit(selectedSize) }}</strong>
          <span>시작 {{ blackDeck.starting.length }}개 / 포켓 {{ totalPocketCount(blackDeck) }}개</span>
          <p class="summary-status">{{ blackSummary.valid ? '시작 가능' : blackSummary.errors[0] }}</p>
        </div>
      </div>
      <div v-else class="summary-grid">
        <div class="card summary-card" :class="{ invalid: !activeSummary.valid }">
          <p class="summary-title">내 덱 설정</p>
          <strong>덱 점수: {{ activeSummary.totalScore }} / {{ scoreLimit(selectedSize) }}</strong>
          <span>시작 {{ activeDeck.starting.length }}개 / 포켓 {{ totalPocketCount(activeDeck) }}개</span>
          <p class="summary-status">{{ activeSummary.valid ? '방 생성/입장 가능' : activeSummary.errors[0] }}</p>
        </div>
      </div>

      <section class="card preset-panel">
        <div class="section-header">
          <div>
            <p class="section-kicker">Preset</p>
            <h2>프리셋 덱으로 바로 시작하기</h2>
          </div>
          <p class="section-description">{{ deckEditorLabel }} 덱에 프리셋을 적용합니다. 적용 후에도 자유롭게 기물을 바꿀 수 있습니다.</p>
        </div>
        <div class="preset-list">
          <button
            v-for="preset in deckPresets"
            :key="preset.id"
            class="preset-card"
            @click="applyPresetToActive(preset.id)"
          >
            <strong>{{ preset.name }}</strong>
            <span>{{ preset.description }}</span>
          </button>
        </div>
      </section>

      <div class="builder-grid">
        <section class="card piece-list-panel">
          <div class="section-header">
            <div>
              <p class="section-kicker">기물 목록</p>
              <h2>{{ deckEditorLabel }} Arsenal</h2>
            </div>
            <p class="section-description">기물을 배치설정이나 포켓설정으로 드래그합니다.</p>
          </div>

          <input
            v-model.trim="pieceSearch"
            class="piece-search"
            type="search"
            placeholder="기물 검색"
          />

          <div class="piece-catalog">
            <div v-for="section in catalogSections" :key="section.id" class="catalog-section">
              <div class="catalog-section-title">
                <span
                  class="catalog-title-label"
                  :class="{ 'has-tooltip': section.id === 'variant' }"
                  :tabindex="section.id === 'variant' ? 0 : undefined"
                >
                  {{ section.label }}
                  <span v-if="section.id === 'variant'" class="tooltip-mark" aria-hidden="true">?</span>
                  <span v-if="section.id === 'variant'" class="variant-tooltip" role="tooltip">
                    {{ variantTooltip }}
                  </span>
                </span>
                <small>{{ section.pieces.length }}</small>
              </div>

              <div class="piece-palette">
                <button
                  v-for="piece in section.pieces"
                  :key="piece.id"
                  v-memo="[piece.id, piece.name, piece.score, pieceCount(activeDeck, piece.id), activePlayer, placementTool]"
                  class="palette-piece"
                  :class="{ active: isPieceToolActive(piece.id) }"
                  :aria-label="pieceAriaLabel(piece)"
                  draggable="true"
                  @click="selectPieceTool(piece.id)"
                  @dragstart="onPieceDragStart($event, piece.id)"
                  @dragend="onPieceDragEnd"
                >
                  <span class="symbol">
                    <img
                      v-if="displayPieceAsset(piece.id, activePlayer)"
                      class="piece-icon"
                      :src="displayPieceAsset(piece.id, activePlayer)"
                      :alt="`${playerLabel(activePlayer)} ${piece.name}`"
                      draggable="false"
                    />
                    <span v-else>{{ displayPieceSymbol(piece.id, activePlayer) }}</span>
                  </span>
                  <span class="meta">
                    <strong>{{ piece.name }}</strong>
                    <small>{{ piece.score === 0 ? '점수 제외' : `${piece.score}점` }}</small>
                  </span>
                  <span class="piece-count">{{ pieceCount(activeDeck, piece.id) }}</span>
                  <span v-if="pieceMoveTip(piece.id)" class="piece-tooltip" role="tooltip">
                    {{ pieceMoveTip(piece.id) }}
                  </span>
                </button>
              </div>
            </div>
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
              <p class="section-kicker">배치설정</p>
              <h2>{{ deckEditorLabel }} Frontline</h2>
            </div>
            <p class="section-description">기물 목록에서 드롭하거나 선택한 기물로 칸을 클릭합니다.</p>
          </div>

          <div class="placement-controls">
            <button
              class="tool-button"
              :class="{ active: placementTool === eraseTool }"
              @click="selectEraseTool"
            >
              <span>✕</span>
              <strong>지우개</strong>
            </button>
            <div class="selected-tool">
              <span class="limit-label">선택 기물</span>
              <strong>{{ selectedToolLabel }}</strong>
            </div>
          </div>

          <div class="placement-board" :style="{ '--board-size': selectedSize }">
            <button
              v-for="square in activeBaseZoneSquares"
              :key="`${square.file}_${square.rank}`"
              v-memo="[activePlayer, square.file, square.rank, pieceAt(activePlayer, square.file, square.rank), draggedPiece]"
              class="placement-square"
              :class="squareClass(square.file, square.rank)"
              @click="onPlacementSquareClick(square.file, square.rank)"
              @dragover.prevent="onPlacementDragOver"
              @drop.prevent="onPlacementDrop($event, square.file, square.rank)"
            >
              <span class="square-label">{{ fileLabel(square.file) }}{{ square.rank + 1 }}</span>
              <span v-if="pieceAt(activePlayer, square.file, square.rank)" class="square-piece">
                <img
                  v-if="displayPieceAsset(pieceAt(activePlayer, square.file, square.rank)!, activePlayer)"
                  class="piece-icon"
                  :src="displayPieceAsset(pieceAt(activePlayer, square.file, square.rank)!, activePlayer)"
                  :alt="`${playerLabel(activePlayer)} ${pieceLabel(pieceAt(activePlayer, square.file, square.rank)!)}`"
                  draggable="false"
                />
                <span v-else>{{ displayPieceSymbol(pieceAt(activePlayer, square.file, square.rank)!, activePlayer) }}</span>
              </span>
              <span v-else class="square-empty">+</span>
            </button>
          </div>
        </section>

        <section class="card pocket-panel">
          <div class="section-header">
            <div>
              <p class="section-kicker">포켓설정</p>
              <h2>{{ deckEditorLabel }} Pocket</h2>
            </div>
            <p class="section-description">기물 목록에서 드롭하면 포켓 수량이 1개 늘어납니다.</p>
          </div>

          <div
            class="pocket-drop-zone"
            :class="{ ready: draggedPiece && canUseInPocket(draggedPiece) }"
            @dragover.prevent="onPocketDragOver"
            @drop.prevent="onPocketDrop($event)"
          >
            <span>{{ pocketDropMessage }}</span>
          </div>

          <div v-if="activePocketEntries.length > 0" class="pocket-summary">
            <div v-for="entry in activePocketEntries" :key="entry.piece.id" class="pocket-chip">
              <span class="symbol">
                <img
                  v-if="displayPieceAsset(entry.piece.id, activePlayer)"
                  class="piece-icon"
                  :src="displayPieceAsset(entry.piece.id, activePlayer)"
                  :alt="`${playerLabel(activePlayer)} ${entry.piece.name}`"
                  draggable="false"
                />
                <span v-else>{{ displayPieceSymbol(entry.piece.id, activePlayer) }}</span>
              </span>
              <strong>{{ entry.count }}</strong>
              <button @click="changePocketCount(entry.piece.id, -1)">-</button>
            </div>
          </div>
          <div v-else class="pocket-empty">
            포켓에 추가된 기물이 없습니다.
          </div>

          <div v-if="activeSummary.errors.length > 0" class="validation-list">
            <p v-for="message in activeSummary.errors" :key="message">{{ message }}</p>
          </div>
        </section>
      </div>

      <button v-if="playMode !== 'multiplayer'" class="btn-start" :disabled="!localLobbyReady" @click="startGame">
        {{ playMode === 'bot' ? '봇 대전 시작' : '게임 시작' }}
      </button>
      <p v-if="lobbyError" class="error">{{ lobbyError }}</p>
    </div>

    <GameScreen
      v-else
      :state="gameState"
      :local-player="localPlayer"
      :room-id="currentRoom?.id ?? null"
      :bot-player="playMode === 'bot' ? botPlayer : null"
      :bot-difficulty="botDifficulty"
      @state-update="onGameStateUpdate"
      @restart="restartToLobby"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import type { BotDifficulty, GameState, Square } from './types/game'
import { api, type MultiplayerRoom, type PlayerDeckRequest } from './api/gameApi'
import GameScreen from './components/GameScreen.vue'
import { appEnv, envBannerLabel, showEnvBanner } from './config'
import { pieceAsset } from './pieceAssets'

type LobbyPlayer = 'white' | 'black'
type DeckPieceType = string

interface PieceCatalogItem {
  id: DeckPieceType
  name: string
  score: number
  category: string
  canPocket: boolean
  uniqueStarting?: boolean
  aliases?: string[]
}

interface LobbyPlacement {
  pieceType: DeckPieceType
  square: Square
}

interface LobbyDeck {
  starting: LobbyPlacement[]
  pocket: Record<DeckPieceType, number>
}

interface DeckSummary {
  totalScore: number
  valid: boolean
  errors: string[]
}

const players: LobbyPlayer[] = ['white', 'black']
const eraseTool = '__erase__'

const pieceCatalog: PieceCatalogItem[] = [
  { id: 'king', name: 'King', score: 0, category: 'royal', canPocket: false, uniqueStarting: true },
  { id: 'queen', name: 'Queen', score: 9, category: 'major', canPocket: true },
  { id: 'amazon', name: 'Amazon', score: 13, category: 'variant', canPocket: true },
  { id: 'tempest-queen', name: 'Tempest Queen', score: 12, category: 'variant', canPocket: true, aliases: ['storm queen'] },
  { id: 'tempest-rook', name: 'Tempest Rook', score: 8, category: 'variant', canPocket: true, aliases: ['storm rook'] },
  { id: 'bouncing-bishop', name: 'Bouncing Bishop', score: 7, category: 'variant', canPocket: true, aliases: ['bounce bishop'] },
  { id: 'rook', name: 'Rook', score: 5, category: 'major', canPocket: true },
  { id: 'bishop', name: 'Bishop', score: 3, category: 'minor', canPocket: true },
  { id: 'knight', name: 'Knight', score: 3, category: 'minor', canPocket: true },
  { id: 'pawn', name: 'Pawn', score: 1, category: 'pawn', canPocket: true },
]

const pocketCatalog = pieceCatalog.filter(
  piece => piece.canPocket,
)

interface DeckPreset {
  id: string
  name: string
  description: string
  backline: (DeckPieceType | null)[]
  pawns: (DeckPieceType | null)[]
  pocket: Partial<Record<DeckPieceType, number>>
}

const deckPresets: DeckPreset[] = [
  {
    id: 'classic',
    name: '기본 체스 덱',
    description: '익숙한 기물 중심의 표준 배치. 처음이라면 이 덱으로 시작해 보세요.',
    backline: ['rook', 'knight', 'bishop', 'queen', 'king', 'bishop', 'knight', 'rook'],
    pawns: ['pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn'],
    pocket: {},
  },
  {
    id: 'swarm',
    name: '물량 덱',
    description: '낮은 점수의 Knight와 Pawn을 잔뜩 채운 물량 승부 덱입니다.',
    backline: ['knight', 'knight', 'knight', 'king', 'knight', 'knight', 'knight', 'knight'],
    pawns: ['pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn'],
    pocket: { pawn: 10 },
  },
  {
    id: 'pocket',
    name: '포켓 덱',
    description: '시작 기물은 최소화하고, 게임 중 포켓 기물을 꺼내 쓰는 덱입니다.',
    backline: [null, null, null, 'king', null, null, null, null],
    pawns: ['pawn', 'pawn', 'pawn', 'pawn', null, null, null, null],
    pocket: { rook: 2, bishop: 2, knight: 2, queen: 1, pawn: 4 },
  },
  {
    id: 'mobility',
    name: '기동 덱',
    description: 'Knight·Bishop과 변형 기물로 기동력을 살린 덱입니다.',
    backline: ['knight', 'bishop', 'knight', 'king', 'knight', 'bishop', 'knight', 'bishop'],
    pawns: ['pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn', 'pawn'],
    pocket: { 'bouncing-bishop': 1, knight: 1 },
  },
  {
    id: 'firepower',
    name: '고화력 덱',
    description: 'Queen과 Rook 같은 고점수 기물로 화력을 극대화한 덱입니다.',
    backline: ['rook', 'queen', 'king', 'queen', 'rook', null, null, null],
    pawns: ['pawn', 'pawn', 'pawn', 'pawn', null, null, null, null],
    pocket: { bishop: 1, pawn: 4 },
  },
]

const catalogCategoryLabels: Record<string, string> = {
  royal: 'Royal',
  major: 'Major',
  variant: 'Variant',
  minor: 'Minor',
  pawn: 'Pawn',
}

const variantTooltip = '시작 전 배치와 포켓 기물을 직접 구성합니다. 턴에는 여러 기물을 움직이거나 포켓 기물 하나를 자신의 초기 진영 또는 공격 범위의 칸에 놓을 수 있습니다.'

const pieceMoveDescriptions: Record<string, string> = {
  king: 'King: 한 칸씩 모든 방향으로 이동합니다.',
  queen: 'Queen: 직선과 대각선 방향으로 이동합니다.',
  amazon: 'Amazon: Queen과 Knight 행마를 함께 사용합니다.',
  'tempest-queen': 'Tempest Queen: 대각선으로 한 칸 이동한 뒤, 그 방향의 직선·대각선·가로세로로 이어서 이동합니다.',
  'tempest-rook': 'Tempest Rook: 각 모서리에 룩을 붙인 행마를 가집니다.',
  'bouncing-bishop': 'Bouncing Bishop: 대각선으로 이동하나 벽에 닿을 시 한 번 튕깁니다.',
  rook: 'Rook: 직선 방향으로 이동합니다.',
  bishop: 'Bishop: 대각선 방향으로 이동합니다.',
  knight: 'Knight: 일반 체스 Knight처럼 L자로 이동합니다.',
  pawn: 'Pawn: 플레이어 방향으로 전진하고 대각선으로 잡습니다.',
}

const gameState = ref<GameState | null>(null)
const selectedSize = ref(8)
const lobbyError = ref<string | null>(null)
const multiplayerStatus = ref<string | null>(null)
const activePlayer = ref<LobbyPlayer>('white')
const placementTool = ref<DeckPieceType>(pieceCatalog[0].id)
const pieceSearch = ref('')
const draggedPiece = ref<DeckPieceType | null>(null)
const lobbyDecks = ref<Record<LobbyPlayer, LobbyDeck>>(createLobbyDecks(selectedSize.value))
const playMode = ref<'local' | 'bot' | 'multiplayer'>('local')
const botHumanSide = ref<LobbyPlayer>('white')
const botDifficulty = ref<BotDifficulty>('normal')
const hostSideMode = ref<LobbyPlayer | 'random'>('random')
const roomCodeInput = ref('')
const currentRoom = ref<MultiplayerRoom | null>(null)
const applyingRoomSize = ref(false)
const localPlayer = ref<LobbyPlayer | null>(null)
const roomPollTimer = ref<number | null>(null)
const gamePollTimer = ref<number | null>(null)
const unloadResignSent = ref(false)

function scoreLimit(n: number): number {
  return n * n - 25
}

function playerLabel(player: LobbyPlayer): string {
  return player === 'white' ? 'White' : 'Black'
}

function fileLabel(file: number): string {
  return String.fromCharCode(97 + file)
}

function emptyPocket(): Record<DeckPieceType, number> {
  return Object.fromEntries(pocketCatalog.map(piece => [piece.id, 0]))
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

function createPresetStarting(player: LobbyPlayer, boardSize: number, preset: DeckPreset): LobbyPlacement[] {
  const offset = Math.floor((boardSize - 8) / 2)
  const backRank = player === 'white' ? 0 : boardSize - 1
  const pawnRank = player === 'white' ? 1 : boardSize - 2

  return [
    ...preset.backline
      .map((pieceType, index) => (pieceType ? { pieceType, square: { file: offset + index, rank: backRank } } : null))
      .filter((placement): placement is LobbyPlacement => placement !== null),
    ...preset.pawns
      .map((pieceType, index) => (pieceType ? { pieceType, square: { file: offset + index, rank: pawnRank } } : null))
      .filter((placement): placement is LobbyPlacement => placement !== null),
  ]
}

function applyPresetToActive(presetId: string) {
  const preset = deckPresets.find(entry => entry.id === presetId)
  if (!preset) return

  const pocket = emptyPocket()
  for (const [pieceType, count] of Object.entries(preset.pocket)) {
    pocket[pieceType] = count ?? 0
  }

  lobbyDecks.value[activePlayer.value] = {
    starting: createPresetStarting(activePlayer.value, selectedSize.value, preset),
    pocket,
  }
  lobbyError.value = null
}

function createLobbyDecks(boardSize: number): Record<LobbyPlayer, LobbyDeck> {
  return {
    white: createLobbyDeck('white', boardSize),
    black: createLobbyDeck('black', boardSize),
  }
}

function resetDecks(resetPlayer = true) {
  lobbyDecks.value = createLobbyDecks(selectedSize.value)
  if (resetPlayer) {
    activePlayer.value = 'white'
  }
  placementTool.value = pieceCatalog[0].id
  draggedPiece.value = null
  lobbyError.value = null
}

watch(selectedSize, () => {
  resetDecks(!applyingRoomSize.value)
  if (!applyingRoomSize.value) {
    currentRoom.value = null
    multiplayerStatus.value = null
  }
})

const whiteDeck = computed(() => lobbyDecks.value.white)
const blackDeck = computed(() => lobbyDecks.value.black)
const activeDeck = computed(() => lobbyDecks.value[activePlayer.value])
const pieceById = computed(() => new Map(pieceCatalog.map(piece => [piece.id, piece])))
const filteredPieceCatalog = computed(() => {
  const query = pieceSearch.value.toLowerCase()
  if (!query) return pieceCatalog

  return pieceCatalog.filter(piece => {
    const searchable = [piece.id, piece.name, piece.category, ...(piece.aliases ?? [])]
      .join(' ')
      .toLowerCase()
    return searchable.includes(query)
  })
})
const catalogSections = computed(() => {
  const groups = new Map<string, PieceCatalogItem[]>()

  for (const piece of filteredPieceCatalog.value) {
    const existing = groups.get(piece.category) ?? []
    existing.push(piece)
    groups.set(piece.category, existing)
  }

  return Array.from(groups.entries()).map(([id, pieces]) => ({
    id,
    label: catalogCategoryLabels[id] ?? id,
    pieces,
  }))
})
const activePocketEntries = computed(() => pocketCatalog
  .map(piece => ({
    piece,
    count: activeDeck.value.pocket[piece.id] ?? 0,
  }))
  .filter(entry => entry.count > 0))
const selectedToolLabel = computed(() => {
  if (placementTool.value === eraseTool) return '지우개'

  return pieceById.value.get(placementTool.value)?.name ?? placementTool.value
})
const pocketDropMessage = computed(() => {
  if (!draggedPiece.value) return '여기에 드롭해서 포켓에 추가'
  if (!canUseInPocket(draggedPiece.value)) return `${pieceLabel(draggedPiece.value)}은 포켓에 넣을 수 없습니다.`

  return `${pieceLabel(draggedPiece.value)} 포켓에 추가`
})

function pieceLabel(pieceType: DeckPieceType): string {
  return pieceById.value.get(pieceType)?.name ?? pieceType
}

function pieceMoveTip(pieceType: DeckPieceType): string {
  return pieceMoveDescriptions[pieceType] ?? 'Custom Piece: 등록된 Chessembly 행마법을 따릅니다.'
}

function pieceAriaLabel(piece: PieceCatalogItem): string {
  const score = piece.score === 0 ? '점수 제외' : `${piece.score}점`
  return `${piece.name}, ${score}. ${pieceMoveTip(piece.id)}`
}

function pieceScore(pieceType: DeckPieceType): number {
  return pieceById.value.get(pieceType)?.score ?? 0
}

function canUseInPocket(pieceType: DeckPieceType): boolean {
  return pieceById.value.get(pieceType)?.canPocket === true
}

function isUniqueStartingPiece(pieceType: DeckPieceType): boolean {
  return pieceById.value.get(pieceType)?.uniqueStarting === true
}

function deckSummary(deck: LobbyDeck): DeckSummary {
  const totalScore = deck.starting
    .reduce((sum, piece) => sum + pieceScore(piece.pieceType), 0)
    + Object.entries(deck.pocket).reduce((sum, [pieceType, count]) => {
      return sum + pieceScore(pieceType) * count
    }, 0)

  const kingCount = deck.starting.filter(piece => piece.pieceType === 'king').length
  const errors: string[] = []

  if (kingCount !== 1) {
    errors.push('King은 기본 진영에 정확히 1개 있어야 합니다. (King은 필수 기물이며 점수에서 제외됩니다)')
  }

  const limit = scoreLimit(selectedSize.value)
  if (totalScore > limit) {
    const over = totalScore - limit
    errors.push(`점수가 ${over}점 초과되었습니다. 기물을 줄이거나 더 낮은 점수의 기물로 바꿔보세요.`)
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
const localLobbyReady = computed(() => whiteSummary.value.valid && blackSummary.value.valid)
const botPlayer = computed<LobbyPlayer>(() => botHumanSide.value === 'white' ? 'black' : 'white')
const deckEditorLabel = computed(() => playMode.value === 'multiplayer' ? '내 덱' : playerLabel(activePlayer.value))
const canJoinRoom = computed(() => {
  const isOwnRoom = Boolean(currentRoom.value && localPlayer.value === currentRoom.value.host_side)
  const hasRoomTarget = !isOwnRoom && (Boolean(currentRoom.value && !currentRoom.value.game_id) || roomCodeInput.value.trim().length > 0)
  return hasRoomTarget && activeSummary.value.valid
})

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
    (file + rank) % 2 === 1 ? 'light' : 'dark',
    pieceAt(activePlayer.value, file, rank) ? 'occupied' : 'empty',
    draggedPiece.value ? 'drop-ready' : '',
  ].filter(Boolean)
}

function selectPieceTool(pieceType: DeckPieceType) {
  placementTool.value = pieceType
}

function selectEraseTool() {
  placementTool.value = eraseTool
}

function isPieceToolActive(pieceType: DeckPieceType): boolean {
  return placementTool.value === pieceType
}

function placePieceAt(pieceType: DeckPieceType, file: number, rank: number) {
  lobbyError.value = null
  const deck = lobbyDecks.value[activePlayer.value]
  const existing = pieceAt(activePlayer.value, file, rank)

  if (existing === pieceType) {
    deck.starting = deck.starting.filter(piece => piece.square.file !== file || piece.square.rank !== rank)
    return
  }

  deck.starting = deck.starting.filter(piece => {
    if (piece.square.file === file && piece.square.rank === rank) {
      return false
    }

    if (isUniqueStartingPiece(pieceType) && piece.pieceType === pieceType) {
      return false
    }

    return true
  })

  deck.starting.push({
    pieceType,
    square: { file, rank },
  })
}

function erasePieceAt(file: number, rank: number) {
  const deck = lobbyDecks.value[activePlayer.value]
  deck.starting = deck.starting.filter(piece => piece.square.file !== file || piece.square.rank !== rank)
}

function onPlacementSquareClick(file: number, rank: number) {
  if (placementTool.value === eraseTool) {
    erasePieceAt(file, rank)
    return
  }

  placePieceAt(placementTool.value, file, rank)
}

function getDraggedPiece(event: DragEvent): DeckPieceType | null {
  const fromEvent = event.dataTransfer?.getData('application/x-brainfuck-chess-piece')
    || event.dataTransfer?.getData('text/plain')
    || null
  const pieceType = draggedPiece.value ?? fromEvent

  return pieceType && pieceById.value.has(pieceType) ? pieceType : null
}

function onPieceDragStart(event: DragEvent, pieceType: DeckPieceType) {
  draggedPiece.value = pieceType
  event.dataTransfer?.setData('application/x-brainfuck-chess-piece', pieceType)
  event.dataTransfer?.setData('text/plain', pieceType)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'copy'
  }
}

function onPieceDragEnd() {
  draggedPiece.value = null
}

function onPlacementDragOver(event: DragEvent) {
  if (draggedPiece.value && event.dataTransfer) {
    event.dataTransfer.dropEffect = 'copy'
  }
}

function onPlacementDrop(event: DragEvent, file: number, rank: number) {
  const pieceType = getDraggedPiece(event)
  draggedPiece.value = null
  if (!pieceType) return

  selectPieceTool(pieceType)
  placePieceAt(pieceType, file, rank)
}

function onPocketDragOver(event: DragEvent) {
  if (!draggedPiece.value || !event.dataTransfer) return

  event.dataTransfer.dropEffect = canUseInPocket(draggedPiece.value) ? 'copy' : 'none'
}

function onPocketDrop(event: DragEvent) {
  const pieceType = getDraggedPiece(event)
  draggedPiece.value = null
  if (!pieceType) return

  if (!canUseInPocket(pieceType)) {
    lobbyError.value = `${pieceLabel(pieceType)}은 포켓에 넣을 수 없습니다.`
    return
  }

  changePocketCount(pieceType, 1)
}

function changePocketCount(pieceType: DeckPieceType, delta: number) {
  if (!canUseInPocket(pieceType)) return

  const deck = lobbyDecks.value[activePlayer.value]
  deck.pocket[pieceType] ??= 0
  deck.pocket[pieceType] = Math.max(0, deck.pocket[pieceType] + delta)
}

function displayPieceSymbol(pieceType: DeckPieceType, player: LobbyPlayer): string {
  const whiteSymbols: Partial<Record<DeckPieceType, string>> = {
    king: '♔',
    queen: '♕',
    amazon: 'A',
    'tempest-queen': 'Q',
    'tempest-rook': 'T',
    'bouncing-bishop': 'B',
    rook: '♖',
    bishop: '♗',
    knight: '♘',
    pawn: '♙',
  }

  const blackSymbols: Partial<Record<DeckPieceType, string>> = {
    king: '♚',
    queen: '♛',
    amazon: 'A',
    'tempest-queen': 'Q',
    'tempest-rook': 'T',
    'bouncing-bishop': 'B',
    rook: '♜',
    bishop: '♝',
    knight: '♞',
    pawn: '♟',
  }

  const symbol = player === 'white' ? whiteSymbols[pieceType] : blackSymbols[pieceType]
  return symbol ?? pieceLabel(pieceType).slice(0, 1).toUpperCase()
}

function displayPieceAsset(pieceType: DeckPieceType, player: LobbyPlayer): string | undefined {
  return pieceAsset(pieceType, player)
}

function serializeDeck(deck: LobbyDeck): PlayerDeckRequest {
  return {
    starting: deck.starting.map(piece => ({
      piece_type: piece.pieceType,
      square: piece.square,
    })),
    pocket: pocketCatalog.flatMap(piece => Array.from({ length: deck.pocket[piece.id] ?? 0 }, () => piece.id)),
  }
}

function mirrorSquare(square: Square): Square {
  return {
    file: square.file,
    rank: selectedSize.value - 1 - square.rank,
  }
}

function serializeNeutralDeck(deck: LobbyDeck, fromSide: LobbyPlayer): PlayerDeckRequest {
  if (fromSide === 'white') {
    return serializeDeck(deck)
  }

  return {
    starting: deck.starting.map(piece => ({
      piece_type: piece.pieceType,
      square: mirrorSquare(piece.square),
    })),
    pocket: pocketCatalog.flatMap(piece => Array.from({ length: deck.pocket[piece.id] ?? 0 }, () => piece.id)),
  }
}

function randomSide(): LobbyPlayer {
  return Math.random() < 0.5 ? 'white' : 'black'
}

async function startGame() {
  if (!localLobbyReady.value) {
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
    localPlayer.value = playMode.value === 'bot' ? botHumanSide.value : null
    unloadResignSent.value = false
    stopGamePolling()
    gameState.value = state
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}

function setPlayMode(mode: 'local' | 'bot' | 'multiplayer') {
  stopRoomPolling()
  stopGamePolling()
  playMode.value = mode
  lobbyError.value = null
  multiplayerStatus.value = null
  currentRoom.value = null
  hostSideMode.value = 'random'
  localPlayer.value = null
  unloadResignSent.value = false
  if (mode === 'multiplayer') {
    activePlayer.value = 'white'
  }
}

function restartToLobby() {
  stopRoomPolling()
  stopGamePolling()
  unloadResignSent.value = false
  gameState.value = null
  localPlayer.value = null
}

function onGameStateUpdate(state: GameState) {
  gameState.value = state
  if (state.phase === 'ended') {
    unloadResignSent.value = false
  }
}

function stopRoomPolling() {
  if (roomPollTimer.value !== null) {
    window.clearInterval(roomPollTimer.value)
    roomPollTimer.value = null
  }
}

function stopGamePolling() {
  if (gamePollTimer.value !== null) {
    window.clearInterval(gamePollTimer.value)
    gamePollTimer.value = null
  }
}

function startGamePolling(gameId: string) {
  stopGamePolling()
  if (!localPlayer.value) return

  gamePollTimer.value = window.setInterval(async () => {
    try {
      gameState.value = await api.getGame(gameId)
    } catch {
      // Keep the last known state visible if a single sync request fails.
    }
  }, 900)
}

function shouldResignOnPageExit(): boolean {
  return Boolean(
    playMode.value === 'multiplayer'
    && currentRoom.value
    && localPlayer.value
    && gameState.value
    && gameState.value.phase === 'playing',
  )
}

function onBeforeUnload(event: BeforeUnloadEvent) {
  if (!shouldResignOnPageExit()) return

  event.preventDefault()
  event.returnValue = ''
}

function sendUnloadResign() {
  if (!shouldResignOnPageExit() || unloadResignSent.value) return

  unloadResignSent.value = true
  api.sendResignBeacon(currentRoom.value!.id, localPlayer.value!)
}

function onPageHide() {
  sendUnloadResign()
}

function startRoomPolling(roomId: string, player: LobbyPlayer) {
  stopRoomPolling()
  roomPollTimer.value = window.setInterval(async () => {
    if (gameState.value) {
      stopRoomPolling()
      return
    }

    try {
      const room = await api.getRoom(roomId)
      currentRoom.value = room
      if (!room.game_id) return

      localPlayer.value = player
      gameState.value = await api.getGame(room.game_id)
      unloadResignSent.value = false
      multiplayerStatus.value = '상대가 입장해 게임을 시작합니다.'
      stopRoomPolling()
      startGamePolling(room.game_id)
    } catch {
      // Keep waiting; transient network failures should not close the room.
    }
  }, 1200)
}

onMounted(() => {
  window.addEventListener('beforeunload', onBeforeUnload)
  window.addEventListener('pagehide', onPageHide)
})

onUnmounted(() => {
  window.removeEventListener('beforeunload', onBeforeUnload)
  window.removeEventListener('pagehide', onPageHide)
  stopRoomPolling()
  stopGamePolling()
})

async function createMultiplayerRoom() {
  if (!activeSummary.value.valid) {
    lobbyError.value = '내 덱 구성이 유효해야 방을 만들 수 있습니다.'
    return
  }

  lobbyError.value = null
  multiplayerStatus.value = null
  try {
    const hostSide = hostSideMode.value === 'random' ? randomSide() : hostSideMode.value
    const room = await api.createRoom(
      selectedSize.value,
      hostSide,
      serializeNeutralDeck(activeDeck.value, 'white'),
    )
    currentRoom.value = room
    localPlayer.value = hostSide
    unloadResignSent.value = false
    roomCodeInput.value = room.id
    multiplayerStatus.value = `방 ${room.id} 생성 완료. 내 색상은 ${playerLabel(room.host_side)}입니다. 상대가 ${playerLabel(room.guest_side)} 덱으로 입장하면 게임이 시작됩니다.`
    startRoomPolling(room.id, hostSide)
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}

async function applyRoomByCode(roomCode: string): Promise<MultiplayerRoom> {
  const room = await api.getRoom(roomCode.toUpperCase())
  applyingRoomSize.value = true
  selectedSize.value = room.board_size
  currentRoom.value = room
  await nextTick()
  activePlayer.value = 'white'
  applyingRoomSize.value = false
  return room
}

async function refreshRoom() {
  if (!currentRoom.value) return

  lobbyError.value = null
  try {
    const room = await api.getRoom(currentRoom.value.id)
    currentRoom.value = room
    if (room.game_id) {
      localPlayer.value = localPlayer.value ?? room.guest_side
      gameState.value = await api.getGame(room.game_id)
      unloadResignSent.value = false
      multiplayerStatus.value = '상대가 입장해 게임을 시작합니다.'
      startGamePolling(room.game_id)
      stopRoomPolling()
    } else {
      multiplayerStatus.value = '아직 상대 입장을 기다리는 중입니다.'
    }
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}

async function joinMultiplayerRoom() {
  if (!currentRoom.value) {
    if (!roomCodeInput.value.trim()) {
      lobbyError.value = '방 번호를 입력하세요.'
      return
    }

    try {
      await applyRoomByCode(roomCodeInput.value)
    } catch (e: unknown) {
      applyingRoomSize.value = false
      const message = e instanceof Error ? e.message : String(e)
      lobbyError.value = message
      multiplayerStatus.value = `입장 실패: ${message}`
      return
    }
  }
  const room = currentRoom.value
  if (!room) {
    lobbyError.value = '방 정보를 불러오지 못했습니다.'
    return
  }
  if (!activeSummary.value.valid) {
    lobbyError.value = '내 덱 구성이 유효해야 입장할 수 있습니다.'
    return
  }

  lobbyError.value = null
  multiplayerStatus.value = null
  try {
    const { state } = await api.joinRoom(room.id, serializeNeutralDeck(activeDeck.value, 'white'))
    localPlayer.value = room.guest_side
    currentRoom.value = { ...room, game_id: state.id }
    unloadResignSent.value = false
    gameState.value = state
    startGamePolling(state.id)
    stopRoomPolling()
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
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.22);
}

.env-banner-local {
  background: #74d4ff;
}

.env-banner-test {
  background: #ffd45f;
}

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

.hero-en {
  font-size: 0.5em;
  color: var(--muted);
  font-weight: 400;
  letter-spacing: 0.04em;
}

.mode-tabs {
  display: flex;
  gap: 10px;
}

.mode-tab {
  border: 1px solid var(--line);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  cursor: pointer;
  padding: 12px 18px;
  font-weight: 700;
}

.mode-tab.active {
  background: rgba(217, 164, 65, 0.2);
  border-color: rgba(217, 164, 65, 0.52);
  color: #f4dfb0;
}

.subtitle,
.section-description,
.summary-status,
.meta small {
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

.board-select select:disabled {
  opacity: 0.65;
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
.tool-button {
  border: none;
  cursor: pointer;
}

.btn-secondary {
  padding: 12px 18px;
  border-radius: 12px;
  background: #243142;
  color: var(--text);
}

.btn-secondary:disabled,
.player-tab:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.multiplayer-panel {
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.bot-panel {
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.bot-options {
  display: grid;
  grid-template-columns: minmax(260px, 1fr) minmax(260px, 1fr) minmax(150px, 0.5fr);
  gap: 14px;
}

.difficulty-select,
.bot-opponent {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 8px;
  padding: 12px 14px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
}

.difficulty-select select {
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--line);
  background: #0d1520;
  color: var(--text);
}

.bot-opponent strong {
  color: #f4dfb0;
  font-size: 1.25rem;
}

.room-grid {
  display: grid;
  grid-template-columns: minmax(280px, 1fr) minmax(260px, 0.8fr);
  gap: 16px;
  align-items: stretch;
}

.room-actions {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.color-match {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  padding: 12px 14px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
}

.color-match label {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text);
}

.color-match input {
  accent-color: var(--accent);
}

.room-code-row,
.room-buttons {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.room-code-input {
  min-width: 160px;
  flex: 1;
  padding: 12px 14px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: #0d1520;
  color: var(--text);
  font-size: 18px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.room-state {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
}

.room-state strong {
  color: #f4dfb0;
  font-size: 1.6rem;
  letter-spacing: 0.08em;
}

.room-state p,
.room-status {
  color: var(--muted);
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

.preset-panel {
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.preset-list {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}

.preset-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 14px 16px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  cursor: pointer;
  text-align: left;
}

.preset-card:hover {
  border-color: rgba(217, 164, 65, 0.52);
  background: rgba(217, 164, 65, 0.12);
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

.piece-search {
  width: 100%;
  padding: 11px 12px;
  border-radius: 10px;
  border: 1px solid var(--line);
  background: #0d1520;
  color: var(--text);
}

.piece-catalog {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 540px;
  overflow-y: auto;
  padding-right: 2px;
}

.catalog-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.catalog-section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--muted);
  font-size: 12px;
  font-weight: 700;
  text-transform: uppercase;
}

.catalog-section-title small {
  color: var(--accent);
}

.catalog-title-label,
.palette-piece {
  position: relative;
}

.catalog-title-label.has-tooltip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  outline: none;
}

.tooltip-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: rgba(217, 164, 65, 0.18);
  color: #f4dfb0;
  font-size: 11px;
}

.variant-tooltip,
.piece-tooltip {
  position: absolute;
  z-index: 20;
  display: none;
  width: min(280px, 70vw);
  padding: 9px 10px;
  border: 1px solid rgba(244, 223, 176, 0.24);
  border-radius: 8px;
  background: #111a27;
  color: var(--text);
  box-shadow: 0 12px 34px rgba(0, 0, 0, 0.34);
  font-size: 12px;
  font-weight: 500;
  line-height: 1.45;
  text-transform: none;
}

.variant-tooltip {
  left: 0;
  top: calc(100% + 8px);
}

.piece-tooltip {
  left: 10px;
  right: 10px;
  bottom: calc(100% + 8px);
}

.catalog-title-label.has-tooltip:hover .variant-tooltip,
.catalog-title-label.has-tooltip:focus-visible .variant-tooltip,
.palette-piece:hover .piece-tooltip,
.palette-piece:focus-visible .piece-tooltip {
  display: block;
}

.piece-palette,
.pocket-summary {
  display: grid;
  gap: 10px;
}

.palette-piece {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
}

.palette-piece {
  min-height: 68px;
  text-align: left;
}

.palette-piece[draggable="true"] {
  cursor: grab;
}

.palette-piece[draggable="true"]:active {
  cursor: grabbing;
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
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
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

.meta {
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

.placement-controls {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 10px;
  align-items: stretch;
}

.tool-button,
.selected-tool {
  min-height: 54px;
  border-radius: 12px;
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
  border-radius: 12px;
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
  font-size: clamp(24px, 2.5vw, 38px);
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
  border-radius: 12px;
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
  grid-template-columns: repeat(auto-fit, minmax(82px, 1fr));
}

.pocket-chip,
.pocket-empty {
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.04);
}

.pocket-chip {
  min-height: 58px;
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: center;
  gap: 8px;
  padding: 10px;
}

.pocket-chip button {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: #243142;
  color: var(--text);
  border: none;
  cursor: pointer;
}

.pocket-empty {
  padding: 14px;
  color: var(--muted);
  text-align: center;
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
  .summary-grid,
  .room-grid,
  .bot-options {
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
