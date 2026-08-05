<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">목록으로</button>
      <div>
        <p class="eyebrow">Deck Editor</p>
        <h1>{{ deck.name || '이름 없는 덱' }}</h1>
      </div>
      <button class="btn-start" :disabled="!canSaveDeck" @click="save">덱 저장</button>
    </div>

    <section class="card editor-topbar">
      <label>
        <span class="limit-label">덱 이름</span>
        <input v-model.trim="deck.name" class="text-input" placeholder="덱 이름" />
      </label>
      <label>
        <span class="limit-label">보드 크기</span>
        <select v-model.number="deck.boardSize" class="text-input" @change="resetToClassic">
          <option v-for="size in boardSizes" :key="size" :value="size">
            {{ size }} x {{ size }} (최대 {{ scoreLimit(size) }}점)
          </option>
        </select>
      </label>
    </section>
    <p v-if="saveError" class="error">{{ saveError }}</p>
    <p v-if="catalogLoadError" class="error">{{ catalogLoadError }}</p>

    <section class="card preset-panel">
      <div class="section-header">
        <p class="section-kicker">Preset</p>
        <h2>프리셋 적용</h2>
      </div>
      <div class="preset-list">
        <button
          v-for="preset in activePresets"
          :key="preset.id"
          class="preset-card"
          @click="applyPreset(preset.id)"
        >
          <strong>{{ preset.name }}</strong>
          <span>{{ preset.description }}</span>
        </button>
      </div>
    </section>

    <section class="card deck-score-panel">
      <div class="deck-score-copy">
        <span class="limit-label">덱 점수</span>
        <strong>{{ deckSummary.totalScore }} / {{ deckSummary.scoreLimit }}점</strong>
        <span>{{ deckSummary.valid ? '게임 사용 가능' : '저장 가능 · 게임 사용 불가' }}</span>
      </div>
      <div class="deck-score-meter" :class="{ over: deckSummary.totalScore > deckSummary.scoreLimit }">
        <span :style="{ width: scoreFillWidth }"></span>
      </div>
    </section>

    <div class="builder-grid">
      <section class="card piece-list-panel">
        <div class="section-header">
          <p class="section-kicker">기물 목록</p>
          <h2>Arsenal</h2>
        </div>
        <input v-model.trim="pieceSearch" class="piece-search" type="search" placeholder="기물 검색" />
        <div class="piece-catalog">
          <div v-for="section in catalogSections" :key="section.id" class="catalog-section">
            <div class="catalog-section-title">
              <span>{{ section.label }}</span>
              <small>{{ section.pieces.length }}</small>
            </div>
            <div class="piece-palette">
              <div
                v-for="piece in section.pieces"
                :key="piece.id"
                class="palette-piece-row"
              >
                <button
                  class="palette-piece"
                  :class="{ active: placementTool === piece.id }"
                  draggable="true"
                  @click="placementTool = piece.id"
                  @dragstart="onPieceDragStart($event, piece.id)"
                  @dragend="draggedPiece = null"
                >
                  <span class="symbol">
                    <img
                      v-if="displayPieceAsset(piece.id)"
                      class="piece-icon"
                      :src="displayPieceAsset(piece.id)"
                      :alt="piece.name"
                      draggable="false"
                    />
                    <span v-else>{{ displayPieceSymbol(piece.id) }}</span>
                  </span>
                  <span class="meta">
                    <strong>{{ piece.name }}</strong>
                    <small>{{ piece.score === 0 ? '점수 제외' : `${piece.score}점` }}</small>
                    <small v-if="piece.custom">
                      커스텀 · v{{ piece.custom.version }} · 버전 고정
                      <template v-if="!piece.custom.active"> · 비활성화됨</template>
                    </small>
                  </span>
                  <span class="piece-count">{{ pieceCount(piece.id) }}</span>
                </button>
                <button
                  v-if="piece.custom && latestCustomVersion(piece.custom.id, piece.custom.version)"
                  class="piece-test-button"
                  @click="updatePinnedVersion(piece.id, piece.custom.id)"
                >
                  최신 버전으로 업데이트
                </button>
                <button class="piece-test-button" @click="emitTestPiece(piece.id)">테스트</button>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section class="card board-panel">
        <div class="section-header">
          <p class="section-kicker">시작 기물 배치</p>
          <div class="section-title-row">
            <h2>Frontline</h2>
            <span class="section-score-pill">{{ frontlineScore }}점 · {{ scorePercent(frontlineScore) }}%</span>
          </div>
        </div>
        <div class="placement-controls">
          <button class="tool-button" :class="{ active: placementTool === eraseTool }" @click="placementTool = eraseTool">
            <span>x</span>
            <strong>지우개</strong>
          </button>
          <div class="selected-tool">
            <span class="limit-label">선택 기물</span>
            <strong>{{ selectedToolLabel }}</strong>
          </div>
        </div>
        <div class="placement-board" :style="{ '--board-size': deck.boardSize }">
          <button
            v-for="square in baseZoneSquares"
            :key="`${square.file}_${square.rank}`"
            class="placement-square"
            :class="squareClass(square.file, square.rank)"
            @click="onPlacementSquareClick(square.file, square.rank)"
            @dragover.prevent="onPlacementDragOver"
            @drop.prevent="onPlacementDrop($event, square.file, square.rank)"
          >
            <span class="square-label">{{ fileLabel(square.file) }}{{ square.rank + 1 }}</span>
            <span v-if="pieceAt(square.file, square.rank)" class="square-piece">
              <img
                v-if="displayPieceAsset(pieceAt(square.file, square.rank)!)"
                class="piece-icon"
                :src="displayPieceAsset(pieceAt(square.file, square.rank)!)"
                :alt="pieceLabel(pieceAt(square.file, square.rank)!)"
                draggable="false"
              />
              <span v-else>{{ displayPieceSymbol(pieceAt(square.file, square.rank)!) }}</span>
            </span>
            <span v-else class="square-empty">+</span>
          </button>
        </div>
      </section>

      <section class="card pocket-panel">
        <div class="section-header">
          <p class="section-kicker">포켓 기물 구성</p>
          <div class="section-title-row">
            <h2>Pocket</h2>
            <span class="section-score-pill">{{ pocketScore }}점 · {{ scorePercent(pocketScore) }}%</span>
          </div>
        </div>
        <div
          class="pocket-drop-zone"
          :class="{ ready: draggedPiece && canUseInPocket(draggedPiece) }"
          @dragover.prevent="onPocketDragOver"
          @drop.prevent="onPocketDrop($event)"
        >
          <span>{{ pocketDropMessage }}</span>
        </div>
        <div v-if="activePocketCatalog.length > 0" class="pocket-summary">
          <div v-for="piece in activePocketCatalog" :key="piece.id" class="pocket-chip">
            <span class="symbol pocket-piece-symbol">
              <img
                v-if="displayPieceAsset(piece.id)"
                class="piece-icon"
                :src="displayPieceAsset(piece.id)"
                :alt="piece.name"
                draggable="false"
              />
              <span v-else>{{ displayPieceSymbol(piece.id) }}</span>
            </span>
            <span class="pocket-piece-name">{{ piece.name }}</span>
            <span class="pocket-quantity">
              <span class="pocket-quantity-bar">
                <span :style="{ width: pocketFillWidth(piece.id) }"></span>
              </span>
              <strong>{{ deck.pocket[piece.id] ?? 0 }}</strong>
            </span>
            <button class="pocket-remove-button" aria-label="포켓 기물 제거" @click="changePocketCount(piece.id, -1)">-</button>
          </div>
        </div>
        <div v-if="deckSummary.errors.length > 0" class="validation-list">
          <p v-for="message in deckSummary.errors" :key="message">{{ message }}</p>
        </div>
      </section>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { onMounted } from 'vue'
