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

    <div v-if="promotionRequest" class="lab-promotion-overlay">
      <div class="lab-promotion-box">
        <h2>프로모션 선택</h2>
        <p>{{ fileLabel(promotionRequest.to.file) }}{{ promotionRequest.to.rank + 1 }} 도착 후 변할 기물을 선택하세요.</p>
        <div class="promotion-choices">
          <button
            v-for="action in promotionRequest.actions"
            :key="action.promotion"
            type="button"
            class="promotion-choice"
            @click="choosePromotion(action)"
          >
            <img
              v-if="action.promotion && displayPieceAsset(action.promotion, promotionRequest.owner)"
              class="piece-icon"
              :src="displayPieceAsset(action.promotion, promotionRequest.owner)"
              :alt="action.promotion ? pieceLabel(action.promotion) : 'Promotion'"
              draggable="false"
            />
            <span v-else>{{ displayPieceSymbol(action.promotion ?? '') }}</span>
            <small>{{ action.promotion ? pieceLabel(action.promotion) : '선택' }}</small>
          </button>
        </div>
        <button class="btn-secondary" type="button" @click="promotionRequest = null">취소</button>
      </div>
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

        <div
          ref="labBoardElement"
          class="lab-board"
          :style="{ '--lab-board-size': boardSize }"
          @contextmenu.prevent
          @pointerdown="onLabBoardPointerDown"
        >
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
          <svg
            v-if="renderedArrows.length"
            class="lab-arrow-overlay"
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
                <path d="M 0 0 L 4 2 L 0 4 z" class="lab-arrow-head" />
              </marker>
            </defs>
            <line
              v-for="arrow in renderedArrows"
              :key="arrow.key"
              class="lab-arrow"
              :class="{ preview: arrow.preview }"
              :x1="arrow.x1"
              :y1="arrow.y1"
              :x2="arrow.x2"
              :y2="arrow.y2"
              :marker-end="`url(#${arrowMarkerId})`"
            />
          </svg>
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
                <button
                  class="btn-start"
                  type="button"
                  :class="{ active: activeAbilityId === ability.id }"
                  :disabled="!selectedLabPiece || !ability.connected || !ability.available"
                  @click="toggleAbility(ability)"
                >
                  {{ activeAbilityId === ability.id ? '해제' : '실행' }}
                </button>
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
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { api, type PieceLabAbilityOption } from '../api/gameApi'
import { pieceAsset } from '../pieceAssets'
import type { DeckPieceType } from '../types/deck'
import type { MoveAction, PlayerId, Square } from '../types/game'
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

interface LabArrow {
  from: string
  to: string
}

