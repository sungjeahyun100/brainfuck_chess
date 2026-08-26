<template>
  <div class="board-wrapper">
    <div
      ref="boardElement"
      class="board"
      :style="{ '--size': board.size }"
      @contextmenu.prevent
      @pointerdown="onBoardPointerDown"
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
        <span
          v-if="sq.terrain"
          class="terrain-layer"
          :class="`terrain-${sq.terrain.type_id}`"
          :title="terrainLabel(sq.terrain.type_id)"
          aria-hidden="true"
        />
        <span v-if="showCoordinates && isFileLabelSquare(sq)" class="board-coordinate file-coordinate">
          {{ fileLabel(sq.file) }}
        </span>
        <span v-if="showCoordinates && isRankLabelSquare(sq)" class="board-coordinate rank-coordinate">
          {{ sq.rank + 1 }}
        </span>
        <span v-if="legalMarker(sq)" class="legal-move-dot" :class="legalMarker(sq)" />
        <span v-if="sq.piece" class="piece" :class="`owner-${sq.piece.owner}`">
          <img
            v-if="pieceImage(sq.piece)"
            :key="pieceRenderKey(sq.piece)"
            class="piece-image"
            :src="pieceImage(sq.piece)"
            :alt="pieceAlt(sq.piece)"
            draggable="false"
          />
          <span v-else>{{ pieceSymbol(sq.piece.type_id) }}</span>
          <span
            v-if="activeCooldownRemaining(sq.piece.move_option_cooldowns) > 0"
            class="piece-cooldown-badge"
            :title="`쿨타임 ${activeCooldownRemaining(sq.piece.move_option_cooldowns)}턴`"
          >
            {{ activeCooldownRemaining(sq.piece.move_option_cooldowns) }}
          </span>
          <span
            v-if="(definitions[sq.piece.type_id]?.max_ammo ?? 0) > 0"
            class="piece-ammo-badge"
            :title="`탄약 ${sq.piece.current_ammo}/${definitions[sq.piece.type_id].max_ammo}`"
          >
            {{ sq.piece.current_ammo ?? 0 }}
          </span>
        </span>
        <span
          v-if="sq.airPiece"
          class="piece air-piece"
          :class="`owner-${sq.airPiece.owner}`"
          :title="`공중 · 남은 비행 ${sq.airPiece.remaining_flight_turns}턴`"
          @click.stop="onPieceClick(sq.airPiece.id)"
        >
          <img
            v-if="pieceImage(sq.airPiece)"
            :key="pieceRenderKey(sq.airPiece)"
            class="piece-image"
            :src="pieceImage(sq.airPiece)"
            :alt="pieceAlt(sq.airPiece)"
            draggable="false"
          />
          <span v-else>{{ pieceSymbol(sq.airPiece.type_id) }}</span>
          <span class="piece-flight-badge" :title="`남은 비행 ${sq.airPiece.remaining_flight_turns}턴`">
            ✈ {{ sq.airPiece.remaining_flight_turns }}
          </span>
          <span
            v-if="(definitions[sq.airPiece.type_id]?.max_ammo ?? 0) > 0"
            class="piece-ammo-badge"
            :title="`탄약 ${sq.airPiece.current_ammo}/${definitions[sq.airPiece.type_id].max_ammo}`"
          >
            {{ sq.airPiece.current_ammo ?? 0 }}
          </span>
          <span
            v-if="activeCooldownRemaining(sq.airPiece.move_option_cooldowns) > 0"
            class="piece-cooldown-badge"
          >
            {{ activeCooldownRemaining(sq.airPiece.move_option_cooldowns) }}
          </span>
        </span>
      </div>
      <svg
        v-if="renderedArrows.length || highlightedSquares.length"
        class="board-arrow-overlay"
        :viewBox="arrowViewBox"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <defs>
          <marker
            :id="arrowMarkerId"
            markerWidth="4"
            markerHeight="4"
            refX="3.4"
            refY="2"
            orient="auto"
            markerUnits="strokeWidth"
          >
            <path d="M 0 0 L 4 2 L 0 4 z" class="board-arrow-head" />
          </marker>
        </defs>
        <circle
          v-for="highlight in renderedHighlights"
          :key="highlight.key"
          class="board-square-highlight"
          :cx="highlight.x"
          :cy="highlight.y"
          r="0.36"
        />
        <line
          v-for="arrow in renderedArrows"
          :key="arrow.key"
          class="board-arrow"
          :class="{ preview: arrow.preview }"
          :x1="arrow.x1"
          :y1="arrow.y1"
          :x2="arrow.x2"
          :y2="arrow.y2"
          :marker-end="`url(#${arrowMarkerId})`"
        />
      </svg>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { Board, Piece, PieceDefinition, PlayerId, Square, TerrainCell } from '../types/game'
