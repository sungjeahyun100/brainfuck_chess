<template>
  <main class="lobby piece-lab-view">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">돌아가기</button>
      <div>
        <p class="eyebrow">Piece Lab</p>
        <h1>기물 테스트장 <span class="hero-en">Piece Lab</span></h1>
      </div>
      <button class="btn-secondary danger" @click="resetLab">전체 초기화</button>
    </div>

    <div class="piece-lab-grid">
      <section class="card lab-panel lab-controls">
        <div class="section-header">
          <p class="section-kicker">Controls</p>
          <h2>테스트 설정</h2>
        </div>

        <label>
          <span class="limit-label">보드 크기</span>
          <select v-model.number="boardSize" class="text-input" @change="resetLabPieces">
            <option v-for="size in boardSizes" :key="size" :value="size">{{ size }} x {{ size }}</option>
          </select>
        </label>

        <div class="lab-side-toggle">
          <span class="limit-label">배치 진영</span>
          <div class="segmented">
            <button :class="{ active: selectedOwner === 'white' }" @click="selectedOwner = 'white'">White</button>
            <button :class="{ active: selectedOwner === 'black' }" @click="selectedOwner = 'black'">Black</button>
          </div>
        </div>

        <div class="placement-controls">
          <button class="tool-button" :class="{ active: selectedTool === eraseTool }" @click="selectedTool = eraseTool">
            <span>x</span>
            <strong>지우개</strong>
          </button>
          <button class="tool-button" @click="clearSelection">
            <span>-</span>
            <strong>선택 해제</strong>
          </button>
        </div>

        <input v-model.trim="pieceSearch" class="piece-search" type="search" placeholder="기물 검색" />
        <div class="piece-catalog lab-catalog">
          <button
            v-for="piece in filteredPieceCatalog"
            :key="piece.id"
            class="palette-piece"
            :class="{ active: selectedTool === piece.id }"
            draggable="true"
            @click="selectCatalogPiece(piece.id)"
            @dragstart="onCatalogDragStart($event, piece.id)"
            @dragend="clearDragState"
          >
            <span class="symbol">
              <img
                v-if="displayPieceAsset(piece.id, selectedOwner)"
                class="piece-icon"
                :src="displayPieceAsset(piece.id, selectedOwner)"
                :alt="piece.name"
                draggable="false"
              />
              <span v-else>{{ displayPieceSymbol(piece.id) }}</span>
            </span>
            <span class="meta">
              <strong>{{ piece.name }}</strong>
              <small>{{ piece.score === 0 ? '점수 제외' : `${piece.score}점` }}</small>
            </span>
          </button>
        </div>
      </section>

      <section class="lab-board-wrap">
        <div class="lab-status">
          <span>선택 도구: <strong>{{ selectedToolLabel }}</strong></span>
          <span>배치 기물: <strong>{{ pieces.length }}</strong></span>
          <span v-if="optionsLoading">행마 계산 중...</span>
          <span v-else-if="optionsError" class="error">{{ optionsError }}</span>
        </div>

        <div class="lab-board" :style="{ '--lab-board-size': boardSize }">
          <button
            v-for="square in boardSquares"
            :key="square.id"
            class="lab-square"
            :class="squareClasses(square)"
            @click="onSquareClick(square.file, square.rank)"
            @dragover.prevent="onSquareDragOver($event, square.file, square.rank)"
            @drop.prevent="onSquareDrop($event, square.file, square.rank)"
          >
            <span class="square-label">{{ fileLabel(square.file) }}{{ square.rank + 1 }}</span>
            <span v-if="isMoveSquare(square)" class="lab-marker move" />
            <span v-if="isAttackSquare(square)" class="lab-marker attack" />
            <span v-if="isAbilitySquare(square)" class="lab-marker ability">*</span>
            <span
              v-if="pieceAt(square.file, square.rank)"
              class="square-piece lab-piece"
              :class="{ dragging: draggedPieceId === pieceAt(square.file, square.rank)!.id }"
              draggable="true"
              @dragstart.stop="onPlacedPieceDragStart($event, pieceAt(square.file, square.rank)!.id)"
              @dragend="clearDragState"
            >
              <img
                v-if="displayPieceAsset(pieceAt(square.file, square.rank)!.pieceType, pieceAt(square.file, square.rank)!.owner)"
                class="piece-icon"
                :src="displayPieceAsset(pieceAt(square.file, square.rank)!.pieceType, pieceAt(square.file, square.rank)!.owner)"
                :alt="pieceLabel(pieceAt(square.file, square.rank)!.pieceType)"
                draggable="false"
              />
              <span v-else>{{ displayPieceSymbol(pieceAt(square.file, square.rank)!.pieceType) }}</span>
            </span>
          </button>
        </div>
      </section>

      <section class="card lab-panel lab-inspector">
        <div class="section-header">
          <p class="section-kicker">Inspector</p>
          <h2>기물 정보</h2>
        </div>

        <template v-if="inspectedCatalogItem">
          <div class="inspector-heading">
            <span class="symbol large">
              <img
                v-if="displayPieceAsset(inspectedCatalogItem.id, inspectedOwner)"
                class="piece-icon"
                :src="displayPieceAsset(inspectedCatalogItem.id, inspectedOwner)"
                :alt="inspectedCatalogItem.name"
                draggable="false"
              />
              <span v-else>{{ displayPieceSymbol(inspectedCatalogItem.id) }}</span>
            </span>
            <div>
              <h2>{{ inspectedCatalogItem.name }}</h2>
              <p>{{ inspectedOwner === 'white' ? 'White' : 'Black' }} · {{ inspectedCatalogItem.score }}점</p>
            </div>
          </div>

          <dl class="info-list">
            <div>
              <dt>포켓 사용</dt>
              <dd>{{ inspectedCatalogItem.canPocket ? '가능' : '불가' }}</dd>
            </div>
            <div>
              <dt>행마 설명</dt>
              <dd>{{ movementDescription(inspectedCatalogItem.id) }}</dd>
            </div>
            <div>
              <dt>특수능력</dt>
              <dd>{{ abilitySummary }}</dd>
            </div>
          </dl>

          <div class="ability-panel">
            <div class="section-header">
              <p class="section-kicker">Abilities</p>
              <h3>특수능력 테스트</h3>
            </div>
            <template v-if="displayAbilities.length > 0">
              <div v-for="ability in displayAbilities" :key="ability.id" class="ability-card">
                <strong>{{ ability.name }}</strong>
                <p>{{ ability.description }}</p>
                <small>{{ ability.available ? '현재 상태에서 사용 가능' : '현재 상태에서 사용 불가' }}</small>
                <button class="btn-start" type="button" disabled>실행</button>
                <small v-if="!ability.connected">이 기물의 특수능력 테스트는 아직 연결되지 않았습니다.</small>
              </div>
            </template>
            <p v-else class="muted-note">이 기물에는 등록된 특수능력이 없습니다.</p>
          </div>
        </template>

        <div v-else class="empty-state">
          <h2>기물을 선택하세요</h2>
          <p>기물 목록에서 선택한 뒤 보드에 배치하거나, 배치된 기물을 클릭하면 행마가 표시됩니다.</p>
        </div>
      </section>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { api, type PieceLabAbilityOption } from '../api/gameApi'
