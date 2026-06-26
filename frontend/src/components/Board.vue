<template>
  <div class="board-wrapper">
    <div
      class="board"
      :style="{ '--size': board.size }"
    >
      <div
        v-for="sq in allSquares"
        :key="sq.id"
        class="square"
        :class="squareClasses(sq)"
        :data-file="sq.file"
        :data-rank="sq.rank"
        @click="onSquareClick(sq)"
        @pointerdown="onSquarePointerDown($event, sq)"
        @dragover.prevent
        @drop.prevent="onNativeDrop($event, sq)"
      >
        <span v-if="legalMarker(sq)" class="legal-move-dot" :class="legalMarker(sq)" />
        <span v-if="sq.piece" class="piece" :class="`owner-${sq.piece.owner}`">
          {{ pieceSymbol(sq.piece.type_id) }}
        </span>
        <span v-if="sq.moveStack !== undefined && sq.moveStack > 0" class="stack-badge">
          {{ sq.moveStack }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import type { Board, Piece, Square } from '../types/game'

interface SquareInfo {
  id: string
  file: number
  rank: number
  piece?: Piece
  moveStack?: number
  isLight: boolean
}

const props = defineProps<{
  board: Board
  pieces: Record<string, Piece>
  selectedPieceId: string | null
  movableSquares: Square[]
  attackSquares: Square[]
  dropSquares: Square[]
}>()

const emit = defineEmits<{
  squareClick: [square: Square]
  pieceDragStart: [pieceId: string]
  squareDrop: [square: Square | null, pieceId: string]
}>()

function squareId(file: number, rank: number) {
  return `${file}_${rank}`
}

const allSquares = computed((): SquareInfo[] => {
  const squares: SquareInfo[] = []
  // Render from rank n-1 down to 0 (top = high rank) for proper chess orientation
  for (let rank = props.board.size - 1; rank >= 0; rank--) {
    for (let file = 0; file < props.board.size; file++) {
      const id = squareId(file, rank)
      const pieceId = props.board.squares[id] ?? null
      const piece = pieceId ? props.pieces[pieceId] : undefined
      squares.push({
        id,
        file,
        rank,
        piece,
        moveStack: piece?.move_stack,
        isLight: (file + rank) % 2 === 1,
      })
    }
  }
  return squares
})

const movableSquareIds = computed(() => new Set(props.movableSquares.map(squareIdFromSquare)))
const attackSquareIds = computed(() => new Set(props.attackSquares.map(squareIdFromSquare)))
const dropSquareIds = computed(() => new Set(props.dropSquares.map(squareIdFromSquare)))
const draggingPieceId = ref<string | null>(null)
const dragOverSquareId = ref<string | null>(null)
const pointerDrag = ref<{
  pointerId: number
  pieceId: string
  startX: number
  startY: number
  active: boolean
} | null>(null)
let suppressNextClick = false

function squareIdFromSquare(square: Square) {
  return squareId(square.file, square.rank)
}

function squareClasses(sq: SquareInfo) {
  const classes: string[] = [sq.isLight ? 'light' : 'dark']

  if (sq.piece && sq.piece.id === props.selectedPieceId) {
    classes.push('selected')
  }
  if (sq.piece?.id === draggingPieceId.value) {
    classes.push('dragging')
  }
  if (dragOverSquareId.value === sq.id) {
    classes.push('drag-over')
  }
  return classes
}

function legalMarker(sq: SquareInfo): string | null {
  if (attackSquareIds.value.has(sq.id)) return 'capture'
  if (movableSquareIds.value.has(sq.id)) return 'move'
  if (dropSquareIds.value.has(sq.id)) return 'drop'
  return null
}

function onSquareClick(sq: SquareInfo) {
  if (suppressNextClick) {
    suppressNextClick = false
    return
  }
  emit('squareClick', { file: sq.file, rank: sq.rank })
}

function onSquarePointerDown(event: PointerEvent, sq: SquareInfo) {
  if (event.button !== 0 || !sq.piece) return

  pointerDrag.value = {
    pointerId: event.pointerId,
    pieceId: sq.piece.id,
    startX: event.clientX,
    startY: event.clientY,
    active: false,
  }
  window.addEventListener('pointermove', onWindowPointerMove)
  window.addEventListener('pointerup', onWindowPointerUp)
  window.addEventListener('pointercancel', onWindowPointerCancel)
}

function onWindowPointerMove(event: PointerEvent) {
  const drag = pointerDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY)
  if (!drag.active && distance < 6) return

  if (!drag.active) {
    drag.active = true
    draggingPieceId.value = drag.pieceId
    emit('pieceDragStart', drag.pieceId)
  }

  dragOverSquareId.value = squareIdFromPoint(event.clientX, event.clientY)
}