import { pieceAsset } from '../pieceAssets'
import { customPieceApi } from '../api/customPieceApi'
import type { DeckPieceType, SavedDeck } from '../types/deck'
import {
  boardSizes,
  canUseInPocket,
  catalogCategoryLabels,
  createPresetDeck,
  deckPresets,
  isUniqueStartingPiece,
  pieceCatalog,
  pieceLabel,
  pieceScore,
  presetLayoutForBoard,
  scoreLimit,
  validateSavedDeck,
  replaceCustomPieceCatalog,
} from '../composables/useDeckValidation'
import { createNewSavedDeck, useSavedDecks } from '../composables/useSavedDecks'

const props = defineProps<{
  deckId?: string | null
}>()

const emit = defineEmits<{
  back: []
  saved: []
  testPiece: [payload: { pieceType: string; boardSize: number }]
}>()

const savedDecks = useSavedDecks()
const eraseTool = '__erase__'
const pieceSearch = ref('')
const placementTool = ref<DeckPieceType>('king')
const draggedPiece = ref<DeckPieceType | null>(null)
const saveError = ref<string | null>(null)
const deck = ref<SavedDeck>(loadDeck())
const catalogLoadError = ref<string | null>(null)
const catalogRevision = ref(0)
const latestCustomPieces = ref<Awaited<ReturnType<typeof customPieceApi.list>>['items']>([])

