<template>
  <div class="game-screen">
    <!-- Header -->
    <div class="header">
      <h2>Brainfuck Chess</h2>
      <div class="turn-info">
        <span class="player-badge" :class="`player-${state.current_player}`">
          {{ state.current_player === 'white' ? '⬜ White' : '⬛ Black' }}
        </span>
        <span v-if="localPlayer" class="local-badge" :class="{ waiting: !isMyTurn }">
          {{ isBotTurn ? '봇 턴' : isMyTurn ? '내 턴' : '상대 턴' }}
        </span>
        <span v-if="botPlayer" class="bot-badge">🤖 {{ botDifficultyLabel }}</span>
        <span class="turn-badge">Turn {{ state.turn_number }}</span>
        <span class="mode-badge" v-if="state.turn_state.mode !== 'undecided'">
          {{ state.turn_state.mode === 'move' ? '🏃 Move' : '🎯 Drop' }}
        </span>
      </div>
    </div>

    <div v-if="botPlayer" class="bot-status" :class="{ thinking: botThinking, failed: Boolean(botError) }" aria-live="polite">
      <div>
        <strong>{{ botThinking ? '봇이 수를 계산하고 있습니다…' : botError ? '봇 턴 실행 실패' : '봇 대전' }}</strong>
        <small v-if="lastBotStats">
          최근 탐색 {{ lastBotStats.searched_nodes.toLocaleString() }}노드 · 깊이 {{ lastBotStats.depth_reached }} · {{ lastBotStats.elapsed_ms }}ms
        </small>
        <small v-else>{{ playerName(botPlayer) }} 봇 · {{ botDifficultyLabel }}</small>
      </div>
      <button v-if="botError && isBotTurn && !botThinking" @click="runBotTurn">다시 시도</button>
    </div>

    <!-- Game over overlay -->
    <div v-if="state.phase === 'ended'" class="game-over-overlay">
      <div class="game-over-box">
        <h2>Game Over</h2>
        <p v-if="state.result?.winner">
          {{ state.result.winner === 'white' ? '⬜ White' : '⬛ Black' }} wins!
          <br><small>({{ state.result.reason }})</small>
        </p>
        <p v-else>Draw</p>
        <button @click="$emit('restart')">New Game</button>
      </div>
    </div>

    <div class="main-layout" :class="{ locked: botThinking || isBotTurn }">
      <!-- Left: Pocket (White) -->
      <div class="pocket">
        <h4>⬜ White Pocket</h4>
        <div class="pocket-pieces">
          <div
            v-for="pid in whitePocket"
            :key="pid"
            class="pocket-piece"
            :class="{ selected: selectedPocketPieceId === pid }"
            draggable="true"
            @click="onPocketClick(pid)"
            @dragstart="onPocketDragStart($event, pid)"
            @dragend="onPocketDragEnd"
          >
            {{ pieceSymbol(state.pieces[pid]?.type_id) }}
            <small>{{ state.piece_definitions[state.pieces[pid]?.type_id]?.score }}pt</small>
          </div>
        </div>
        <div class="score-info" v-if="whiteDeck">
          <span>{{ whiteDeck.total_score }} / {{ whiteDeck.score_limit }} pts</span>
        </div>
      </div>

      <!-- Center: Board -->
      <Board
        :board="state.board"
        :pieces="state.pieces"
        :selected-piece-id="selectedPieceId"
        :movable-squares="movableSquares"
        :attack-squares="attackSquares"
        :drop-squares="dropSquares"
        @square-click="onSquareClick"
        @piece-drag-start="onBoardPieceDragStart"
        @square-drop="onSquareDrop"
      />

      <!-- Right: Pocket (Black) -->
      <div class="pocket">
        <h4>⬛ Black Pocket</h4>
        <div class="pocket-pieces">
          <div
            v-for="pid in blackPocket"
            :key="pid"
            class="pocket-piece"
            :class="{ selected: selectedPocketPieceId === pid }"
            draggable="true"
            @click="onPocketClick(pid)"
            @dragstart="onPocketDragStart($event, pid)"
            @dragend="onPocketDragEnd"
          >
            {{ pieceSymbol(state.pieces[pid]?.type_id) }}
            <small>{{ state.piece_definitions[state.pieces[pid]?.type_id]?.score }}pt</small>
          </div>
        </div>
        <div class="score-info" v-if="blackDeck">
          <span>{{ blackDeck.total_score }} / {{ blackDeck.score_limit }} pts</span>
        </div>
      </div>
    </div>

    <!-- Footer: actions -->
    <div class="footer">
      <button
        class="btn btn-end-turn"
        :disabled="!isMyTurn || botThinking || state.turn_state.actions.length === 0 || state.phase === 'ended'"
        @click="onEndTurn"
      >
        End Turn
      </button>
      <button
        class="btn btn-resign"
        :disabled="botThinking || state.phase === 'ended' || (Boolean(roomId) && !localPlayer)"
        @click="onResign"
      >
        기권
      </button>
      <div class="turn-log">
        <small>Actions this turn: {{ state.turn_state.actions.length }}</small>
      </div>
    </div>

    <div v-if="error || botError" class="error-banner">{{ error || botError }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type {
  BotDifficulty,
  BotTurnStats,
  DropAction,
  GameState,
  MoveAction,
  PlayerId,
  Square,
} from '../types/game'
import { api } from '../api/gameApi'
import Board from './Board.vue'

const props = defineProps<{
  state: GameState
  localPlayer?: PlayerId | null
  roomId?: string | null
  botPlayer?: PlayerId | null
  botDifficulty?: BotDifficulty
}>()
const emit = defineEmits<{
  stateUpdate: [state: GameState]
  restart: []
}>()

const selectedPieceId = ref<string | null>(null)
const selectedPocketPieceId = ref<string | null>(null)
const legalTargetSquares = ref<Square[]>([])
const movableSquares = ref<Square[]>([])
const attackSquares = ref<Square[]>([])
const dropSquares = ref<Square[]>([])
const error = ref<string | null>(null)
const botError = ref<string | null>(null)
const botThinking = ref(false)
const lastBotStats = ref<BotTurnStats | null>(null)
const draggedPocketPieceId = ref<string | null>(null)

interface LegalPieceOptions {
  legalTargets: Square[]
  movable: Square[]
  captures: Square[]
}

const pieceOptionsCache = new Map<string, LegalPieceOptions>()
const pieceOptionsRequests = new Map<string, Promise<LegalPieceOptions>>()
const dropOptionsCache = new Map<string, DropAction[]>()
const dropOptionsRequests = new Map<string, Promise<DropAction[]>>()

const whitePocket = computed(() =>
  props.state.players['white']?.deck.pocket_pieces ?? []
)
const blackPocket = computed(() =>
  props.state.players['black']?.deck.pocket_pieces ?? []
)
const whiteDeck = computed(() => props.state.players['white']?.deck)
const blackDeck = computed(() => props.state.players['black']?.deck)
const isMyTurn = computed(() => !props.localPlayer || props.state.current_player === props.localPlayer)
const isBotTurn = computed(() => Boolean(
  props.botPlayer
  && props.state.current_player === props.botPlayer
  && props.state.phase === 'playing',
))
const botDifficultyLabel = computed(() => {
  const labels: Record<BotDifficulty, string> = {
    easy: 'Easy',
    normal: 'Normal',
    hard: 'Hard',
  }
  return labels[props.botDifficulty ?? 'normal']
})

function playerName(player: PlayerId): string {
  return player === 'white' ? 'White' : 'Black'
}

async function runBotTurn() {
  if (!props.botPlayer || !isBotTurn.value || botThinking.value) return

  botThinking.value = true
  botError.value = null
  clearSelection()
  try {
    const response = await api.botTurn(
      props.state.id,
      props.botPlayer,
      props.botDifficulty ?? 'normal',
    )
    lastBotStats.value = response.stats
    emit('stateUpdate', response.game_state)
  } catch (e: unknown) {
    botError.value = e instanceof Error ? e.message : String(e)
  } finally {
    botThinking.value = false
  }
}

watch(
  () => [
    props.state.id,
    props.state.current_player,
    props.state.turn_number,
    props.state.phase,
    props.botPlayer,
    props.botDifficulty,
  ],
  () => {
    if (isBotTurn.value) void runBotTurn()
  },
  { immediate: true },
)

const PIECE_SYMBOLS: Record<string, string> = {
  king: '♔', queen: '♕', rook: '♖', bishop: '♗', knight: '♘',
  amazon: 'A', 'tempest-rook': 'T', 'bouncing-bishop': 'B',
  'pawn-white': '♙', 'pawn-black': '♟',
}
function pieceSymbol(typeId: string): string {
  return PIECE_SYMBOLS[typeId] ?? '?'
}

function clearSelection() {
  selectedPieceId.value = null
  selectedPocketPieceId.value = null
  draggedPocketPieceId.value = null
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []
}

function actionCacheKey(pieceId?: string): string {
  return [
    props.state.id,
    props.state.current_player,
    props.state.turn_number,
    props.state.turn_state.mode,
    props.state.turn_state.actions.length,
    pieceId ?? '',
  ].join(':')
}

function sameSquare(left: Square, right: Square): boolean {
  return left.file === right.file && left.rank === right.rank
}

function isLegalSquare(square: Square, legalSquares: Square[]): boolean {
  return legalSquares.some(target => sameSquare(target, square))
}

async function loadPieceOptions(pieceId: string): Promise<LegalPieceOptions> {
  const key = actionCacheKey(pieceId)
  const cached = pieceOptionsCache.get(key)
  if (cached) return cached

  const pending = pieceOptionsRequests.get(key)
  if (pending) return pending

  const request = api.getPieceOptions(props.state.id, pieceId).then(({ moves }) => {
    const options: LegalPieceOptions = {
      legalTargets: moves.map(move => move.to),
      movable: moves.filter(move => !move.captured_piece_id).map(move => move.to),
      captures: moves.filter(move => Boolean(move.captured_piece_id)).map(move => move.to),
    }
    pieceOptionsCache.set(key, options)
    pieceOptionsRequests.delete(key)
    return options
  }).catch(error => {
    pieceOptionsRequests.delete(key)
    throw error
  })

  pieceOptionsRequests.set(key, request)
  return request
}

async function selectBoardPiece(pieceId: string): Promise<LegalPieceOptions | null> {
  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player || piece.move_stack <= 0) {
    clearSelection()
    return null
  }

  selectedPieceId.value = pieceId
  selectedPocketPieceId.value = null
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []

  try {
    const options = await loadPieceOptions(pieceId)
    if (selectedPieceId.value !== pieceId) return options

    legalTargetSquares.value = options.legalTargets
    movableSquares.value = options.movable
    attackSquares.value = options.captures
    return options
  } catch {
    if (selectedPieceId.value === pieceId) {
      legalTargetSquares.value = []
      movableSquares.value = []
      attackSquares.value = []
    }
    return null
  }
}