import { activeCooldownRemaining } from '../moveOptionUi'
import { renderedPieceAsset, resolvePieceAssetKey } from '../pieceAssets'

interface SquareInfo {
  id: string
  file: number
  rank: number
  piece?: Piece
  airPiece?: Piece
  terrain?: TerrainCell
  isLight: boolean
}

interface BoardArrow {
  from: string
  to: string
}

interface RenderedArrow extends BoardArrow {
  key: string
  x1: number
  y1: number
  x2: number
  y2: number
  preview: boolean
}

const props = defineProps<{
  board: Board
  pieces: Record<string, Piece>
  definitions: Record<string, PieceDefinition>
  selectedPieceId: string | null
  movableSquares: Square[]
  attackSquares: Square[]
  threatSquares?: Square[]
  dropSquares: Square[]
  lastMove?: { from: Square; to: Square } | null
  orientation?: PlayerId
  abilityMode?: boolean
  showCoordinates?: boolean
}>()

function pieceImage(piece: Piece): string | undefined {
  return renderedPieceAsset(piece, props.definitions[piece.type_id])
}

function pieceRenderKey(piece: Piece): string {
  return `${piece.id}:${resolvePieceAssetKey(piece, props.definitions[piece.type_id])}`
}

const emit = defineEmits<{
  squareClick: [square: Square]
  pieceDragStart: [pieceId: string]
  squareDrop: [square: Square | null, pieceId: string]
  pieceClick: [pieceId: string]
}>()

function squareId(file: number, rank: number) {
  return `${file}_${rank}`
}

const allSquares = computed((): SquareInfo[] => {
  const squares: SquareInfo[] = []
  const isBlackOrientation = props.orientation === 'black'
  const ranks = isBlackOrientation
    ? Array.from({ length: props.board.size }, (_, index) => index)
    : Array.from({ length: props.board.size }, (_, index) => props.board.size - 1 - index)
  const files = isBlackOrientation
    ? Array.from({ length: props.board.size }, (_, index) => props.board.size - 1 - index)
    : Array.from({ length: props.board.size }, (_, index) => index)

  for (const rank of ranks) {
    for (const file of files) {
      const id = squareId(file, rank)
      const pieceId = props.board.squares[id] ?? null
      const piece = pieceId ? props.pieces[pieceId] : undefined
      const airPieceId = props.board.air_squares?.[id] ?? null
      const airPiece = airPieceId ? props.pieces[airPieceId] : undefined
      const terrain = props.board.terrain?.[id]
      squares.push({
        id,
        file,
        rank,
        piece,
        airPiece,
        terrain,
        isLight: (file + rank) % 2 === 1,
      })
    }
  }
  return squares
})