onMounted(async () => {
  try {
    const { items } = await customPieceApi.list()
    latestCustomPieces.value = items
    const pinned = await Promise.all(
      (deck.value.customPieces ?? [])
        .filter(reference => !items.some(item => item.id === reference.id && item.version === reference.version))
        .map(reference => customPieceApi.getVersion(reference.id, reference.version).catch(() => null)),
    )
    replaceCustomPieceCatalog([...items, ...pinned.filter(item => item !== null)])
    catalogRevision.value += 1
  } catch (error) {
    catalogLoadError.value = error instanceof Error ? error.message : String(error)
  }
})

function loadDeck(): SavedDeck {
  if (props.deckId) {
    const existing = savedDecks.getDeck(props.deckId)
    if (existing) return cloneSavedDeck(existing)
  }
  return createNewSavedDeck()
}

function cloneSavedDeck(source: SavedDeck): SavedDeck {
  return {
    id: source.id,
    name: source.name,
    boardSize: source.boardSize,
    starting: source.starting.map(piece => ({
      pieceType: piece.pieceType,
      square: {
        file: piece.square.file,
        rank: piece.square.rank,
      },
    })),
    pocket: { ...source.pocket },
    createdAt: source.createdAt,
    updatedAt: source.updatedAt,
    customPieces: [...(source.customPieces ?? [])],
  }
}

watch(() => props.deckId, () => {
  deck.value = loadDeck()
})

const deckSummary = computed(() => {
  catalogRevision.value
  return validateSavedDeck(deck.value)
})
const canSaveDeck = computed(() => deck.value.name.trim().length > 0)
const activePresets = computed(() => deckPresets.filter(preset => presetLayoutForBoard(preset, deck.value.boardSize)))
const selectedToolLabel = computed(() => placementTool.value === eraseTool ? '지우개' : pieceLabel(placementTool.value))
const scoreFillWidth = computed(() => `${Math.min(100, Math.round((deckSummary.value.totalScore / deckSummary.value.scoreLimit) * 100))}%`)
const frontlineScore = computed(() => {
  catalogRevision.value
  return deck.value.starting.reduce((sum, piece) => sum + pieceScore(piece.pieceType), 0)
})
const pocketScore = computed(() => {
  catalogRevision.value
  return Object.entries(deck.value.pocket).reduce((sum, [pieceType, count]) => sum + pieceScore(pieceType) * count, 0)
})
const activePocketCatalog = computed(() => {
  catalogRevision.value
  return pieceCatalog.filter(piece => piece.canPocket && (deck.value.pocket[piece.id] ?? 0) > 0)
})
const maxPocketCount = computed(() => Math.max(1, ...activePocketCatalog.value.map(piece => deck.value.pocket[piece.id] ?? 0)))
const pocketDropMessage = computed(() => {
  if (!draggedPiece.value) return '여기에 드롭해서 포켓에 추가'
  if (!canUseInPocket(draggedPiece.value)) return `${pieceLabel(draggedPiece.value)}은 포켓에 넣을 수 없습니다.`
  return `${pieceLabel(draggedPiece.value)} 포켓에 추가`
})
const filteredPieceCatalog = computed(() => {
  catalogRevision.value
  const query = pieceSearch.value.toLowerCase()
  if (!query) return pieceCatalog
  return pieceCatalog.filter(piece => [piece.id, piece.name, piece.category, ...(piece.aliases ?? [])].join(' ').toLowerCase().includes(query))
})
const catalogSections = computed(() => {
  const groups = new Map<string, typeof pieceCatalog>()
  for (const piece of filteredPieceCatalog.value) {
    groups.set(piece.category, [...(groups.get(piece.category) ?? []), piece])
  }
  return Array.from(groups.entries()).map(([id, pieces]) => ({
    id,
    label: catalogCategoryLabels[id] ?? id,
    pieces,
  }))
})
const baseZoneSquares = computed(() => [1, 0].flatMap(rank => Array.from({ length: deck.value.boardSize }, (_, file) => ({ file, rank }))))