async function loadDropOptions(): Promise<DropAction[]> {
  const key = actionCacheKey('drops')
  const cached = dropOptionsCache.get(key)
  if (cached) return cached

  const pending = dropOptionsRequests.get(key)
  if (pending) return pending

  const request = api.getLegalDrops(props.state.id).then(({ drops }) => {
    dropOptionsCache.set(key, drops)
    dropOptionsRequests.delete(key)
    return drops
  }).catch(error => {
    dropOptionsRequests.delete(key)
    throw error
  })

  dropOptionsRequests.set(key, request)
  return request
}

async function selectPocketPiece(pieceId: string): Promise<Square[]> {
  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player || props.state.turn_state.mode === 'move') {
    clearSelection()
    return []
  }

  selectedPieceId.value = null
  selectedPocketPieceId.value = pieceId
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []

  try {
    const drops = await loadDropOptions()
    const targets = drops.filter(drop => drop.piece_id === pieceId).map(drop => drop.to)
    if (selectedPocketPieceId.value === pieceId) {
      dropSquares.value = targets
    }
    return targets
  } catch {
    if (selectedPocketPieceId.value === pieceId) {
      dropSquares.value = []
    }
    return []
  }
}

async function submitMove(pieceId: string, to: Square) {
  const fromPiece = props.state.pieces[pieceId]
  if (!fromPiece?.current_square || sameSquare(fromPiece.current_square, to)) {
    clearSelection()
    return
  }

  const options = selectedPieceId.value === pieceId && legalTargetSquares.value.length > 0
    ? { legalTargets: legalTargetSquares.value }
    : await selectBoardPiece(pieceId)
  if (!options || !isLegalSquare(to, options.legalTargets)) {
    clearSelection()
    return
  }

  const sqId = `${to.file}_${to.rank}`
  try {
    const capturedPieceId = props.state.board.squares[sqId] ?? undefined
    const action: MoveAction = {
      type: 'move',
      player_id: props.state.current_player,
      piece_id: pieceId,
      from: fromPiece.current_square,
      to,
      captured_piece_id: capturedPieceId ?? undefined,
    }
    const newState = await api.submitAction(props.state.id, action)
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    clearSelection()
  }
}