interface RenderedArrow extends LabArrow {
  key: string
  x1: number
  y1: number
  x2: number
  y2: number
  preview: boolean
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
const legalMoves = ref<MoveAction[]>([])
const attacks = ref<Square[]>([])
const globalState = ref<Record<string, number>>({})
const abilitySquares = ref<Square[]>([])
const abilities = ref<PieceLabAbilityOption[]>([])
const activeAbilityId = ref<string | null>(null)
const optionsLoading = ref(false)
const optionsError = ref<string | null>(null)
const draggedCatalogPiece = ref<string | null>(null)
const draggedPieceId = ref<string | null>(null)
const loadedOptionsPieceId = ref<string | null>(null)
const labBoardElement = ref<HTMLElement | null>(null)
const arrows = ref<LabArrow[]>([])
const rightDrag = ref<{
  pointerId: number
  from: string
  previewTo: string | null
} | null>(null)
const promotionRequest = ref<{
  pieceId: string
  owner: PlayerId
  to: Square
  actions: MoveAction[]
} | null>(null)
let nextPieceSerial = 1
let optionsSerial = 0
const arrowMarkerId = `lab-arrow-head-${Math.random().toString(36).slice(2)}`
const arrowViewBox = computed(() => `0 0 ${boardSize.value} ${boardSize.value}`)

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

watch(selectedPieceId, () => {
  activeAbilityId.value = null
})

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
    'cannon-rook': 'C',
    'tempest-queen': 'Q',
    'tempest-rook': 'T',
    'tempest-bishop': 'B',
    'tempest-knight': 'N',
    'bouncing-bishop': 'B',
    windmill: 'W',
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
    'cannon-rook': '기본은 Rook처럼 이동합니다. 특수능력으로 이번 선택 동안 장기의 포처럼 정확히 하나의 기물을 뛰어넘습니다.',
    bishop: '대각선으로 막히기 전까지 이동하고 공격합니다.',
    knight: 'L자 형태로 도약하며 중간 기물에 막히지 않습니다.',
    pawn: 'White는 위로, Black은 아래로 전진합니다. 대각선 전방을 공격하고 시작 위치에서는 2칸 전진할 수 있습니다.',
    amazon: 'Queen의 장거리 행마와 Knight의 도약 행마를 모두 사용합니다.',
    'tempest-rook': '대각선으로 한 칸 진입한 뒤 그 지점에서 가로 또는 세로 방향으로 뻗어 나갑니다.',
    'tempest-bishop': '가로 또는 세로로 한 칸 진입한 뒤 그 지점에서 대각선으로 뻗어 나갑니다.',
    'bouncing-bishop': '대각선으로 이동하며 보드 가장자리에서 반사되는 경로를 가집니다.',
    'tempest-pawn': 'Pawn과 같은 기본 행마를 사용하되, 승격 후보가 Tempest 계열 기물입니다.',
    'tempest-queen': '대각선 진입 후 가로, 세로, 대각선으로 폭풍처럼 뻗어 나갑니다.',
    'tempest-knight': '대각선 진입 후 확장된 Knight 계열 도약과 3칸 직선 도약을 사용합니다.',
    windmill: '성공적으로 이동할 때마다 Bishop 계열 대각선 행마와 Rook 계열 직선 행마가 번갈아 전환됩니다.',
  }
  return descriptions[pieceType] ?? 'Custom Piece: 등록된 Chessembly 행마법을 따릅니다.'
}

function staticAbilities(pieceType: string): PieceLabAbilityOption[] {
  if (pieceType === 'cannon-rook') {
    return [{
      id: 'cannon_move',
      name: '포 이동',
      description: '이번 이동 동안 장기의 포처럼 정확히 하나의 기물을 뛰어넘어 이동합니다. 사용 후 3턴 동안 다시 사용할 수 없습니다.',
      available: false,
      connected: true,
    }]
  }
  return []
}

function resetLab() {
  selectedOwner.value = 'white'
  selectedTool.value = props.initialPieceType ?? 'king'
  resetLabPieces()
}

function resetLabPieces() {
  pieces.value = []
  arrows.value = []
  globalState.value = {}
  cleanupRightDrag()
  selectedPieceId.value = null
  moves.value = []
  legalMoves.value = []
  attacks.value = []
  abilitySquares.value = []
  abilities.value = []
  activeAbilityId.value = null
  loadedOptionsPieceId.value = null
  promotionRequest.value = null
  optionsError.value = null
}

function clearSelection() {
  selectedPieceId.value = null
  moves.value = []
  legalMoves.value = []
  attacks.value = []
  abilitySquares.value = []
  abilities.value = []
  activeAbilityId.value = null
  loadedOptionsPieceId.value = null
  promotionRequest.value = null
  optionsError.value = null
}

function selectCatalogPiece(pieceType: DeckPieceType) {
  selectedTool.value = pieceType
  clearSelection()
}

function pieceAt(file: number, rank: number): PieceLabPiece | null {
  return pieces.value.find(piece => piece.square.file === file && piece.square.rank === rank) ?? null
}

function sameSquare(left: Square, right: Square): boolean {
  return left.file === right.file && left.rank === right.rank
}

function legalActionsForTarget(pieceId: string, to: Square): MoveAction[] {
  return legalMoves.value.filter(action => action.piece_id === pieceId && sameSquare(action.to, to))
}

function applyLabMove(action: MoveAction) {
  pieces.value = pieces.value
    .filter(piece => piece.id !== action.captured_piece_id)
    .map(piece => (
      piece.id === action.piece_id
        ? {
            ...piece,
            pieceType: action.promotion ?? piece.pieceType,
            square: action.to,
          }
        : piece
    ))
  if (action.set_state) {
    globalState.value = {
      ...globalState.value,
      [action.set_state.key]: action.set_state.value,
    }
  }
  selectedPieceId.value = action.piece_id
  loadedOptionsPieceId.value = null
  optionsError.value = null
  promotionRequest.value = null
}