function fileLabel(file: number): string {
  return String.fromCharCode(97 + file)
}

function displayPieceAsset(pieceType: DeckPieceType): string | undefined {
  const custom = pieceCatalog.find(piece => piece.id === pieceType)?.custom
  if (custom?.image.kind === 'built_in') return pieceAsset(custom.image.asset_key, 'white')
  return pieceAsset(pieceType, 'white')
}

function latestCustomVersion(id: string, version: number): number | null {
  const latest = latestCustomPieces.value.find(piece => piece.id === id)
  return latest && latest.version > version ? latest.version : null
}

function updatePinnedVersion(oldPieceType: string, id: string) {
  const latest = latestCustomPieces.value.find(piece => piece.id === id)
  if (!latest) return
  const nextPieceType = `custom:${latest.id}:v${latest.version}:${latest.exposed_piece_key}`
  deck.value.starting = deck.value.starting.map(piece => (
    piece.pieceType === oldPieceType ? { ...piece, pieceType: nextPieceType } : piece
  ))
  const count = deck.value.pocket[oldPieceType] ?? 0
  if (count > 0) {
    deck.value.pocket[nextPieceType] = (deck.value.pocket[nextPieceType] ?? 0) + count
    delete deck.value.pocket[oldPieceType]
  }
  deck.value.customPieces = [
    ...(deck.value.customPieces ?? []).filter(piece => piece.id !== id),
    {
      id: latest.id,
      version: latest.version,
      contentHash: latest.content_hash,
      exposedPieceKey: latest.exposed_piece_key,
    },
  ]
}

function displayPieceSymbol(pieceType: DeckPieceType): string {
  const symbols: Partial<Record<DeckPieceType, string>> = {
    king: '♔',
    queen: '♕',
    amazon: 'A',
    'cannon-rook': 'C',
    'tempest-queen': 'Q',
    'tempest-rook': 'T',
    'tempest-bishop': 'B',
    'tempest-knight': 'N',
    'bouncing-bishop': 'B',
    nightrider: 'N',
    guhang: 'G',
    windmill: 'W',
    'tempest-pawn': '♙',
    dozer: 'D',
    rook: '♖',
    bishop: '♗',
    knight: '♘',
    pawn: '♙',
  }
  return symbols[pieceType] ?? pieceLabel(pieceType).slice(0, 1).toUpperCase()
}

function resetToClassic() {
  const base = createPresetDeck(deck.value.boardSize)
  deck.value.starting = base.starting
  deck.value.pocket = base.pocket
}

function applyPreset(presetId: string) {
  const base = createPresetDeck(deck.value.boardSize, presetId)
  deck.value.starting = base.starting
  deck.value.pocket = base.pocket
}

function pieceAt(file: number, rank: number): DeckPieceType | null {
  return deck.value.starting.find(piece => piece.square.file === file && piece.square.rank === rank)?.pieceType ?? null
}

function pieceCount(pieceType: DeckPieceType): number {
  return deck.value.starting.filter(piece => piece.pieceType === pieceType).length
}

function scorePercent(score: number): number {
  return Math.round((score / deckSummary.value.scoreLimit) * 100)
}

function pocketFillWidth(pieceType: DeckPieceType): string {
  const count = deck.value.pocket[pieceType] ?? 0
  return `${Math.round((count / maxPocketCount.value) * 100)}%`
}