async function submitDrop(pieceId: string, to: Square) {
  const targets = selectedPocketPieceId.value === pieceId && dropSquares.value.length > 0
    ? dropSquares.value
    : await selectPocketPiece(pieceId)
  if (!isLegalSquare(to, targets)) {
    clearSelection()
    return
  }

  try {
    const action: DropAction = {
      type: 'drop',
      player_id: props.state.current_player,
      piece_id: pieceId,
      to,
    }
    const newState = await api.submitAction(props.state.id, action)
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    clearSelection()
  }
}

async function onSquareClick(sq: Square) {
  error.value = null
  if (!isMyTurn.value) {
    error.value = '상대 턴입니다.'
    clearSelection()
    return
  }

  const currentPlayer = props.state.current_player
  const sqId = `${sq.file}_${sq.rank}`
  const pieceId = props.state.board.squares[sqId] ?? null
  const piece = pieceId ? props.state.pieces[pieceId] : null

  // ── Drop mode: selected pocket piece → drop on target ──
  if (selectedPocketPieceId.value) {
    await submitDrop(selectedPocketPieceId.value, sq)
    return
  }

  // ── Move mode: selected piece → move to target ──
  if (selectedPieceId.value) {
    await submitMove(selectedPieceId.value, sq)
    return
  }

  // ── Select own piece ──
  if (pieceId && piece && piece.owner === currentPlayer && piece.move_stack > 0) {
    await selectBoardPiece(pieceId)
  } else {
    clearSelection()
  }
}