import { pieceAsset } from '../pieceAssets'
import type { DeckPieceType } from '../types/deck'
import type { PlayerId, Square } from '../types/game'
import { boardSizes, pieceCatalog, pieceLabel } from '../composables/useDeckValidation'

interface PieceLabPiece {
  id: string
  pieceType: string
  owner: PlayerId
  square: Square
}

interface LabSquare extends Square {
  id: string
}

const props = defineProps<{
  initialPieceType?: string | null
  initialBoardSize?: number | null
}>()

defineEmits<{
  back: []
}>()

const eraseTool = '__erase__'
const boardSize = ref<number>(props.initialBoardSize ?? 8)
const selectedOwner = ref<PlayerId>('white')
const selectedTool = ref<string | null>(props.initialPieceType ?? 'king')
const selectedPieceId = ref<string | null>(null)
const pieces = ref<PieceLabPiece[]>([])
const pieceSearch = ref('')
const moves = ref<Square[]>([])
const attacks = ref<Square[]>([])
const abilitySquares = ref<Square[]>([])
const abilities = ref<PieceLabAbilityOption[]>([])
const optionsLoading = ref(false)
const optionsError = ref<string | null>(null)
const draggedCatalogPiece = ref<string | null>(null)
const draggedPieceId = ref<string | null>(null)
let nextPieceSerial = 1
let optionsSerial = 0

