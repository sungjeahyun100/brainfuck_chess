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
          {{ isMyTurn ? '내 턴' : '상대 턴' }}
        </span>
        <span class="turn-badge">Turn {{ state.turn_number }}</span>
        <span class="mode-badge" v-if="state.turn_state.mode !== 'undecided'">
          {{ state.turn_state.mode === 'move' ? '🏃 Move' : '🎯 Drop' }}
        </span>
      </div>
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

    <div class="main-layout">
      <!-- Left: Pocket (White) -->
      <div class="pocket">
        <h4>⬜ White Pocket</h4>
        <div class="pocket-pieces">
          <div
            v-for="pid in whitePocket"
            :key="pid"
            class="pocket-piece"
            :class="{ selected: selectedPocketPieceId === pid }"
            @click="onPocketClick(pid)"
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
            @click="onPocketClick(pid)"
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
        :disabled="!isMyTurn || state.turn_state.actions.length === 0 || state.phase === 'ended'"
        @click="onEndTurn"
      >
        End Turn
      </button>
      <div class="turn-log">
        <small>Actions this turn: {{ state.turn_state.actions.length }}</small>
      </div>
    </div>

    <div v-if="error" class="error-banner">{{ error }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { GameState, PlayerId, Square, MoveAction, DropAction } from '../types/game'
import { api } from '../api/gameApi'
import Board from './Board.vue'

const props = defineProps<{
  state: GameState
  localPlayer?: PlayerId | null
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

const whitePocket = computed(() =>
  props.state.players['white']?.deck.pocket_pieces ?? []
)
const blackPocket = computed(() =>
  props.state.players['black']?.deck.pocket_pieces ?? []
)
const whiteDeck = computed(() => props.state.players['white']?.deck)
const blackDeck = computed(() => props.state.players['black']?.deck)
const isMyTurn = computed(() => !props.localPlayer || props.state.current_player === props.localPlayer)

const PIECE_SYMBOLS: Record<string, string> = {
  king: '♔', queen: '♕', rook: '♖', bishop: '♗', knight: '♘',
  'pawn-white': '♙', 'pawn-black': '♟',
}
function pieceSymbol(typeId: string): string {
  return PIECE_SYMBOLS[typeId] ?? '?'
}

function clearSelection() {
  selectedPieceId.value = null
  selectedPocketPieceId.value = null
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []
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
    const isDroppable = dropSquares.value.some(s => s.file === sq.file && s.rank === sq.rank)
    if (isDroppable) {
      try {
        const action: DropAction = {
          type: 'drop',
          player_id: currentPlayer,
          piece_id: selectedPocketPieceId.value,
          to: sq,
        }
        const newState = await api.submitAction(props.state.id, action)
        emit('stateUpdate', newState)
      } catch (e: unknown) {
        error.value = e instanceof Error ? e.message : String(e)
      }
    }
    clearSelection()
    return
  }

  // ── Move mode: selected piece → move to target ──
  if (selectedPieceId.value) {
    const isLegalTarget = legalTargetSquares.value.some(s => s.file === sq.file && s.rank === sq.rank)
    if (isLegalTarget) {
      const fromPiece = props.state.pieces[selectedPieceId.value]
      if (fromPiece?.current_square) {
        try {
          const capturedPieceId = props.state.board.squares[sqId] ?? undefined
          const action: MoveAction = {
            type: 'move',
            player_id: currentPlayer,
            piece_id: selectedPieceId.value,
            from: fromPiece.current_square,
            to: sq,
            captured_piece_id: capturedPieceId ?? undefined,
          }
          const newState = await api.submitAction(props.state.id, action)
          emit('stateUpdate', newState)
        } catch (e: unknown) {
          error.value = e instanceof Error ? e.message : String(e)
        }
      }
    }
    clearSelection()
    return
  }

  // ── Select own piece ──
  if (piece && piece.owner === currentPlayer && piece.move_stack > 0) {
    clearSelection()
    selectedPieceId.value = pieceId

    try {
      const [{ moves }, { squares }] = await Promise.all([
        api.getLegalMoves(props.state.id),
        api.getPieceAttacks(props.state.id, selectedPieceId.value as string),
      ])
      legalTargetSquares.value = moves
        .filter(m => m.piece_id === pieceId)
        .map(m => m.to)
      movableSquares.value = moves
        .filter(m => m.piece_id === pieceId && !m.captured_piece_id)
        .map(m => m.to)
      attackSquares.value = squares
    } catch {
      legalTargetSquares.value = []
      movableSquares.value = []
      attackSquares.value = []
    }
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

  clearSelection()
  selectedPocketPieceId.value = pieceId
  try {
    const { drops } = await api.getLegalDrops(props.state.id)
    dropSquares.value = drops.filter(d => d.piece_id === pieceId).map(d => d.to)
  } catch {
    dropSquares.value = []
  }
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
</script>

<style scoped>
.game-screen { display: flex; flex-direction: column; gap: 12px; padding: 16px; position: relative; }

.header { display: flex; align-items: center; gap: 16px; }
.player-badge { padding: 4px 10px; border-radius: 6px; font-weight: bold; }
.player-badge.player-white { background: #eee; color: #333; }
.player-badge.player-black { background: #333; color: #eee; }
.turn-badge, .mode-badge { padding: 4px 8px; background: #ddd; border-radius: 6px; }
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

.main-layout { display: flex; gap: 16px; align-items: flex-start; }

.pocket { min-width: 120px; display: flex; flex-direction: column; gap: 8px; }
.pocket h4 { margin: 0; font-size: 14px; }
.pocket-pieces { display: flex; flex-wrap: wrap; gap: 6px; }
.pocket-piece {
  width: 44px; height: 44px; display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  border: 2px solid #bbb; border-radius: 6px; cursor: pointer;
  font-size: 22px; background: #f9f9f9;
}
.pocket-piece.selected { border-color: #4a8fff; background: #e0eeff; }
.score-info { font-size: 12px; color: #666; }

.footer { display: flex; align-items: center; gap: 16px; }
.btn { padding: 8px 16px; border-radius: 6px; border: none; cursor: pointer; font-size: 14px; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.btn-end-turn { background: #4caf50; color: white; }
.btn-end-turn:hover:not(:disabled) { background: #388e3c; }

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
}
.game-over-box button { margin-top: 16px; padding: 10px 24px; background: #1976d2; color: white; border: none; border-radius: 6px; cursor: pointer; font-size: 16px; }
</style>