function onWindowPointerUp(event: PointerEvent) {
  const drag = pointerDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  const targetSquareId = squareIdFromPoint(event.clientX, event.clientY)
  cleanupPointerDrag()

  if (!drag.active) return

  suppressNextClick = true
  emit('squareDrop', squareFromId(targetSquareId), drag.pieceId)
}

function onWindowPointerCancel(event: PointerEvent) {
  const drag = pointerDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  const pieceId = drag.pieceId
  const wasActive = drag.active
  cleanupPointerDrag()
  if (wasActive) emit('squareDrop', null, pieceId)
}

function cleanupPointerDrag() {
  pointerDrag.value = null
  draggingPieceId.value = null
  dragOverSquareId.value = null
  window.removeEventListener('pointermove', onWindowPointerMove)
  window.removeEventListener('pointerup', onWindowPointerUp)
  window.removeEventListener('pointercancel', onWindowPointerCancel)
}

function squareIdFromPoint(clientX: number, clientY: number): string | null {
  const element = document.elementFromPoint(clientX, clientY)
  const square = element?.closest<HTMLElement>('.square')
  if (!square) return null

  const file = Number(square.dataset.file)
  const rank = Number(square.dataset.rank)
  return Number.isFinite(file) && Number.isFinite(rank) ? squareId(file, rank) : null
}

function squareFromId(id: string | null): Square | null {
  if (!id) return null

  const [file, rank] = id.split('_').map(Number)
  if (!Number.isFinite(file) || !Number.isFinite(rank)) return null

  return { file, rank }
}

function onNativeDrop(event: DragEvent, sq: SquareInfo) {
  const pieceId = event.dataTransfer?.getData('application/x-brainfuck-chess-pocket-piece')
    || event.dataTransfer?.getData('text/plain')
    || null
  if (!pieceId) return

  emit('squareDrop', { file: sq.file, rank: sq.rank }, pieceId)
}

onBeforeUnmount(() => {
  cleanupPointerDrag()
})

const PIECE_SYMBOLS: Record<string, string> = {
  'king': '♔',
  'queen': '♕',
  'amazon': 'A',
  'tempest-rook': 'T',
  'bouncing-bishop': 'B',
  'rook': '♖',
  'bishop': '♗',
  'knight': '♘',
  'pawn-white': '♙',
  'pawn-black': '♟',
}

function pieceSymbol(typeId: string): string {
  return PIECE_SYMBOLS[typeId] ?? '?'
}
</script>

<style scoped>
.board-wrapper {
  display: flex;
  justify-content: center;
  align-items: center;
}

.board {
  display: grid;
  grid-template-columns: repeat(var(--size), 1fr);
  grid-template-rows: repeat(var(--size), 1fr);
  border: 2px solid #555;
  width: min(80vw, 80vh);
  aspect-ratio: 1;
}

.square {
  position: relative;
  display: flex;
  justify-content: center;
  align-items: center;
  cursor: pointer;
  user-select: none;
  touch-action: none;
  min-width: 0;
  min-height: 0;
}

.square.light { background: #f0d9b5; }
.square.dark  { background: #b58863; }

.square.selected::before,
.square.drag-over::before {
  content: '';
  position: absolute;
  inset: 4px;
  border: 2px solid rgba(246, 246, 105, 0.82);
  border-radius: 4px;
  pointer-events: none;
}

.square.drag-over::before {
  border-color: rgba(74, 143, 255, 0.82);
}

.legal-move-dot {
  position: absolute;
  left: 50%;
  top: 50%;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  transform: translate(-50%, -50%);
  pointer-events: none;
  z-index: 3;
  background: rgba(33, 150, 83, 0.72);
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.18);
}

.legal-move-dot.capture {
  background: rgba(220, 50, 50, 0.78);
}

.legal-move-dot.drop {
  background: rgba(74, 143, 255, 0.78);
}

.piece {
  font-size: clamp(16px, 4vw, 48px);
  line-height: 1;
  pointer-events: none;
  position: relative;
  z-index: 2;
  transition: opacity 80ms ease, transform 80ms ease;
}

.square.dragging .piece {
  opacity: 0.55;
  transform: scale(0.96);
}

.piece.owner-white { color: #fff; text-shadow: 0 0 2px #333; }
.piece.owner-black { color: #111; text-shadow: 0 0 2px #ccc; }

.stack-badge {
  position: absolute;
  top: 2px;
  right: 3px;
  font-size: 10px;
  background: rgba(0,0,0,0.5);
  color: #fff;
  border-radius: 3px;
  padding: 0 2px;
}
</style>