const filteredPieceCatalog = computed(() => {
  const query = pieceSearch.value.toLowerCase()
  if (!query) return pieceCatalog
  return pieceCatalog.filter(piece => [piece.id, piece.name, piece.category, ...(piece.aliases ?? [])].join(' ').toLowerCase().includes(query))
})

const selectedLabPiece = computed(() => pieces.value.find(piece => piece.id === selectedPieceId.value) ?? null)
const inspectedType = computed(() => selectedLabPiece.value?.pieceType ?? (selectedTool.value && selectedTool.value !== eraseTool ? selectedTool.value : null))
const inspectedOwner = computed(() => selectedLabPiece.value?.owner ?? selectedOwner.value)
const inspectedCatalogItem = computed(() => pieceCatalog.find(piece => piece.id === inspectedType.value) ?? null)
const selectedToolLabel = computed(() => {
  if (selectedTool.value === eraseTool) return '지우개'
  return selectedTool.value ? pieceLabel(selectedTool.value) : '없음'
})
const displayAbilities = computed<PieceLabAbilityOption[]>(() => {
  if (selectedLabPiece.value) return abilities.value
  return inspectedType.value ? staticAbilities(inspectedType.value) : []
})
const abilitySummary = computed(() => {
  if (displayAbilities.value.length > 0) return `${displayAbilities.value.length}개 등록`
  return '없음'
})

const boardSquares = computed(() => {
  const squares: LabSquare[] = []
  for (let rank = boardSize.value - 1; rank >= 0; rank--) {
    for (let file = 0; file < boardSize.value; file++) {
      squares.push({ id: squareId({ file, rank }), file, rank })
    }
  }
  return squares
})

watch(
  () => [props.initialPieceType, props.initialBoardSize] as const,
  ([pieceType, nextBoardSize]) => {
    if (nextBoardSize && boardSizes.includes(nextBoardSize as typeof boardSizes[number])) {
      boardSize.value = nextBoardSize
      resetLabPieces()
    }
    if (pieceType && pieceCatalog.some(piece => piece.id === pieceType)) {
      selectedTool.value = pieceType
    }
  },
  { immediate: true },
)

watch(
  () => [selectedPieceId.value, pieces.value.map(piece => `${piece.id}:${piece.pieceType}:${piece.owner}:${piece.square.file}_${piece.square.rank}`).join('|'), boardSize.value],
  () => {
    void loadSelectedPieceOptions()
  },
)

function squareId(square: Square): string {
  return `${square.file}_${square.rank}`
}

function fileLabel(file: number): string {
  return String.fromCharCode(97 + file)
}

function displayPieceAsset(pieceType: string, owner: PlayerId): string | undefined {
  return pieceAsset(pieceType, owner)
}

function displayPieceSymbol(pieceType: string): string {
  const symbols: Record<string, string> = {
    king: 'K',
    queen: 'Q',
    amazon: 'A',
    'tempest-queen': 'Q',
    'tempest-rook': 'T',
    'tempest-knight': 'N',
    'bouncing-bishop': 'B',
    rook: 'R',
    bishop: 'B',
    knight: 'N',
    pawn: 'P',
    'tempest-pawn': 'P',
  }
  return symbols[pieceType] ?? pieceLabel(pieceType).slice(0, 1).toUpperCase()
}