async function onPocketClick(pieceId: string) {
  error.value = null
  if (!isMyTurn.value) {
    error.value = '상대 턴입니다.'
    clearSelection()
    return
  }

  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player) return
  if (props.state.turn_state.mode === 'move') return

  await selectPocketPiece(pieceId)
}

function onBoardPieceDragStart(pieceId: string) {
  error.value = null
  if (!isMyTurn.value) {
    clearSelection()
    return
  }

  void selectBoardPiece(pieceId)
}

async function onSquareDrop(sq: Square | null, pieceId: string) {
  error.value = null
  if (!isMyTurn.value || !sq) {
    clearSelection()
    return
  }

  const piece = props.state.pieces[pieceId]
  if (!piece) {
    clearSelection()
    return
  }

  if (piece.in_pocket || draggedPocketPieceId.value === pieceId) {
    await submitDrop(pieceId, sq)
  } else {
    await submitMove(pieceId, sq)
  }
}

function onPocketDragStart(event: DragEvent, pieceId: string) {
  error.value = null
  if (!isMyTurn.value || props.state.turn_state.mode === 'move') {
    event.preventDefault()
    clearSelection()
    return
  }

  draggedPocketPieceId.value = pieceId
  event.dataTransfer?.setData('application/x-brainfuck-chess-pocket-piece', pieceId)
  event.dataTransfer?.setData('text/plain', pieceId)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
  void selectPocketPiece(pieceId)
}

function onPocketDragEnd() {
  draggedPocketPieceId.value = null
}