const movableSquareIds = computed(() => new Set(props.movableSquares.map(squareIdFromSquare)))
const attackSquareIds = computed(() => new Set(props.attackSquares.map(squareIdFromSquare)))
const threatSquareIds = computed(() => new Set((props.threatSquares ?? []).map(squareIdFromSquare)))
const dropSquareIds = computed(() => new Set(props.dropSquares.map(squareIdFromSquare)))
const lastMoveSquareIds = computed(() => {
  if (!props.lastMove) return new Set<string>()
  return new Set([squareIdFromSquare(props.lastMove.from), squareIdFromSquare(props.lastMove.to)])
})
const boardElement = ref<HTMLElement | null>(null)
const arrows = ref<BoardArrow[]>([])
const highlightedSquares = ref<string[]>([])
const rightDrag = ref<{
  pointerId: number
  from: string
  previewTo: string | null
} | null>(null)
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
const arrowMarkerId = `board-arrow-head-${Math.random().toString(36).slice(2)}`
const arrowViewBox = computed(() => `0 0 ${props.board.size} ${props.board.size}`)
const renderedArrows = computed(() => {
  const rendered = arrows.value
    .map((arrow, index) => renderArrow(arrow, `arrow-${index}-${arrow.from}-${arrow.to}`, false))
    .filter((arrow): arrow is RenderedArrow => Boolean(arrow))

  const drag = rightDrag.value
  if (drag?.previewTo && drag.previewTo !== drag.from) {
    const preview = renderArrow(
      { from: drag.from, to: drag.previewTo },
      `preview-${drag.from}-${drag.previewTo}`,
      true,
    )
    if (preview) rendered.push(preview)
  }

  return rendered
})
const renderedHighlights = computed(() => highlightedSquares.value.flatMap((squareId) => {
  const center = squareCenterFromId(squareId)
  return center ? [{ key: `highlight-${squareId}`, ...center }] : []
}))

function squareIdFromSquare(square: Square) {
  return squareId(square.file, square.rank)
}

function isFileLabelSquare(square: SquareInfo) {
  return props.orientation === 'black'
    ? square.rank === props.board.size - 1
    : square.rank === 0
}

function isRankLabelSquare(square: SquareInfo) {
  return props.orientation === 'black'
    ? square.file === props.board.size - 1
    : square.file === 0
}

function fileLabel(file: number) {
  return String.fromCharCode('a'.charCodeAt(0) + file)
}

function terrainLabel(typeId: string) {
  return typeId === 'high-ground' ? '고지' : typeId
}

function squareClasses(sq: SquareInfo) {
  const classes: string[] = [sq.isLight ? 'light' : 'dark']

  if (lastMoveSquareIds.value.has(sq.id)) {
    classes.push('last-move')
  }
  if (threatSquareIds.value.has(sq.id)) {
    classes.push('opponent-threat')
  }
  if ((sq.piece && sq.piece.id === props.selectedPieceId)
    || (sq.airPiece && sq.airPiece.id === props.selectedPieceId)) {
    classes.push('selected')
    if (props.abilityMode) classes.push('ability-selected')
  }
  if (sq.piece?.id === draggingPieceId.value) {
    classes.push('dragging')
  }
  if (dragOverSquareId.value === sq.id) {
    classes.push('drag-over')
  }
  return classes
}

function onPieceClick(pieceId: string) {
  emit('pieceClick', pieceId)
}