function movementDescription(pieceType: string): string {
  const descriptions: Record<string, string> = {
    king: '8방향으로 한 칸 이동하고 공격합니다. 실제 게임에서는 캐슬링도 지원됩니다.',
    queen: '가로, 세로, 대각선으로 막히기 전까지 이동하고 공격합니다.',
    rook: '가로와 세로로 막히기 전까지 이동하고 공격합니다.',
    bishop: '대각선으로 막히기 전까지 이동하고 공격합니다. 특수능력으로 Bouncing Bishop 행마를 일시 적용할 수 있습니다.',
    knight: 'L자 형태로 도약하며 중간 기물에 막히지 않습니다.',
    pawn: 'White는 위로, Black은 아래로 전진합니다. 대각선 전방을 공격하고 시작 위치에서는 2칸 전진할 수 있습니다.',
    amazon: 'Queen의 장거리 행마와 Knight의 도약 행마를 모두 사용합니다.',
    'tempest-rook': '대각선으로 한 칸 진입한 뒤 그 지점에서 가로 또는 세로 방향으로 뻗어 나갑니다.',
    'bouncing-bishop': '대각선으로 이동하며 보드 가장자리에서 반사되는 경로를 가집니다.',
    'tempest-pawn': 'Pawn과 같은 기본 행마를 사용하되, 승격 후보가 Tempest 계열 기물입니다.',
    'tempest-queen': '대각선 진입 후 가로, 세로, 대각선으로 폭풍처럼 뻗어 나갑니다.',
    'tempest-knight': '대각선 진입 후 확장된 Knight 계열 도약과 3칸 직선 도약을 사용합니다.',
  }
  return descriptions[pieceType] ?? 'Custom Piece: 등록된 Chessembly 행마법을 따릅니다.'
}

function staticAbilities(pieceType: string): PieceLabAbilityOption[] {
  if (pieceType !== 'bishop') return []
  return [{
    id: 'bounce_mode',
    name: 'Reflective Movement',
    description: 'Moves like a Bouncing Bishop until this turn ends.',
    available: false,
    connected: false,
  }]
}

function resetLab() {
  selectedOwner.value = 'white'
  selectedTool.value = props.initialPieceType ?? 'king'
  resetLabPieces()
}

function resetLabPieces() {
  pieces.value = []
  selectedPieceId.value = null
  moves.value = []
  attacks.value = []
  abilitySquares.value = []
  abilities.value = []
  optionsError.value = null
}

function clearSelection() {
  selectedPieceId.value = null
  moves.value = []
  attacks.value = []
  abilitySquares.value = []
  abilities.value = []
  optionsError.value = null
}

function selectCatalogPiece(pieceType: DeckPieceType) {
  selectedTool.value = pieceType
  clearSelection()
}

function pieceAt(file: number, rank: number): PieceLabPiece | null {
  return pieces.value.find(piece => piece.square.file === file && piece.square.rank === rank) ?? null
}

function onSquareClick(file: number, rank: number) {
  const existing = pieceAt(file, rank)
  if (selectedTool.value === eraseTool) {
    if (!existing) return
    pieces.value = pieces.value.filter(piece => piece.id !== existing.id)
    if (selectedPieceId.value === existing.id) clearSelection()
    return
  }

  if (existing) {
    selectedPieceId.value = existing.id
    return
  }

  if (!selectedTool.value) return
  const piece: PieceLabPiece = {
    id: `lab_${selectedOwner.value}_${selectedTool.value.replace(/[^a-z0-9]+/gi, '_')}_${nextPieceSerial++}`,
    pieceType: selectedTool.value,
    owner: selectedOwner.value,
    square: { file, rank },
  }
  pieces.value = [...pieces.value, piece]
  selectedPieceId.value = piece.id
}

function onCatalogDragStart(event: DragEvent, pieceType: DeckPieceType) {
  draggedCatalogPiece.value = pieceType
  draggedPieceId.value = null
  selectedTool.value = pieceType
  event.dataTransfer?.setData('application/x-piece-lab-catalog-piece', pieceType)
  event.dataTransfer?.setData('text/plain', pieceType)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'copy'
  }
}