async function onEndTurn() {
  error.value = null
  if (!isMyTurn.value) {
    error.value = '상대 턴입니다.'
    return
  }

  try {
    const newState = await api.endTurn(props.state.id)
    clearSelection()
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function onResign() {
  error.value = null
  if (props.state.phase === 'ended') return

  const resigningPlayer = props.localPlayer ?? props.state.current_player
  if (!window.confirm('정말 기권하시겠습니까?')) return

  try {
    const newState = props.roomId
      ? await api.resignRoom(props.roomId, resigningPlayer)
      : await api.resignGame(props.state.id, resigningPlayer)
    clearSelection()
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<style scoped>
.game-screen { display: flex; flex-direction: column; gap: 12px; padding: 16px; position: relative; }

.header { display: flex; align-items: center; gap: 16px; }
.player-badge { padding: 4px 10px; border-radius: 6px; font-weight: bold; }
.player-badge.player-white { background: #eee; color: #333; }
.player-badge.player-black { background: #333; color: #eee; }
.turn-badge, .mode-badge { padding: 4px 8px; background: #ddd; color: #1f2933; border-radius: 6px; }
.local-badge {
  padding: 4px 8px;
  background: #e8f5e9;
  color: #256029;
  border-radius: 6px;
  font-weight: 700;
}
.local-badge.waiting {
  background: #fff3cd;
  color: #7a5a00;
}
.bot-badge {
  padding: 4px 8px;
  background: #342a18;
  color: #f4dfb0;
  border: 1px solid rgba(217, 164, 65, 0.38);
  border-radius: 6px;
  font-weight: 700;
}

.bot-status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 16px;
  border: 1px solid rgba(217, 164, 65, 0.28);
  border-radius: 10px;
  background: rgba(217, 164, 65, 0.08);
  color: #f4dfb0;
}
.bot-status > div { display: flex; flex-direction: column; gap: 3px; }
.bot-status small { color: #a8b1c2; }
.bot-status.thinking { animation: bot-pulse 1.3s ease-in-out infinite alternate; }
.bot-status.failed { border-color: rgba(255, 125, 125, 0.55); }
.bot-status button {
  padding: 7px 12px;
  border: none;
  border-radius: 6px;
  background: #d9a441;
  color: #221a0d;
  cursor: pointer;
  font-weight: 700;
}
@keyframes bot-pulse {
  from { background: rgba(217, 164, 65, 0.06); }
  to { background: rgba(217, 164, 65, 0.16); }
}

.main-layout { display: flex; gap: 16px; align-items: flex-start; justify-content: center; }
.main-layout.locked { pointer-events: none; opacity: 0.78; }

.pocket { width: 132px; min-width: 120px; display: flex; flex-direction: column; gap: 8px; }
.pocket h4 { margin: 0; font-size: 14px; }
.pocket-pieces { display: flex; flex-wrap: wrap; gap: 6px; }
.pocket-piece {
  width: 44px; height: 44px; display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  border: 2px solid #bbb; border-radius: 6px; cursor: pointer;
  font-size: 22px; background: #f9f9f9; color: #1f2933;
  user-select: none;
}
.pocket-piece[draggable="true"] { cursor: grab; }
.pocket-piece[draggable="true"]:active { cursor: grabbing; }
.pocket-piece.selected { border-color: #4a8fff; background: #e0eeff; }
.score-info { font-size: 12px; color: #666; }

.footer { display: flex; align-items: center; gap: 16px; }
.btn { padding: 8px 16px; border-radius: 6px; border: none; cursor: pointer; font-size: 14px; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.btn-end-turn { background: #4caf50; color: white; }
.btn-end-turn:hover:not(:disabled) { background: #388e3c; }
.btn-resign { background: #c62828; color: white; }
.btn-resign:hover:not(:disabled) { background: #a61f1f; }

.error-banner {
  position: fixed; bottom: 16px; left: 50%; transform: translateX(-50%);
  background: #c62828; color: white; padding: 10px 20px; border-radius: 8px;
  font-size: 14px; z-index: 100;
}

.game-over-overlay {
  position: absolute; inset: 0; background: rgba(0,0,0,0.5);
  display: flex; align-items: center; justify-content: center; z-index: 50;
}
.game-over-box {
  background: white; padding: 32px 48px; border-radius: 12px; text-align: center;
  color: #1f2933;
}
.game-over-box button { margin-top: 16px; padding: 10px 24px; background: #1976d2; color: white; border: none; border-radius: 6px; cursor: pointer; font-size: 16px; }

@media (max-width: 900px) {
  .game-screen { padding: 12px; }
  .header,
  .footer {
    flex-wrap: wrap;
  }
  .turn-info {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .main-layout {
    flex-wrap: wrap;
    align-items: stretch;
  }
  .pocket {
    order: 2;
    width: min(320px, 100%);
    flex: 1 1 220px;
  }
}
</style>