function legalMarker(sq: SquareInfo): string | null {
  if (attackSquareIds.value.has(sq.id)) return props.abilityMode ? 'ability capture' : 'capture'
  if (movableSquareIds.value.has(sq.id)) return props.abilityMode ? 'ability move' : 'move'
  if (dropSquareIds.value.has(sq.id)) return sq.piece ? 'drop capture' : 'drop'
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

function onBoardPointerDown(event: PointerEvent) {
  if (event.button === 0) {
    const squareId = squareIdFromClientPoint(event.clientX, event.clientY)
    if (squareId && !props.board.squares[squareId]) {
      clearAnnotations()
    }
    return
  }

  if (event.button !== 2) return

  const from = squareIdFromClientPoint(event.clientX, event.clientY)
  if (!from) return

  event.preventDefault()
  rightDrag.value = {
    pointerId: event.pointerId,
    from,
    previewTo: null,
  }
  window.addEventListener('pointermove', onWindowRightPointerMove)
  window.addEventListener('pointerup', onWindowRightPointerUp)
  window.addEventListener('pointercancel', onWindowRightPointerCancel)
  window.addEventListener('contextmenu', preventRightDragContextMenu)
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

function onWindowRightPointerMove(event: PointerEvent) {
  const drag = rightDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  event.preventDefault()
  drag.previewTo = squareIdFromClientPoint(event.clientX, event.clientY)
}

function onWindowRightPointerUp(event: PointerEvent) {
  const drag = rightDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  event.preventDefault()
  const to = squareIdFromClientPoint(event.clientX, event.clientY)
  cleanupRightDrag()

  if (!to) return

  if (to === drag.from) {
    toggleHighlight(to)
    return
  }

  toggleArrow({ from: drag.from, to })
}

function onWindowRightPointerCancel(event: PointerEvent) {
  const drag = rightDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  event.preventDefault()
  cleanupRightDrag()
}

function cleanupPointerDrag() {
  pointerDrag.value = null
  draggingPieceId.value = null
  dragOverSquareId.value = null
  window.removeEventListener('pointermove', onWindowPointerMove)
  window.removeEventListener('pointerup', onWindowPointerUp)
  window.removeEventListener('pointercancel', onWindowPointerCancel)
}

function cleanupRightDrag() {
  rightDrag.value = null
  window.removeEventListener('pointermove', onWindowRightPointerMove)
  window.removeEventListener('pointerup', onWindowRightPointerUp)
  window.removeEventListener('pointercancel', onWindowRightPointerCancel)
  window.removeEventListener('contextmenu', preventRightDragContextMenu)
}

function onDocumentPointerDown(event: PointerEvent) {
  if (event.button !== 0 || !boardElement.value) return
  if (boardElement.value.contains(event.target as Node | null)) return

  clearAnnotations()
}

function clearAnnotations() {
  arrows.value = []
  highlightedSquares.value = []
}

function preventRightDragContextMenu(event: MouseEvent) {
  if (!rightDrag.value) return

  event.preventDefault()
}

function toggleArrow(nextArrow: BoardArrow) {
  const existingIndex = arrows.value.findIndex(
    arrow => arrow.from === nextArrow.from && arrow.to === nextArrow.to,
  )

  if (existingIndex >= 0) {
    arrows.value.splice(existingIndex, 1)
    return
  }

  arrows.value.push(nextArrow)
}

function toggleHighlight(squareId: string) {
  const existingIndex = highlightedSquares.value.indexOf(squareId)
  if (existingIndex >= 0) {
    highlightedSquares.value.splice(existingIndex, 1)
    return
  }

  highlightedSquares.value.push(squareId)
}

function squareIdFromPoint(clientX: number, clientY: number): string | null {
  const element = document.elementFromPoint(clientX, clientY)
  const square = element?.closest<HTMLElement>('.square')
  if (!square) return null

  const file = Number(square.dataset.file)
  const rank = Number(square.dataset.rank)
  return Number.isFinite(file) && Number.isFinite(rank) ? squareId(file, rank) : null
}

function squareIdFromClientPoint(clientX: number, clientY: number): string | null {
  const board = boardElement.value
  if (!board) return null

  const rect = board.getBoundingClientRect()
  const boardLeft = rect.left + board.clientLeft
  const boardTop = rect.top + board.clientTop
  const boardWidth = board.clientWidth
  const boardHeight = board.clientHeight
  const x = clientX - boardLeft
  const y = clientY - boardTop
  if (x < 0 || y < 0 || x >= boardWidth || y >= boardHeight) return null

  const displayFile = Math.floor((x / boardWidth) * props.board.size)
  const displayRank = Math.floor((y / boardHeight) * props.board.size)
  const file = props.orientation === 'black'
    ? props.board.size - 1 - displayFile
    : displayFile
  const rank = props.orientation === 'black'
    ? displayRank
    : props.board.size - 1 - displayRank

  return squareId(file, rank)
}

function squareFromId(id: string | null): Square | null {
  if (!id) return null

  const [file, rank] = id.split('_').map(Number)
  if (!Number.isFinite(file) || !Number.isFinite(rank)) return null

  return { file, rank }
}

function renderArrow(arrow: BoardArrow, key: string, preview: boolean): RenderedArrow | null {
  const from = squareCenterFromId(arrow.from)
  const to = squareCenterFromId(arrow.to)
  if (!from || !to) return null

  const dx = to.x - from.x
  const dy = to.y - from.y
  const length = Math.hypot(dx, dy)
  if (length === 0) return null

  const startPadding = 0.18
  const endPadding = 0.28
  return {
    ...arrow,
    key,
    x1: from.x + (dx / length) * startPadding,
    y1: from.y + (dy / length) * startPadding,
    x2: to.x - (dx / length) * endPadding,
    y2: to.y - (dy / length) * endPadding,
    preview,
  }
}

function squareCenterFromId(id: string): { x: number; y: number } | null {
  const square = squareFromId(id)
  if (!square) return null

  const displayFile = props.orientation === 'black'
    ? props.board.size - 1 - square.file
    : square.file
  const displayRank = props.orientation === 'black'
    ? square.rank
    : props.board.size - 1 - square.rank

  return {
    x: displayFile + 0.5,
    y: displayRank + 0.5,
  }
}

function onNativeDrop(event: DragEvent, sq: SquareInfo) {
  const pieceId = event.dataTransfer?.getData('application/x-brainfuck-chess-pocket-piece')
    || event.dataTransfer?.getData('text/plain')
    || null
  if (!pieceId) return

  emit('squareDrop', { file: sq.file, rank: sq.rank }, pieceId)
}

onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointerDown)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown)
  cleanupPointerDrag()
  cleanupRightDrag()
})