function choosePromotion(action: MoveAction) {
  applyLabMove(action)
}

async function ensureLegalMovesForPiece(pieceId: string) {
  if (selectedPieceId.value !== pieceId) {
    selectedPieceId.value = pieceId
  }
  if (loadedOptionsPieceId.value === pieceId) return
  await loadSelectedPieceOptions()
}

async function tryMovePlacedPiece(pieceId: string, file: number, rank: number): Promise<boolean> {
  await ensureLegalMovesForPiece(pieceId)
  const piece = pieces.value.find(entry => entry.id === pieceId)
  if (!piece) return false

  const to = { file, rank }
  const actions = legalActionsForTarget(pieceId, to)
  if (actions.length === 0) return false

  const promotionActions = actions.filter(action => action.promotion)
  if (promotionActions.length > 0) {
    promotionRequest.value = {
      pieceId,
      owner: piece.owner,
      to,
      actions: promotionActions,
    }
    return true
  }

  applyLabMove(actions[0])
  return true
}

async function onSquareClick(file: number, rank: number) {
  const existing = pieceAt(file, rank)
  if (selectedTool.value === eraseTool) {
    if (!existing) return
    pieces.value = pieces.value.filter(piece => piece.id !== existing.id)
    if (selectedPieceId.value === existing.id) clearSelection()
    return
  }

  if (existing) {
    if (selectedPieceId.value && existing.id !== selectedPieceId.value) {
      const selected = selectedLabPiece.value
      if (selected && selected.owner !== existing.owner && await tryMovePlacedPiece(selectedPieceId.value, file, rank)) {
        return
      }
    }
    selectedPieceId.value = existing.id
    return
  }

  if (selectedPieceId.value) {
    if (!await tryMovePlacedPiece(selectedPieceId.value, file, rank)) {
      optionsError.value = '선택한 기물이 행마법상 이동할 수 없는 칸입니다.'
    }
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

function onLabBoardPointerDown(event: PointerEvent) {
  if (event.button === 0) {
    const squareId = squareIdFromClientPoint(event.clientX, event.clientY)
    if (squareId) {
      const square = squareFromId(squareId)
      if (square && !pieceAt(square.file, square.rank)) {
        clearArrows()
      }
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

  if (!to || to === drag.from) return

  toggleArrow({ from: drag.from, to })
}

function onWindowRightPointerCancel(event: PointerEvent) {
  const drag = rightDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return

  event.preventDefault()
  cleanupRightDrag()
}

function cleanupRightDrag() {
  rightDrag.value = null
  window.removeEventListener('pointermove', onWindowRightPointerMove)
  window.removeEventListener('pointerup', onWindowRightPointerUp)
  window.removeEventListener('pointercancel', onWindowRightPointerCancel)
  window.removeEventListener('contextmenu', preventRightDragContextMenu)
}

function onDocumentPointerDown(event: PointerEvent) {
  if (event.button !== 0 || !labBoardElement.value) return
  if (labBoardElement.value.contains(event.target as Node | null)) return

  clearArrows()
}

function clearArrows() {
  arrows.value = []
}

function preventRightDragContextMenu(event: MouseEvent) {
  if (!rightDrag.value) return

  event.preventDefault()
}

function toggleArrow(nextArrow: LabArrow) {
  const existingIndex = arrows.value.findIndex(
    arrow => arrow.from === nextArrow.from && arrow.to === nextArrow.to,
  )

  if (existingIndex >= 0) {
    arrows.value.splice(existingIndex, 1)
    return
  }

  arrows.value.push(nextArrow)
}

function squareIdFromClientPoint(clientX: number, clientY: number): string | null {
  const board = labBoardElement.value
  if (!board) return null

  const rect = board.getBoundingClientRect()
  const boardLeft = rect.left + board.clientLeft
  const boardTop = rect.top + board.clientTop
  const boardWidth = board.clientWidth
  const boardHeight = board.clientHeight
  const x = clientX - boardLeft
  const y = clientY - boardTop
  if (x < 0 || y < 0 || x >= boardWidth || y >= boardHeight) return null

  const displayFile = Math.floor((x / boardWidth) * boardSize.value)
  const displayRank = Math.floor((y / boardHeight) * boardSize.value)
  return squareId({
    file: displayFile,
    rank: boardSize.value - 1 - displayRank,
  })
}

function squareFromId(id: string): Square | null {
  const [file, rank] = id.split('_').map(Number)
  if (!Number.isFinite(file) || !Number.isFinite(rank)) return null
  return { file, rank }
}

function renderArrow(arrow: LabArrow, key: string, preview: boolean): RenderedArrow | null {
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

  return {
    x: square.file + 0.5,
    y: boardSize.value - 1 - square.rank + 0.5,
  }
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

async function onSquareDrop(event: DragEvent, file: number, rank: number) {
  const boardPieceId = draggedPieceId.value || event.dataTransfer?.getData('application/x-piece-lab-board-piece') || null
  const catalogPiece = draggedCatalogPiece.value || event.dataTransfer?.getData('application/x-piece-lab-catalog-piece') || null
  clearDragState()

  const existing = pieceAt(file, rank)
  if (boardPieceId) {
    const movingPiece = pieces.value.find(piece => piece.id === boardPieceId)
    if (existing && existing.id !== boardPieceId && movingPiece?.owner === existing.owner) {
      selectedPieceId.value = existing.id
      return
    }
    if (!await tryMovePlacedPiece(boardPieceId, file, rank)) {
      optionsError.value = '선택한 기물이 행마법상 이동할 수 없는 칸입니다.'
    }
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

async function toggleAbility(ability: PieceLabAbilityOption) {
  if (!selectedLabPiece.value || !ability.connected || !ability.available) return

  activeAbilityId.value = activeAbilityId.value === ability.id ? null : ability.id
  loadedOptionsPieceId.value = null
  await loadSelectedPieceOptions()
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
    loadedOptionsPieceId.value = null
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
      ability_id: activeAbilityId.value ?? undefined,
      global_state: globalState.value,
    })
    if (serial !== optionsSerial) return
    moves.value = response.moves
    legalMoves.value = response.legal_moves
    attacks.value = response.attacks
    abilitySquares.value = activeAbilityId.value ? response.moves : []
    abilities.value = response.abilities
    loadedOptionsPieceId.value = selected.id
  } catch (e: unknown) {
    if (serial !== optionsSerial) return
    moves.value = []
    legalMoves.value = []
    attacks.value = []
    abilitySquares.value = []
    abilities.value = []
    loadedOptionsPieceId.value = null
    optionsError.value = e instanceof Error ? e.message : String(e)
  } finally {
    if (serial === optionsSerial) optionsLoading.value = false
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointerDown)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown)
  cleanupRightDrag()
})

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

.lab-promotion-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 18px;
  background: rgba(5, 9, 14, 0.68);
}

.lab-promotion-box {
  width: min(420px, 100%);
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: #131a27;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.42);
}

.lab-promotion-box p {
  color: var(--muted);
}

.promotion-choices {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(90px, 1fr));
  gap: 10px;
}

.promotion-choice {
  min-height: 92px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  cursor: pointer;
}

.promotion-choice:hover {
  border-color: rgba(217, 164, 65, 0.58);
  background: rgba(217, 164, 65, 0.14);
}

.promotion-choice .piece-icon {
  width: 42px;
  height: 42px;
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
  position: relative;
  width: min(100%, 78vh);
  aspect-ratio: 1;
  margin: 0 auto;
  border: 2px solid rgba(244, 223, 176, 0.28);
  border-radius: 8px;
  overflow: hidden;
}

.lab-arrow-overlay {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 5;
}

.lab-arrow {
  stroke: rgba(236, 126, 30, 0.86);
  stroke-width: 0.16;
  stroke-linecap: round;
  stroke-linejoin: round;
  fill: none;
}

.lab-arrow.preview {
  opacity: 0.58;
}

.lab-arrow-head {
  fill: rgba(236, 126, 30, 0.86);
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

.ability-card .btn-start.active {
  background: var(--accent);
  color: #06121f;
  border-color: var(--accent);
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
