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
        @click="onSquareClick(sq)"
      >
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
import { computed } from 'vue'
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
        isLight: (file + rank) % 2 === 0,
      })
    }
  }
  return squares
})

function squareClasses(sq: SquareInfo) {
  const classes: string[] = [sq.isLight ? 'light' : 'dark']

  if (sq.piece && sq.piece.id === props.selectedPieceId) {
    classes.push('selected')
  }
  if (props.movableSquares.some(s => s.file === sq.file && s.rank === sq.rank)) {
    classes.push('movable')
  }
  if (props.attackSquares.some(s => s.file === sq.file && s.rank === sq.rank)) {
    classes.push('attack')
  }
  if (props.dropSquares.some(s => s.file === sq.file && s.rank === sq.rank)) {
    classes.push('droppable')
  }
  return classes
}

function onSquareClick(sq: SquareInfo) {
  emit('squareClick', { file: sq.file, rank: sq.rank })
}

const PIECE_SYMBOLS: Record<string, string> = {
  'king': '♔',
  'queen': '♕',
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
  border: 2px solid #555;
  width: min(80vw, 80vh);
  height: min(80vw, 80vh);
}

.square {
  position: relative;
  display: flex;
  justify-content: center;
  align-items: center;
  cursor: pointer;
  user-select: none;
}

.square.light { background: #f0d9b5; }
.square.dark  { background: #b58863; }

.square.selected { background: #f6f669 !important; }
.square.movable  { background: rgba(0, 200, 80, 0.4) !important; }
.square.attack   { background: rgba(220, 50, 50, 0.35) !important; }
.square.droppable { background: rgba(80, 80, 255, 0.35) !important; }

.piece {
  font-size: clamp(16px, 4vw, 48px);
  line-height: 1;
  pointer-events: none;
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