const PIECE_SYMBOLS: Record<string, string> = {
  'king': '♔',
  'queen': '♕',
  'amazon': 'A',
  'cannon-rook': 'C',
  'tempest-queen': 'Q',
  'tempest-rook': 'T',
  'tempest-bishop': 'B',
  'tempest-knight': 'N',
  'bouncing-bishop': 'B',
  'bouncing-rook': 'R',
  'bouncing-queen': 'Q',
  'bouncing-pawn-white': '♙',
  'bouncing-pawn-black': '♟',
  nightrider: 'N',
  guhang: 'G',
  'windmill': 'W',
  'rook': '♖',
  'bishop': '♗',
  'knight': '♘',
  'pawn-white': '♙',
  'pawn-black': '♟',
  'tempest-pawn-white': '♙',
  'tempest-pawn-black': '♟',
  'dozer-white': 'D',
  'dozer-black': 'D',
  'tank': '🛡',
  'bomber': '✈',
  'surface-to-air-missile-white': '▲',
  'surface-to-air-missile-black': '▲',
}

function pieceSymbol(typeId: string): string {
  return PIECE_SYMBOLS[typeId] ?? '?'
}

function pieceAlt(piece: Piece): string {
  return `${piece.owner} ${piece.type_id}`
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
  position: relative;
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
.square.last-move { background: #f2d34f; }

.terrain-layer {
  position: absolute;
  inset: 7%;
  z-index: 1;
  pointer-events: none;
  border-radius: 16%;
}

.terrain-high-ground {
  background:
    radial-gradient(circle at 34% 30%, rgba(246, 224, 151, 0.62) 0 7%, transparent 8%),
    radial-gradient(circle at 68% 62%, rgba(67, 54, 38, 0.36) 0 9%, transparent 10%),
    linear-gradient(145deg, rgba(196, 166, 101, 0.94), rgba(102, 83, 55, 0.94));
  border: 2px solid rgba(64, 49, 30, 0.72);
  box-shadow:
    inset 2px 2px 0 rgba(255, 239, 184, 0.35),
    inset -3px -3px 0 rgba(48, 35, 21, 0.28),
    0 2px 3px rgba(24, 17, 10, 0.35);
}

.square.opponent-threat::after {
  content: '';
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  background: rgba(198, 40, 40, 0.27);
  box-shadow: inset 0 0 0 2px rgba(145, 20, 20, 0.42);
}

.board-coordinate {
  position: absolute;
  z-index: 1;
  font-size: clamp(8px, 1.2vw, 13px);
  font-weight: 750;
  line-height: 1;
  pointer-events: none;
  opacity: 0.78;
}

.file-coordinate {
  right: 3px;
  bottom: 2px;
}

.rank-coordinate {
  left: 3px;
  top: 2px;
}

.square.light .board-coordinate { color: #8b6545; }
.square.dark .board-coordinate { color: #f0d9b5; }

.square.selected::before,
.square.drag-over::before {
  content: '';
  position: absolute;
  inset: 4px;
  border: 2px solid rgba(246, 246, 105, 0.82);
  border-radius: 4px;
  pointer-events: none;
}

.square.ability-selected::before {
  border-color: rgba(19, 184, 166, 0.95);
  box-shadow: 0 0 0 2px rgba(19, 184, 166, 0.22);
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

.legal-move-dot.ability {
  width: 18px;
  height: 18px;
  background: rgba(20, 184, 166, 0.84);
  box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.22), 0 0 0 6px rgba(20, 184, 166, 0.18);
}

.legal-move-dot.ability.capture {
  background: rgba(217, 119, 6, 0.9);
}

.legal-move-dot.drop {
  background: rgba(74, 143, 255, 0.78);
}

.legal-move-dot.drop.capture {
  background: rgba(220, 50, 50, 0.88);
  box-shadow: 0 0 0 3px rgba(74, 143, 255, 0.4);
}

.board-arrow-overlay {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 4;
}

.board-arrow {
  stroke: rgba(34, 139, 72, 0.86);
  stroke-width: 0.16;
  stroke-linecap: round;
  stroke-linejoin: round;
  fill: none;
}

.board-arrow.preview {
  opacity: 0.58;
}

.board-arrow-head {
  fill: rgba(34, 139, 72, 0.86);
}

.board-square-highlight {
  fill: rgba(34, 139, 72, 0.18);
  stroke: rgba(34, 139, 72, 0.9);
  stroke-width: 0.1;
}

.piece {
  font-size: clamp(16px, 4vw, 48px);
  line-height: 1;
  pointer-events: none;
  position: relative;
  z-index: 2;
  transition: opacity 80ms ease, transform 80ms ease;
  display: flex;
  width: 82%;
  height: 82%;
  align-items: center;
  justify-content: center;
}

.piece-image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.air-piece {
  position: absolute;
  z-index: 4;
  pointer-events: auto;
  transform: translate(10%, -10%) scale(0.88);
  filter: drop-shadow(0 8px 5px rgba(20, 50, 80, 0.48));
}

.piece-ammo-badge {
  position: absolute;
  left: -4%;
  bottom: -4%;
  z-index: 6;
  display: inline-flex;
  min-width: 1.4em;
  height: 1.4em;
  padding: 0 0.28em;
  align-items: center;
  justify-content: center;
  border: 2px solid rgba(255, 255, 255, 0.92);
  border-radius: 999px;
  background: #16834a;
  color: #fff;
  font-size: clamp(10px, 1.45vw, 16px);
  font-weight: 800;
  line-height: 1;
  pointer-events: none;
}

.piece-flight-badge {
  position: absolute;
  top: -10%;
  right: -8%;
  z-index: 6;
  padding: 0.12em 0.35em;
  border-radius: 999px;
  background: #246da8;
  color: white;
  font-size: clamp(9px, 1.2vw, 14px);
  font-weight: 800;
  pointer-events: none;
}

.piece-cooldown-badge {
  position: absolute;
  right: -4%;
  bottom: -4%;
  z-index: 5;
  display: inline-flex;
  min-width: 1.4em;
  height: 1.4em;
  padding: 0 0.28em;
  align-items: center;
  justify-content: center;
  border: 2px solid rgba(255, 255, 255, 0.92);
  border-radius: 999px;
  background: #b4232f;
  color: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
  font-size: clamp(10px, 1.45vw, 16px);
  font-weight: 800;
  line-height: 1;
  text-shadow: none;
}

.square.dragging .piece {
  opacity: 0.55;
  transform: scale(0.96);
}

.piece.owner-white { color: #fff; text-shadow: 0 0 2px #333; }
.piece.owner-black { color: #111; text-shadow: 0 0 2px #ccc; }
</style>
