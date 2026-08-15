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

    <div v-if="labAirdropOpen" class="lab-airdrop-overlay">
      <div class="lab-airdrop-box">
        <h2>공수부대 다중 소환</h2>
        <p>4점 이하 포켓 기물을 골라 전방 2×3 영역에 원하는 만큼 배치하세요.</p>
        <div class="lab-airdrop-layout">
          <div class="lab-airdrop-pocket">
            <button
              v-for="pieceId in labAirdropEligiblePieceIds"
              :key="pieceId"
              type="button"
              :class="{ selected: labAirdropSelectedPieceId === pieceId, used: labAirdropUsedPieceIds.has(pieceId) }"
              :disabled="labAirdropUsedPieceIds.has(pieceId)"
              @click="labAirdropSelectedPieceId = pieceId"
            >
              <span>{{ pieceLabel(pocketPieces.find(piece => piece.id === pieceId)?.pieceType ?? pieceId) }}</span>
              <small>{{ labDefinitions[pocketPieces.find(piece => piece.id === pieceId)?.pieceType ?? '']?.score ?? 0 }}점</small>
            </button>
          </div>
          <div class="lab-airdrop-grid">
            <button
              v-for="square in labAirdropSquares"
              :key="squareId(square)"
              type="button"
              :class="{ occupied: labAirdropDeploymentAt(square) }"
              @click="placeLabAirdrop(square)"
            >
              {{ labAirdropDeploymentLabel(square) || `${fileLabel(square.file)}${square.rank + 1}` }}
            </button>
          </div>
        </div>
        <small>{{ labAirdropDraft.length }}개 배치됨 · 배치된 칸을 다시 누르면 취소됩니다.</small>
        <div class="lab-airdrop-actions">
          <button class="btn-secondary" type="button" @click="cancelLabAirdrop">취소</button>
          <button class="btn-start" type="button" :disabled="labAirdropDraft.length === 0" @click="confirmLabAirdrop">한 번에 착수</button>
        </div>
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
          <button class="tool-button" :class="{ active: selectedTool === eraseTool }" @click="toggleEraseTool">
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
          <span>포켓: <strong>{{ pocketPieces.length }}</strong></span>
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
                v-if="placedPieceAsset(pieceAt(square.file, square.rank)!)"
                :key="placedPieceAsset(pieceAt(square.file, square.rank)!)"
                class="piece-icon"
                :src="placedPieceAsset(pieceAt(square.file, square.rank)!)"
                :alt="pieceLabel(pieceAt(square.file, square.rank)!.pieceType)"
                draggable="false"
              />
              <span v-else>{{ displayPieceSymbol(pieceAt(square.file, square.rank)!.pieceType) }}</span>
              <span
                v-if="labPieceCooldown(pieceAt(square.file, square.rank)!) > 0"
                class="lab-cooldown-badge"
                :title="`쿨타임 ${labPieceCooldown(pieceAt(square.file, square.rank)!)}턴`"
              >
                {{ labPieceCooldown(pieceAt(square.file, square.rank)!) }}
              </span>
            </span>
          </button>
          <svg
            v-if="renderedArrows.length || highlightedSquares.length"
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
            <circle
              v-for="highlight in renderedHighlights"
              :key="highlight.key"
              class="lab-square-highlight"
              :cx="highlight.x"
              :cy="highlight.y"
              r="0.36"
            />
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

      <section class="card lab-panel lab-pocket-panel">
        <div class="section-header compact">
          <div>
            <p class="section-kicker">Pocket</p>
            <h3>착수 테스트 포켓</h3>
          </div>
          <button
            class="btn-secondary pocket-add"
            type="button"
            :disabled="!canAddSelectedToolToPocket"
            @click="addSelectedToolToPocket"
          >+ 추가</button>
        </div>
        <p v-if="pocketPieces.length === 0" class="muted-note">포켓 사용 가능 기물을 선택하고 추가하세요.</p>
        <div v-else class="lab-pocket-list">
          <button
            v-for="piece in pocketPieces"
            :key="piece.id"
            type="button"
            class="lab-pocket-piece"
            :class="{ selected: selectedPieceId === piece.id }"
            draggable="true"
            @click="selectPocketPiece(piece.id)"
            @dragstart="onPocketPieceDragStart($event, piece.id)"
            @dragend="clearDragState"
          >
            <img v-if="displayPieceAsset(piece.pieceType, piece.owner)" class="piece-icon" :src="displayPieceAsset(piece.pieceType, piece.owner)" :alt="pieceLabel(piece.pieceType)" draggable="false" />
            <span v-else>{{ displayPieceSymbol(piece.pieceType) }}</span>
            <strong>{{ pieceLabel(piece.pieceType) }}</strong>
            <small>{{ piece.owner === 'white' ? 'White' : 'Black' }}</small>
            <span class="pocket-remove" title="포켓에서 제거" @click.stop="removePocketPiece(piece.id)">×</span>
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
                <button
                  class="btn-start"
                  type="button"
                  :class="{ active: activeAbilityId === ability.id }"
                  :disabled="!selectedLabPiece || !ability.available"
                  @click="toggleAbility(ability)"
                >
                  {{ activeAbilityId === ability.id ? '해제' : '실행' }}
                </button>
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
import { api, type PieceLabMoveOption } from '../api/gameApi'
import { activeCooldownRemaining, usesMoveSubmission } from '../moveOptionUi'
import { pieceAsset, renderedPieceAsset } from '../pieceAssets'
import type { DeckPieceType } from '../types/deck'
import type { AbilityAction, AbilityDeployment, DropAction, MoveAction, Piece, PieceDefinition, PieceStateValue, PlayerId, Square } from '../types/game'
import { boardSizes, pieceCatalog, pieceLabel, pocketCatalog } from '../composables/useDeckValidation'