function onPlacedPieceDragStart(event: DragEvent, pieceId: string) {
  draggedPieceId.value = pieceId
  draggedCatalogPiece.value = null
  selectedPieceId.value = pieceId
  event.dataTransfer?.setData('application/x-piece-lab-board-piece', pieceId)
  event.dataTransfer?.setData('text/plain', pieceId)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
}

function clearDragState() {
  draggedCatalogPiece.value = null
  draggedPieceId.value = null
}

function onSquareDragOver(event: DragEvent, file: number, rank: number) {
  if (!event.dataTransfer) return
  const existing = pieceAt(file, rank)
  if (draggedCatalogPiece.value || Array.from(event.dataTransfer.types).includes('application/x-piece-lab-catalog-piece')) {
    event.dataTransfer.dropEffect = selectedTool.value === eraseTool ? 'none' : 'copy'
    return
  }
  const boardPieceId = draggedPieceId.value || event.dataTransfer.getData('application/x-piece-lab-board-piece')
  event.dataTransfer.dropEffect = boardPieceId && (!existing || existing.id === boardPieceId) ? 'move' : 'none'
}

function onSquareDrop(event: DragEvent, file: number, rank: number) {
  const boardPieceId = draggedPieceId.value || event.dataTransfer?.getData('application/x-piece-lab-board-piece') || null
  const catalogPiece = draggedCatalogPiece.value || event.dataTransfer?.getData('application/x-piece-lab-catalog-piece') || null
  clearDragState()

  const existing = pieceAt(file, rank)
  if (boardPieceId) {
    if (existing && existing.id !== boardPieceId) {
      selectedPieceId.value = existing.id
      return
    }
    pieces.value = pieces.value.map(piece => (
      piece.id === boardPieceId
        ? { ...piece, square: { file, rank } }
        : piece
    ))
    selectedPieceId.value = boardPieceId
    return
  }

  if (catalogPiece && pieceCatalog.some(piece => piece.id === catalogPiece)) {
    selectedTool.value = catalogPiece
    if (existing) {
      selectedPieceId.value = existing.id
      return
    }
    pieces.value = [
      ...pieces.value,
      {
        id: `lab_${selectedOwner.value}_${catalogPiece.replace(/[^a-z0-9]+/gi, '_')}_${nextPieceSerial++}`,
        pieceType: catalogPiece,
        owner: selectedOwner.value,
        square: { file, rank },
      },
    ]
    selectedPieceId.value = pieces.value[pieces.value.length - 1]?.id ?? null
  }
}

function squareClasses(square: LabSquare): string[] {
  const piece = pieceAt(square.file, square.rank)
  return [
    (square.file + square.rank) % 2 === 1 ? 'light' : 'dark',
    piece ? 'occupied' : 'empty',
    piece?.id === selectedPieceId.value ? 'selected' : '',
    isMoveSquare(square) ? 'can-move' : '',
    isAttackSquare(square) ? 'can-attack' : '',
    isAbilitySquare(square) ? 'can-ability' : '',
  ].filter(Boolean)
}

function isMoveSquare(square: Square): boolean {
  const id = squareId(square)
  return moves.value.some(move => squareId(move) === id)
}

function isAttackSquare(square: Square): boolean {
  const id = squareId(square)
  return attacks.value.some(attack => squareId(attack) === id)
}

function isAbilitySquare(square: Square): boolean {
  const id = squareId(square)
  return abilitySquares.value.some(abilitySquare => squareId(abilitySquare) === id)
}

async function loadSelectedPieceOptions() {
  const selected = selectedLabPiece.value
  const serial = ++optionsSerial
  if (!selected) {
    optionsLoading.value = false
    optionsError.value = null
    return
  }

  optionsLoading.value = true
  optionsError.value = null
  try {
    const response = await api.getPieceLabOptions({
      board_size: boardSize.value,
      pieces: pieces.value.map(piece => ({
        id: piece.id,
        piece_type: piece.pieceType,
        owner: piece.owner,
        square: piece.square,
      })),
      selected_piece_id: selected.id,
    })
    if (serial !== optionsSerial) return
    moves.value = response.moves
    attacks.value = response.attacks
    abilitySquares.value = []
    abilities.value = response.abilities
  } catch (e: unknown) {
    if (serial !== optionsSerial) return
    moves.value = []
    attacks.value = []
    abilitySquares.value = []
    abilities.value = []
    optionsError.value = e instanceof Error ? e.message : String(e)
  } finally {
    if (serial === optionsSerial) optionsLoading.value = false
  }
}