function squareClass(file: number, rank: number): string[] {
  return [
    (file + rank) % 2 === 1 ? 'light' : 'dark',
    pieceAt(file, rank) ? 'occupied' : 'empty',
    draggedPiece.value ? 'drop-ready' : '',
  ].filter(Boolean)
}

function placePieceAt(pieceType: DeckPieceType, file: number, rank: number) {
  const existing = pieceAt(file, rank)
  if (existing === pieceType) {
    deck.value.starting = deck.value.starting.filter(piece => piece.square.file !== file || piece.square.rank !== rank)
    return
  }

  deck.value.starting = deck.value.starting.filter(piece => {
    if (piece.square.file === file && piece.square.rank === rank) return false
    if (isUniqueStartingPiece(pieceType) && piece.pieceType === pieceType) return false
    return true
  })
  deck.value.starting.push({ pieceType, square: { file, rank } })
}

function onPlacementSquareClick(file: number, rank: number) {
  if (placementTool.value === eraseTool) {
    deck.value.starting = deck.value.starting.filter(piece => piece.square.file !== file || piece.square.rank !== rank)
    return
  }
  placePieceAt(placementTool.value, file, rank)
}

function onPieceDragStart(event: DragEvent, pieceType: DeckPieceType) {
  draggedPiece.value = pieceType
  event.dataTransfer?.setData('application/x-brainfuck-chess-piece', pieceType)
  event.dataTransfer?.setData('text/plain', pieceType)
}

function getDraggedPiece(event: DragEvent): DeckPieceType | null {
  const fromEvent = event.dataTransfer?.getData('application/x-brainfuck-chess-piece')
    || event.dataTransfer?.getData('text/plain')
    || null
  const pieceType = draggedPiece.value ?? fromEvent
  return pieceType && pieceCatalog.some(piece => piece.id === pieceType) ? pieceType : null
}

function onPlacementDragOver(event: DragEvent) {
  if (draggedPiece.value && event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
}

function onPlacementDrop(event: DragEvent, file: number, rank: number) {
  const pieceType = getDraggedPiece(event)
  draggedPiece.value = null
  if (!pieceType) return
  placementTool.value = pieceType
  placePieceAt(pieceType, file, rank)
}

function onPocketDragOver(event: DragEvent) {
  if (!draggedPiece.value || !event.dataTransfer) return
  event.dataTransfer.dropEffect = canUseInPocket(draggedPiece.value) ? 'copy' : 'none'
}

function onPocketDrop(event: DragEvent) {
  const pieceType = getDraggedPiece(event)
  draggedPiece.value = null
  if (!pieceType || !canUseInPocket(pieceType)) return
  changePocketCount(pieceType, 1)
}

function changePocketCount(pieceType: DeckPieceType, delta: number) {
  if (!canUseInPocket(pieceType)) return
  deck.value.pocket[pieceType] ??= 0
  deck.value.pocket[pieceType] = Math.max(0, deck.value.pocket[pieceType] + delta)
}

function emitTestPiece(pieceType: DeckPieceType) {
  emit('testPiece', {
    pieceType,
    boardSize: deck.value.boardSize,
  })
}

function save() {
  saveError.value = null
  if (!canSaveDeck.value) {
    saveError.value = '덱 이름은 비어 있을 수 없습니다.'
    return
  }
  try {
    const usedTypes = new Set([
      ...deck.value.starting.map(piece => piece.pieceType),
      ...Object.entries(deck.value.pocket).filter(([, count]) => count > 0).map(([pieceType]) => pieceType),
    ])
    deck.value.customPieces = pieceCatalog
      .filter(piece => piece.custom && usedTypes.has(piece.id))
      .map(piece => ({
        id: piece.custom!.id,
        version: piece.custom!.version,
        contentHash: piece.custom!.contentHash,
        exposedPieceKey: piece.custom!.exposedPieceKey,
      }))
    savedDecks.saveDeck(cloneSavedDeck(deck.value))
    emit('saved')
  } catch (e: unknown) {
    saveError.value = e instanceof Error ? e.message : String(e)
  }
}
</script>