interface PieceLabPiece {
  id: string
  pieceType: string
  owner: PlayerId
  square: Square
  state: Record<string, PieceStateValue>
  moveOptionCooldowns: Record<string, { remaining: number }>
}

interface PieceLabPocketPiece {
  id: string
  pieceType: string
  owner: PlayerId
  state: Record<string, PieceStateValue>
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
const pocketPieces = ref<PieceLabPocketPiece[]>([])
const pieceSearch = ref('')
const moves = ref<Square[]>([])
const legalMoves = ref<MoveAction[]>([])
const legalDrops = ref<DropAction[]>([])
const legalAbilityActions = ref<AbilityAction[]>([])
const attacks = ref<Square[]>([])
const globalState = ref<Record<string, number>>({})
const abilitySquares = ref<Square[]>([])
const abilities = ref<PieceLabMoveOption[]>([])
const labDefinitions = ref<Record<string, PieceDefinition>>({})
const activeAbilityId = ref<string | null>(null)
const labAirdropOpen = ref(false)
const labAirdropSelectedPieceId = ref<string | null>(null)
const labAirdropDraft = ref<AbilityDeployment[]>([])
const optionsLoading = ref(false)
const optionsError = ref<string | null>(null)
const draggedCatalogPiece = ref<string | null>(null)
const draggedPieceId = ref<string | null>(null)
const draggedPocketPieceId = ref<string | null>(null)
const loadedOptionsPieceId = ref<string | null>(null)
const labBoardElement = ref<HTMLElement | null>(null)
const arrows = ref<LabArrow[]>([])
const highlightedSquares = ref<string[]>([])
const lastMove = ref<{ from: Square; to: Square } | null>(null)
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
const selectedPocketPiece = computed(() => pocketPieces.value.find(piece => piece.id === selectedPieceId.value) ?? null)
const inspectedType = computed(() => selectedLabPiece.value?.pieceType ?? selectedPocketPiece.value?.pieceType ?? (selectedTool.value && selectedTool.value !== eraseTool ? selectedTool.value : null))
const inspectedOwner = computed(() => selectedLabPiece.value?.owner ?? selectedPocketPiece.value?.owner ?? selectedOwner.value)
const inspectedCatalogItem = computed(() => pieceCatalog.find(piece => piece.id === inspectedType.value) ?? null)
const selectedToolLabel = computed(() => {
  if (selectedTool.value === eraseTool) return '지우개'
  return selectedTool.value ? pieceLabel(selectedTool.value) : '없음'
})
const canAddSelectedToolToPocket = computed(() => Boolean(
  selectedTool.value && pocketCatalog.some(piece => piece.id === selectedTool.value),
))
const displayAbilities = computed<PieceLabMoveOption[]>(() => {
  if (selectedLabPiece.value || selectedPocketPiece.value) return abilities.value
  return inspectedType.value ? staticAbilities(inspectedType.value) : []
})
const activeAbility = computed(() => (
  abilities.value.find(ability => ability.id === activeAbilityId.value) ?? null
))
const abilitySummary = computed(() => {
  if (displayAbilities.value.length > 0) return `${displayAbilities.value.length}개 등록`
  return '없음'
})
const labAirdropEligiblePieceIds = computed(() => Array.from(new Set(
  legalAbilityActions.value.flatMap(action => action.pocket_piece_id ? [action.pocket_piece_id] : []),
)))
const labAirdropUsedPieceIds = computed(() => new Set(labAirdropDraft.value.map(item => item.pocket_piece_id)))
const labAirdropSquares = computed(() => {
  const seen = new Set<string>()
  return legalAbilityActions.value.flatMap(action => {
    if (!action.to || seen.has(squareId(action.to))) return []
    seen.add(squareId(action.to))
    return [action.to]
  }).sort((left, right) => right.rank - left.rank || left.file - right.file)
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
const renderedHighlights = computed(() => highlightedSquares.value.flatMap((squareId) => {
  const center = squareCenterFromId(squareId)
  return center ? [{ key: `highlight-${squareId}`, ...center }] : []
}))

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
  () => [selectedPieceId.value, pieces.value.map(piece => `${piece.id}:${piece.pieceType}:${piece.owner}:${piece.square.file}_${piece.square.rank}:${JSON.stringify(piece.state)}:${JSON.stringify(piece.moveOptionCooldowns)}`).join('|'), pocketPieces.value.map(piece => `${piece.id}:${piece.pieceType}:${piece.owner}:${JSON.stringify(piece.state)}`).join('|'), boardSize.value],
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
  return pieceAsset(labDefinitions.value[pieceType]?.visual?.default_asset_key ?? pieceType, owner)
}

function placedPieceAsset(piece: PieceLabPiece): string | undefined {
  const renderPiece: Piece = {
    id: piece.id,
    owner: piece.owner,
    type_id: piece.pieceType,
    current_square: piece.square,
    in_pocket: false,
    captured: false,
    has_moved: false,
    state: piece.state,
    move_option_cooldowns: piece.moveOptionCooldowns,
  }
  return renderedPieceAsset(renderPiece, labDefinitions.value[piece.pieceType])
}

function labPieceCooldown(piece: PieceLabPiece): number {
  return activeCooldownRemaining(piece.moveOptionCooldowns)
}

function initialPieceState(pieceType: string): Record<string, PieceStateValue> {
  return Object.fromEntries(
    (labDefinitions.value[pieceType]?.state_schema ?? [])
      .map(definition => [definition.key, definition.default_value]),
  )
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
    'bouncing-rook': 'R',
    'bouncing-queen': 'Q',
    'bouncing-pawn': 'P',
    nightrider: 'N',
    guhang: 'G',
    windmill: 'W',
    rook: 'R',
    bishop: 'B',
    knight: 'N',
    pawn: 'P',
    'tempest-pawn': 'P',
    dozer: 'D',
    paratrooper: 'P',
  }
  return symbols[pieceType] ?? pieceLabel(pieceType).slice(0, 1).toUpperCase()
}

function movementDescription(pieceType: string): string {
  const descriptions: Record<string, string> = {
    king: '8방향으로 한 칸 이동하고 공격합니다. 실제 게임에서는 캐슬링도 지원됩니다.',
    queen: '가로, 세로, 대각선으로 막히기 전까지 이동하고 공격합니다.',
    rook: '가로와 세로로 막히기 전까지 이동하고 공격합니다.',
    'cannon-rook': '기본은 Rook처럼 이동합니다. 특수능력으로 이번 선택 동안 아군 또는 상대 기물 정확히 하나를 뛰어넘습니다.',
    bishop: '대각선으로 막히기 전까지 이동하고 공격합니다.',
    knight: 'L자 형태로 도약하며 중간 기물에 막히지 않습니다.',
    pawn: 'White는 위로, Black은 아래로 전진합니다. 대각선 전방을 공격하고 시작 위치에서는 2칸 전진할 수 있습니다.',
    amazon: 'Queen의 장거리 행마와 Knight의 도약 행마를 모두 사용합니다.',
    'tempest-rook': '대각선으로 한 칸 진입한 뒤 그 지점에서 가로 또는 세로 방향으로 뻗어 나갑니다.',
    'tempest-bishop': '가로 또는 세로로 한 칸 진입한 뒤 그 지점에서 대각선으로 뻗어 나갑니다.',
    'bouncing-bishop': '대각선으로 이동하며 보드 가장자리에 도달하면 반사하여 계속 이동합니다.',
    'bouncing-rook': '직선으로 이동하며 보드 가장자리에 도달하면 직각 방향으로 꺾여 계속 이동합니다.',
    'bouncing-queen': '바운싱 비숍의 대각선 반사 행마와 바운싱 룩의 직선 꺾임 행마를 모두 사용합니다.',
    'bouncing-pawn': 'Pawn과 같은 기본 행마를 사용하되, 승격 후보가 Bouncing 계열 기물입니다.',
    nightrider: '나이트의 한 방향 도약을 같은 방향으로 반복하며, 다른 기물에 막히기 전까지 이동합니다.',
    guhang: '룩의 행마를 한 번더 반복하는 기물입니다.',
    'tempest-pawn': 'Pawn과 같은 기본 행마를 사용하되, 승격 후보가 Tempest 계열 기물입니다.',
    dozer: '앞쪽 다섯 칸 중 하나로 한 칸 이동하거나 잡으며, 끝 랭크에서 Knight 또는 Bishop으로 승격합니다.',
    'tempest-queen': '대각선 진입 후 가로, 세로, 대각선으로 폭풍처럼 뻗어 나갑니다.',
    'tempest-knight': '대각선 진입 후 확장된 Knight 계열 도약과 3칸 직선 도약을 사용합니다.',
    windmill: '성공적으로 이동할 때마다 Bishop 계열 대각선 행마와 Rook 계열 직선 행마가 번갈아 전환됩니다.',
    paratrooper: '이동할 수 없습니다. 빈 착수 가능 칸에 착수하거나, 착수 가능 칸의 적 기물을 잡으면서 착수할 수 있습니다.',
    'alternating-soldier': '왕처럼 8방향으로 한 칸 이동합니다. 교대를 사용하면 주변 8칸의 기물 하나와 자신의 포켓 기물 하나를 바꿉니다.',
    airborne: '왕처럼 8방향으로 한 칸 이동합니다. 공중 소환을 사용하면 전방 2×3 구역에 점수 4 이하인 포켓 기물을 소환합니다.',
    'green-camp': '왕처럼 8방향으로 한 칸 이동합니다. 포켓 복귀를 사용하면 주변 8칸의 기물 하나를 해당 기물 소유자의 포켓으로 돌려보냅니다.',
  }
  return descriptions[pieceType] ?? 'Custom Piece: 등록된 Chessembly 행마법을 따릅니다.'
}

function staticAbilities(pieceType: string): PieceLabMoveOption[] {
  if (pieceType === 'cannon-rook') {
    return [{
      id: 'cannon_move',
      name: '포 이동',
      description: '이번 이동 동안 장기의 포처럼 아군 또는 상대 기물 정확히 하나를 뛰어넘어 이동합니다. 사용 후 3턴 동안 다시 사용할 수 없습니다.',
      available: false,
      kind: 'ability',
      execution_mode: 'move_modifier',
      cooldown_remaining: 0,
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
  pocketPieces.value = []
  arrows.value = []
  highlightedSquares.value = []
  lastMove.value = null
  globalState.value = {}
  cleanupRightDrag()
  selectedPieceId.value = null
  moves.value = []
  legalMoves.value = []
  legalDrops.value = []
  legalAbilityActions.value = []
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
  legalDrops.value = []
  legalAbilityActions.value = []
  attacks.value = []
  abilitySquares.value = []
  abilities.value = []
  activeAbilityId.value = null
  loadedOptionsPieceId.value = null
  promotionRequest.value = null
  optionsError.value = null
}

function toggleEraseTool() {
  selectedTool.value = selectedTool.value === eraseTool ? null : eraseTool
  clearSelection()
}

function selectCatalogPiece(pieceType: DeckPieceType) {
  selectedTool.value = pieceType
  clearSelection()
}

function addSelectedToolToPocket() {
  const pieceType = selectedTool.value
  if (!pieceType || !pocketCatalog.some(piece => piece.id === pieceType)) return
  const piece: PieceLabPocketPiece = {
    id: `lab_${selectedOwner.value}_pocket_${pieceType.replace(/[^a-z0-9]+/gi, '_')}_${nextPieceSerial++}`,
    pieceType,
    owner: selectedOwner.value,
    state: initialPieceState(pieceType),
  }
  pocketPieces.value = [...pocketPieces.value, piece]
  selectedPieceId.value = piece.id
}

function selectPocketPiece(pieceId: string) {
  selectedPieceId.value = pieceId
}

function removePocketPiece(pieceId: string) {
  pocketPieces.value = pocketPieces.value.filter(piece => piece.id !== pieceId)
  if (selectedPieceId.value === pieceId) clearSelection()
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
  lastMove.value = { from: action.from, to: action.to }
  pieces.value = pieces.value
    .filter(piece => piece.id !== action.captured_piece_id)
    .map(piece => (
      piece.id === action.piece_id
        ? {
            ...piece,
            pieceType: action.promotion ?? piece.pieceType,
            square: action.to,
            state: action.promotion ? initialPieceState(action.promotion) : piece.state,
            moveOptionCooldowns: action.promotion ? {} : piece.moveOptionCooldowns,
          }
        : piece
    ))
  for (const update of action.effects.piece_state_updates) {
    const piece = pieces.value.find(piece => piece.id === update.piece_id)
    if (!piece) continue
    const definition = labDefinitions.value[piece.pieceType]
    if (!definition?.state_schema.some(state => state.key === update.key)) continue
    piece.state = { ...piece.state, [update.key]: update.value }
  }
  for (const update of action.effects.global_state_updates) {
    globalState.value = {
      ...globalState.value,
      [update.key]: update.value,
    }
  }
  const newlySetCooldowns = new Set(
    action.effects.cooldown_updates.map(update => `${update.piece_id}\u0000${update.move_option_id}`),
  )
  for (const update of action.effects.cooldown_updates) {
    const piece = pieces.value.find(piece => piece.id === update.piece_id)
    const option = piece
      ? labDefinitions.value[piece.pieceType]?.move_options.find(option => option.id === update.move_option_id)
      : undefined
    if (!piece || !option?.cooldown) continue
    piece.moveOptionCooldowns = {
      ...piece.moveOptionCooldowns,
      [update.move_option_id]: { remaining: update.remaining },
    }
  }
  // The lab has no server-side turn loop, so advance cooldown clocks locally
  // after every committed move. This mirrors the engine and deliberately skips
  // cooldowns created by the move that just happened.
  for (const piece of pieces.value) {
    const definition = labDefinitions.value[piece.pieceType]
    const nextCooldowns = { ...piece.moveOptionCooldowns }
    for (const [optionId, cooldown] of Object.entries(nextCooldowns)) {
      if (newlySetCooldowns.has(`${piece.id}\u0000${optionId}`)) continue
      const clock = definition?.move_options.find(option => option.id === optionId)?.cooldown?.clock
      const shouldTick = clock === 'global_turns'
        || (clock === 'owner_turns' && piece.owner === action.player_id)
      if (!shouldTick) continue
      const remaining = Math.max(0, cooldown.remaining - 1)
      if (remaining === 0) delete nextCooldowns[optionId]
      else nextCooldowns[optionId] = { remaining }
    }
    piece.moveOptionCooldowns = nextCooldowns
  }
  selectedPieceId.value = action.piece_id
  activeAbilityId.value = null
  abilitySquares.value = []
  loadedOptionsPieceId.value = null
  optionsError.value = null
  promotionRequest.value = null
}

function applyLabDrop(action: DropAction) {
  const pocketPiece = pocketPieces.value.find(piece => piece.id === action.piece_id)
  if (!pocketPiece) return
  lastMove.value = null
  pieces.value = pieces.value.filter(piece => piece.id !== action.captured_piece_id)
  pieces.value = [...pieces.value, {
    ...pocketPiece,
    square: action.to,
    moveOptionCooldowns: {},
  }]
  pocketPieces.value = pocketPieces.value.filter(piece => piece.id !== action.piece_id)
  selectedPieceId.value = action.piece_id
  moves.value = []
  legalDrops.value = []
  loadedOptionsPieceId.value = null
  optionsError.value = null
}

function applyLabAbility(action: AbilityAction) {
  if (!action.to) return
  const abilityActor = pieces.value.find(piece => piece.id === action.piece_id)
  const cooldown = abilityActor
    ? labDefinitions.value[abilityActor.pieceType]?.move_options
      .find(option => option.id === action.ability_id)?.cooldown
    : undefined
  if (action.ability_id === 'mortar-barrage') {
    const affectedSquares = new Set([
      action.to,
      { file: action.to.file + 1, rank: action.to.rank },
      { file: action.to.file - 1, rank: action.to.rank },
      { file: action.to.file, rank: action.to.rank + 1 },
      { file: action.to.file, rank: action.to.rank - 1 },
    ].map(squareId))
    pieces.value = pieces.value.filter(piece => !affectedSquares.has(squareId(piece.square)))
  } else if (action.ability_id === 'machine-gun-barrage') {
    if (!abilityActor) return
    const forward = abilityActor.owner === 'white' ? 1 : -1
    const affectedSquares = new Set(
      [1, 2].flatMap(depth => [-1, 0, 1].map(fileOffset => squareId({
        file: abilityActor.square.file + fileOffset,
        rank: abilityActor.square.rank + forward * depth,
      }))),
    )
    pieces.value = pieces.value.filter(piece => !affectedSquares.has(squareId(piece.square)))
  } else if (action.ability_id === 'relieve' && action.target_piece_id && action.pocket_piece_id) {
    const target = pieces.value.find(piece => piece.id === action.target_piece_id)
    const reserve = pocketPieces.value.find(piece => piece.id === action.pocket_piece_id)
    if (!target || !reserve) return
    pieces.value = pieces.value.filter(piece => piece.id !== target.id)
    pocketPieces.value = pocketPieces.value.filter(piece => piece.id !== reserve.id)
    pocketPieces.value = [...pocketPieces.value, { ...target, square: undefined } as PieceLabPocketPiece]
    pieces.value = [...pieces.value, { ...reserve, square: action.to, moveOptionCooldowns: {} }]
  } else if (action.ability_id === 'airdrop' && action.pocket_piece_id) {
    const reserve = pocketPieces.value.find(piece => piece.id === action.pocket_piece_id)
    if (!reserve) return
    pocketPieces.value = pocketPieces.value.filter(piece => piece.id !== reserve.id)
    pieces.value = [...pieces.value, { ...reserve, square: action.to, moveOptionCooldowns: {} }]
  } else if (action.ability_id === 'recall' && action.target_piece_id) {
    const target = pieces.value.find(piece => piece.id === action.target_piece_id)
    if (!target) return
    pieces.value = pieces.value.filter(piece => piece.id !== target.id)
    pocketPieces.value = [...pocketPieces.value, { ...target, square: undefined } as PieceLabPocketPiece]
  }
  const survivingActor = pieces.value.find(piece => piece.id === action.piece_id)
  const newlySetCooldown = Boolean(survivingActor && cooldown && cooldown.turns > 0)
  if (survivingActor && cooldown && cooldown.turns > 0) {
    survivingActor.moveOptionCooldowns = {
      ...survivingActor.moveOptionCooldowns,
      [action.ability_id]: { remaining: cooldown.turns },
    }
  }
  for (const piece of pieces.value) {
    const definition = labDefinitions.value[piece.pieceType]
    const nextCooldowns = { ...piece.moveOptionCooldowns }
    for (const [optionId, state] of Object.entries(nextCooldowns)) {
      if (newlySetCooldown && piece.id === action.piece_id && optionId === action.ability_id) continue
      const clock = definition?.move_options.find(option => option.id === optionId)?.cooldown?.clock
      const shouldTick = clock === 'global_turns'
        || (clock === 'owner_turns' && piece.owner === action.player_id)
      if (!shouldTick) continue
      const remaining = Math.max(0, state.remaining - 1)
      if (remaining === 0) delete nextCooldowns[optionId]
      else nextCooldowns[optionId] = { remaining }
    }
    piece.moveOptionCooldowns = nextCooldowns
  }
  activeAbilityId.value = null
  abilitySquares.value = []
  legalAbilityActions.value = []
  loadedOptionsPieceId.value = null
  optionsError.value = null
  if (!pieces.value.some(piece => piece.id === selectedPieceId.value)) selectedPieceId.value = null
}

function tryLabAbility(to: Square): boolean {
  const candidates = legalAbilityActions.value.filter(action => action.to && sameSquare(action.to, to))
  if (candidates.length === 0) return false
  let chosen = candidates[0]
  if (candidates.length > 1) {
    const choices = candidates.map((action, index) => {
      const pocket = action.pocket_piece_id
        ? pocketPieces.value.find(piece => piece.id === action.pocket_piece_id)
        : null
      return `${index + 1}: ${pocket ? pieceLabel(pocket.pieceType) : '대상 기물'}`
    })
    const selected = Number(window.prompt(`사용할 포켓 기물을 고르세요.\n${choices.join('\n')}`, '1')) - 1
    if (!Number.isInteger(selected) || !candidates[selected]) return false
    chosen = candidates[selected]
  }
  applyLabAbility(chosen)
  return true
}

function tryDropPocketPiece(pieceId: string, file: number, rank: number): boolean {
  const action = legalDrops.value.find(drop => drop.piece_id === pieceId && sameSquare(drop.to, { file, rank }))
  if (!action) return false
  applyLabDrop(action)
  return true
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
  if (activeAbilityId.value && selectedLabPiece.value) {
    const used = usesMoveSubmission(activeAbility.value)
      ? await tryMovePlacedPiece(selectedLabPiece.value.id, file, rank)
      : tryLabAbility({ file, rank })
    if (!used) {
      optionsError.value = usesMoveSubmission(activeAbility.value)
        ? '선택한 특수 이동으로 갈 수 없는 칸입니다.'
        : '특수능력을 사용할 수 없는 대상입니다.'
    }
    return
  }
  if (selectedTool.value === eraseTool) {
    if (!existing) return
    pieces.value = pieces.value.filter(piece => piece.id !== existing.id)
    if (selectedPieceId.value === existing.id) clearSelection()
    return
  }

  if (selectedPocketPiece.value) {
    if (!tryDropPocketPiece(selectedPocketPiece.value.id, file, rank)) {
      clearSelection()
      optionsError.value = '선택한 포켓 기물을 착수할 수 없는 칸입니다.'
    }
    return
  }

  if (existing) {
    if (selectedPieceId.value && existing.id !== selectedPieceId.value) {
      const selected = selectedLabPiece.value
      if (selected && selected.owner !== existing.owner && await tryMovePlacedPiece(selectedPieceId.value, file, rank)) {
        return
      }
      if (selected && selected.owner !== existing.owner) {
        clearSelection()
        optionsError.value = '선택한 기물이 행마법상 이동할 수 없는 칸입니다.'
        return
      }
    }
    selectedPieceId.value = existing.id
    return
  }

  if (selectedPieceId.value) {
    if (!await tryMovePlacedPiece(selectedPieceId.value, file, rank)) {
      clearSelection()
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
    state: initialPieceState(selectedTool.value),
    moveOptionCooldowns: {},
  }
  pieces.value = [...pieces.value, piece]
  selectedPieceId.value = piece.id
}

function onCatalogDragStart(event: DragEvent, pieceType: DeckPieceType) {
  draggedCatalogPiece.value = pieceType
  draggedPieceId.value = null
  draggedPocketPieceId.value = null
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
  draggedPocketPieceId.value = null
  selectedPieceId.value = pieceId
  event.dataTransfer?.setData('application/x-piece-lab-board-piece', pieceId)
  event.dataTransfer?.setData('text/plain', pieceId)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
}

function onPocketPieceDragStart(event: DragEvent, pieceId: string) {
  draggedPocketPieceId.value = pieceId
  draggedPieceId.value = null
  draggedCatalogPiece.value = null
  selectedPieceId.value = pieceId
  event.dataTransfer?.setData('application/x-piece-lab-pocket-piece', pieceId)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function clearDragState() {
  draggedCatalogPiece.value = null
  draggedPieceId.value = null
  draggedPocketPieceId.value = null
}

function onLabBoardPointerDown(event: PointerEvent) {
  if (event.button === 0) {
    const squareId = squareIdFromClientPoint(event.clientX, event.clientY)
    if (squareId) {
      const square = squareFromId(squareId)
      if (square && !pieceAt(square.file, square.rank)) {
        clearAnnotations()
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

  clearAnnotations()
  const target = event.target
  if (!(target instanceof Element) || !target.closest('button, input, select, textarea, a, label')) {
    clearSelection()
  }
}

function clearAnnotations() {
  arrows.value = []
  highlightedSquares.value = []
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

function toggleHighlight(squareId: string) {
  const existingIndex = highlightedSquares.value.indexOf(squareId)
  if (existingIndex >= 0) {
    highlightedSquares.value.splice(existingIndex, 1)
    return
  }

  highlightedSquares.value.push(squareId)
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
  const pocketPieceId = draggedPocketPieceId.value || event.dataTransfer.getData('application/x-piece-lab-pocket-piece')
  if (pocketPieceId) {
    event.dataTransfer.dropEffect = legalDrops.value.some(action => action.piece_id === pocketPieceId && sameSquare(action.to, { file, rank })) ? 'move' : 'none'
    return
  }
  const existing = pieceAt(file, rank)
  if (draggedCatalogPiece.value || Array.from(event.dataTransfer.types).includes('application/x-piece-lab-catalog-piece')) {
    event.dataTransfer.dropEffect = selectedTool.value === eraseTool ? 'none' : 'copy'
    return
  }
  const boardPieceId = draggedPieceId.value || event.dataTransfer.getData('application/x-piece-lab-board-piece')
  event.dataTransfer.dropEffect = boardPieceId && (!existing || existing.id === boardPieceId) ? 'move' : 'none'
}

async function onSquareDrop(event: DragEvent, file: number, rank: number) {
  const pocketPieceId = draggedPocketPieceId.value || event.dataTransfer?.getData('application/x-piece-lab-pocket-piece') || null
  const boardPieceId = draggedPieceId.value || event.dataTransfer?.getData('application/x-piece-lab-board-piece') || null
  const catalogPiece = draggedCatalogPiece.value || event.dataTransfer?.getData('application/x-piece-lab-catalog-piece') || null
  clearDragState()

  const existing = pieceAt(file, rank)
  if (pocketPieceId) {
    if (!tryDropPocketPiece(pocketPieceId, file, rank)) {
      optionsError.value = '선택한 포켓 기물을 착수할 수 없는 칸입니다.'
    }
    return
  }
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
        state: initialPieceState(catalogPiece),
        moveOptionCooldowns: {},
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
    lastMove.value && (sameSquare(square, lastMove.value.from) || sameSquare(square, lastMove.value.to)) ? 'last-move' : '',
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

async function toggleAbility(ability: PieceLabMoveOption) {
  if (!selectedLabPiece.value || !ability.available) return

  activeAbilityId.value = activeAbilityId.value === ability.id ? null : ability.id
  loadedOptionsPieceId.value = null
  await loadSelectedPieceOptions()
  if (activeAbilityId.value === 'airdrop' && legalAbilityActions.value.length > 0) {
    labAirdropDraft.value = []
    labAirdropSelectedPieceId.value = labAirdropEligiblePieceIds.value[0] ?? null
    labAirdropOpen.value = true
  }
}

function labAirdropDeploymentAt(square: Square): AbilityDeployment | undefined {
  return labAirdropDraft.value.find(item => sameSquare(item.to, square))
}

function labAirdropDeploymentLabel(square: Square): string {
  const deployment = labAirdropDeploymentAt(square)
  const pocket = deployment
    ? pocketPieces.value.find(piece => piece.id === deployment.pocket_piece_id)
    : null
  return pocket ? pieceLabel(pocket.pieceType) : ''
}

function placeLabAirdrop(square: Square) {
  const existing = labAirdropDeploymentAt(square)
  if (existing) {
    labAirdropDraft.value = labAirdropDraft.value.filter(item => !sameSquare(item.to, square))
    labAirdropSelectedPieceId.value = existing.pocket_piece_id
    return
  }
  const pieceId = labAirdropSelectedPieceId.value
  if (!pieceId || labAirdropUsedPieceIds.value.has(pieceId)) return
  const legal = legalAbilityActions.value.some(action =>
    action.pocket_piece_id === pieceId && action.to && sameSquare(action.to, square))
  if (!legal) return
  labAirdropDraft.value = [...labAirdropDraft.value, { pocket_piece_id: pieceId, to: square }]
  labAirdropSelectedPieceId.value = labAirdropEligiblePieceIds.value.find(id => !labAirdropUsedPieceIds.value.has(id)) ?? null
}

function cancelLabAirdrop() {
  labAirdropOpen.value = false
  labAirdropDraft.value = []
  labAirdropSelectedPieceId.value = null
  activeAbilityId.value = null
  abilitySquares.value = []
}

function confirmLabAirdrop() {
  if (labAirdropDraft.value.length === 0) return
  const deployments = [...labAirdropDraft.value]
  const deployedIds = new Set(deployments.map(item => item.pocket_piece_id))
  const deployed = pocketPieces.value
    .filter(piece => deployedIds.has(piece.id))
    .map(piece => {
      const deployment = deployments.find(item => item.pocket_piece_id === piece.id)!
      return { ...piece, square: deployment.to, moveOptionCooldowns: {} }
    })
  pocketPieces.value = pocketPieces.value.filter(piece => !deployedIds.has(piece.id))
  pieces.value = [...pieces.value, ...deployed]
  lastMove.value = null
  labAirdropOpen.value = false
  labAirdropDraft.value = []
  labAirdropSelectedPieceId.value = null
  activeAbilityId.value = null
  abilitySquares.value = []
  legalAbilityActions.value = []
  loadedOptionsPieceId.value = null
  optionsError.value = null
}

function isAbilitySquare(square: Square): boolean {
  const id = squareId(square)
  return abilitySquares.value.some(abilitySquare => squareId(abilitySquare) === id)
}

async function loadSelectedPieceOptions() {
  const selected = selectedLabPiece.value ?? selectedPocketPiece.value
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
        state: piece.state,
        move_option_cooldowns: piece.moveOptionCooldowns,
      })),
      pocket_pieces: pocketPieces.value.map(piece => ({
        id: piece.id,
        piece_type: piece.pieceType,
        owner: piece.owner,
        state: piece.state,
      })),
      custom_pieces: pieceCatalog
        .filter(piece => piece.custom && (
          pieces.value.some(placed => placed.pieceType === piece.id)
          || pocketPieces.value.some(pocket => pocket.pieceType === piece.id)
        ))
        .map(piece => ({
          custom_piece_id: piece.custom!.id,
          version: piece.custom!.version,
          content_hash: piece.custom!.contentHash,
          exposed_piece_key: piece.custom!.exposedPieceKey,
        })),
      selected_piece_id: selected.id,
      move_option_id: activeAbilityId.value ?? undefined,
      global_state: globalState.value,
    })
    if (serial !== optionsSerial) return
    moves.value = response.moves
    legalMoves.value = response.legal_moves
    legalDrops.value = response.legal_drops
    legalAbilityActions.value = response.legal_ability_actions
    attacks.value = response.attacks
    abilitySquares.value = activeAbilityId.value ? response.moves : []
    labDefinitions.value = response.piece_definitions
    pieces.value = pieces.value.map(piece => ({
      ...piece,
      state: response.piece_states[piece.id] ?? piece.state,
      moveOptionCooldowns: response.piece_cooldowns[piece.id] ?? piece.moveOptionCooldowns,
    }))
    pocketPieces.value = pocketPieces.value.map(piece => ({
      ...piece,
      state: response.piece_states[piece.id] ?? piece.state,
    }))
    abilities.value = response.move_options.filter(option => option.kind === 'ability')
    loadedOptionsPieceId.value = selected.id
  } catch (e: unknown) {
    if (serial !== optionsSerial) return
    moves.value = []
    legalMoves.value = []
    legalDrops.value = []
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
  width: min(1600px, 100%);
}

.piece-lab-grid {
  display: grid;
  grid-template-columns: minmax(230px, 0.75fr) minmax(500px, 1.7fr) minmax(200px, 0.6fr) minmax(270px, 0.85fr);
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

.lab-pocket-panel {
  display: grid;
  gap: 10px;
  align-content: start;
}

.section-header.compact {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.pocket-add {
  width: auto;
  padding: 7px 10px;
}

.lab-pocket-list {
  display: grid;
  gap: 7px;
}

.lab-pocket-piece {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr) auto 22px;
  align-items: center;
  gap: 8px;
  min-height: 46px;
  padding: 6px 8px;
  border: 1px solid var(--line);
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  text-align: left;
  cursor: grab;
}

.lab-pocket-piece.selected {
  border-color: #d9a441;
  background: rgba(217, 164, 65, 0.14);
}

.lab-pocket-piece .piece-icon { width: 32px; height: 32px; }
.lab-pocket-piece small { color: var(--muted); }
.pocket-remove { color: #e78b8b; font-size: 20px; text-align: center; cursor: pointer; }

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
  stroke: rgba(34, 139, 72, 0.86);
  stroke-width: 0.16;
  stroke-linecap: round;
  stroke-linejoin: round;
  fill: none;
}

.lab-arrow.preview {
  opacity: 0.58;
}

.lab-arrow-head {
  fill: rgba(34, 139, 72, 0.86);
}

.lab-square-highlight {
  fill: rgba(34, 139, 72, 0.18);
  stroke: rgba(34, 139, 72, 0.9);
  stroke-width: 0.1;
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
.lab-square.last-move { background: #f2d34f; color: #232a38; }

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

.lab-cooldown-badge {
  position: absolute;
  right: -5%;
  bottom: -5%;
  z-index: 5;
  display: inline-flex;
  min-width: 1.45em;
  height: 1.45em;
  padding: 0 0.28em;
  align-items: center;
  justify-content: center;
  border: 2px solid rgba(255, 255, 255, 0.92);
  border-radius: 999px;
  background: #b4232f;
  color: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
  font-size: clamp(10px, 1.45vw, 15px);
  font-weight: 800;
  line-height: 1;
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

.lab-airdrop-overlay {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgba(3, 12, 22, 0.76);
}

.lab-airdrop-box {
  width: min(680px, 100%);
  padding: 24px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: #102235;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.45);
}

.lab-airdrop-box h2,
.lab-airdrop-box p { margin-top: 0; }
.lab-airdrop-box p,
.lab-airdrop-box > small { color: var(--muted); }
.lab-airdrop-layout { display: grid; grid-template-columns: minmax(180px, 1fr) minmax(240px, 1.2fr); gap: 18px; margin: 18px 0 12px; }
.lab-airdrop-pocket { display: grid; align-content: start; gap: 8px; max-height: 280px; overflow: auto; }
.lab-airdrop-pocket button { display: flex; justify-content: space-between; padding: 10px; border: 2px solid var(--line); border-radius: 8px; color: inherit; background: rgba(255,255,255,.04); cursor: pointer; }
.lab-airdrop-pocket button.selected { border-color: var(--accent); background: rgba(82, 208, 255, .14); }
.lab-airdrop-pocket button.used { opacity: .42; }
.lab-airdrop-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.lab-airdrop-grid button { min-height: 72px; padding: 6px; border: 2px dashed #66839e; border-radius: 8px; color: inherit; background: rgba(255,255,255,.04); cursor: pointer; }
.lab-airdrop-grid button.occupied { border-style: solid; border-color: #66d58a; background: rgba(102, 213, 138, .14); font-weight: 800; }
.lab-airdrop-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 18px; }

@media (max-width: 1200px) {
  .lab-airdrop-layout { grid-template-columns: 1fr; }
  .piece-lab-grid {
    grid-template-columns: 1fr;
  }

  .lab-board {
    width: min(100%, 86vh);
  }
}
</style>