</script>

<style scoped>
.piece-lab-view {
  width: min(1500px, 100%);
}

.piece-lab-grid {
  display: grid;
  grid-template-columns: minmax(260px, 0.85fr) minmax(480px, 1.5fr) minmax(300px, 0.9fr);
  gap: 16px;
  align-items: start;
}

.lab-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 18px;
}

.lab-controls label,
.lab-side-toggle {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.segmented {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: 8px;
}

.segmented button {
  border: none;
  cursor: pointer;
}

.segmented button {
  min-height: 42px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
}

.segmented button.active {
  background: rgba(217, 164, 65, 0.18);
  color: #f4dfb0;
  font-weight: 800;
}

.lab-catalog {
  max-height: 580px;
}

.lab-board-wrap {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}

.lab-status {
  min-height: 42px;
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--muted);
}

.lab-status strong {
  color: #f4dfb0;
}

.lab-board {
  display: grid;
  grid-template-columns: repeat(var(--lab-board-size), 1fr);
  grid-template-rows: repeat(var(--lab-board-size), minmax(0, 1fr));
  width: min(100%, 78vh);
  aspect-ratio: 1;
  margin: 0 auto;
  border: 2px solid rgba(244, 223, 176, 0.28);
  border-radius: 8px;
  overflow: hidden;
}

.lab-square {
  position: relative;
  min-width: 0;
  min-height: 0;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  overflow: hidden;
  padding: 0;
}

.lab-square.light { background: #f1dfbf; color: #232a38; }
.lab-square.dark { background: #b7844d; color: #fff8ef; }

.lab-square.selected::before {
  content: '';
  position: absolute;
  inset: 4px;
  z-index: 4;
  border: 2px solid rgba(246, 246, 105, 0.9);
  border-radius: 5px;
  pointer-events: none;
}

.lab-square.can-attack::after {
  content: '';
  position: absolute;
  inset: 6px;
  z-index: 1;
  border: 2px solid rgba(214, 55, 55, 0.88);
  border-radius: 999px;
  pointer-events: none;
}

.lab-marker {
  position: absolute;
  z-index: 2;
  pointer-events: none;
}

.lab-marker.move {
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: rgba(46, 120, 230, 0.82);
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.18);
}

.lab-marker.attack {
  inset: 8px;
  border: 2px solid rgba(214, 55, 55, 0.88);
  border-radius: 6px;
}

.lab-marker.ability {
  right: 7px;
  bottom: 5px;
  color: #c77dff;
  font-size: 18px;
  font-weight: 900;
}

.lab-piece {
  position: absolute;
  inset: 12%;
  z-index: 3;
  width: auto;
  height: auto;
  max-width: none;
  max-height: none;
  cursor: grab;
}

.lab-piece.dragging {
  opacity: 0.45;
}

.lab-piece:active {
  cursor: grabbing;
}

.inspector-heading {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 12px;
  align-items: center;
}

.symbol.large {
  width: 54px;
  height: 54px;
}

.inspector-heading p,
.info-list dd,
.ability-card p,
.ability-card small {
  color: var(--muted);
}

.info-list,
.ability-panel,
.ability-card {
  display: grid;
  gap: 12px;
}

.info-list > div {
  display: grid;
  gap: 4px;
}

.info-list dt {
  color: var(--accent);
  font-size: 12px;
  font-weight: 800;
  text-transform: uppercase;
}

.ability-card {
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
}

.ability-card .btn-start {
  width: max-content;
}

@media (max-width: 1200px) {
  .piece-lab-grid {
    grid-template-columns: 1fr;
  }

  .lab-board {
    width: min(100%, 86vh);
  }
}
</style>
