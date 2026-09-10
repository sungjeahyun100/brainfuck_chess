<template>
  <main class="lobby">
    <template v-if="!deckLoaded">
      <button class="btn-secondary" @click="$emit('back')">목록으로</button>
      <p v-if="loadError" class="error" role="alert">{{ loadError }}</p>
      <p v-else role="status">덱을 불러오는 중…</p>
      <button v-if="loadError" class="btn-secondary" @click="loadDeck">다시 불러오기</button>
    </template>
    <template v-else>
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">목록으로</button>
      <div>
        <p class="eyebrow">Deck Editor</p>
        <h1>{{ deck.name || '이름 없는 덱' }}</h1>
      </div>
      <div class="deck-editor-actions">
        <button class="btn-secondary" :disabled="!canCopyDeckCode" @click="copyDeckCode">덱 코드 복사</button>
        <button class="btn-secondary" @click="openImportDialog">덱 코드 불러오기</button>
        <button class="btn-secondary danger" @click="resetDeck">전체 초기화</button>
        <button class="btn-start" :disabled="!canSaveDeck || saving" @click="save">{{ saving ? '저장 중…' : '덱 저장' }}</button>
      </div>
    </div>

    <section class="card editor-topbar">
      <label>
        <span class="limit-label">덱 이름</span>
        <input v-model.trim="deck.name" class="text-input" placeholder="덱 이름" />
      </label>
      <label>
        <span class="limit-label">전용 맵</span>
        <select v-model="deck.mapId" class="text-input" @change="changeMap">
          <option v-for="map in boardMaps" :key="map.id" :value="map.id">
            {{ map.name }} (최대 {{ scoreLimit(map.boardSize, deck.ruleset) }}점)
          </option>
        </select>
      </label>
      <label>
        <span class="limit-label">룰</span>
        <select v-model="deck.ruleset" class="text-input">
          <option v-for="ruleset in deckRulesets" :key="ruleset.id" :value="ruleset.id">{{ ruleset.label }}</option>
        </select>
      </label>
    </section>
    <p v-if="deck.ruleset === 'standard'" class="deck-code-notice">
      Standard는 중앙 Back과 주변 Front를 기본 진영으로 사용합니다. Front의 모든 칸을 채워야 합니다. 룰 변경 시 기존 배치는 유지되며, 기본 배치는 아래 프리셋으로 적용할 수 있습니다.
    </p>
    <p v-if="saveError" class="error">{{ saveError }}</p>
    <p v-if="saveNotice" class="deck-code-notice" role="status">{{ saveNotice }}</p>
    <p v-if="deckCodeNotice" class="deck-code-notice" :class="{ error: deckCodeNoticeIsError }" role="status">
      {{ deckCodeNotice }}
    </p>
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
        <span class="limit-label">{{ deck.ruleset === 'standard' ? 'Main Deck 점수' : '덱 점수' }}</span>
        <strong>{{ deckSummary.totalScore }} / {{ deckSummary.scoreLimit }}점</strong>
        <span>{{ deckSummary.valid ? '게임 사용 가능' : (canSaveDeck ? '저장 가능 · 게임 사용 불가' : '저장 불가') }}</span>
      </div>
      <div class="deck-score-meter" :class="{ over: deckSummary.totalScore > deckSummary.scoreLimit }">
        <span :style="{ width: scoreFillWidth }"></span>
      </div>
    </section>

    <section v-if="deck.ruleset === 'standard'" class="card extra-deck-panel">
      <h2>Extra Deck <small>{{ extraPieces.length }} / {{ MAX_EXTRA_DECK_PIECES }}기</small></h2>
      <p>각 기물의 점수는 유지되며 Main Deck 점수 상한에 포함되지 않습니다.</p>
      <div class="extra-slots">
        <div v-for="(pieceType, index) in extraPieces" :key="index" class="extra-slot">
          <img v-if="displayPieceAsset(pieceType)" :src="displayPieceAsset(pieceType)" alt="" />
          <strong>{{ pieceLabel(pieceType) }}</strong><span>{{ pieceScore(pieceType) }}점</span>
          <button class="btn-secondary" :aria-label="`${pieceLabel(pieceType)} Extra에서 제거`" @click="removeExtra(index)">제거</button>
        </div>
        <div v-for="index in Math.max(0, MAX_EXTRA_DECK_PIECES - extraPieces.length)" :key="`empty-${index}`" class="extra-slot empty">빈 슬롯</div>
      </div>
      <div class="extra-choices">
        <button v-for="piece in extraCatalog" :key="piece.id" class="btn-secondary" :disabled="extraPieces.length >= MAX_EXTRA_DECK_PIECES" @click="addExtra(piece.id)">
          {{ piece.name }} · {{ piece.score }}점 추가
        </button>
      </div>
    </section>
    <p v-else-if="extraPieces.length" class="error" role="alert">
      Legacy에서는 남아 있는 Extra {{ extraPieces.length }}기를 사용할 수 없습니다. 내용은 보존됩니다. Standard로 전환해 제거하거나 전체 초기화를 사용해 주세요.
    </p>

    <div class="builder-grid">
      <div class="piece-catalog-column">
        <section class="card piece-list-panel">
          <div class="section-header">
            <p class="section-kicker">공식 기물 목록</p>
            <h2>Arsenal</h2>
          </div>
          <input v-model.trim="arsenalPieceSearch" class="piece-search" type="search" placeholder="공식 기물 검색" />
          <div class="piece-catalog">
            <section
              v-for="section in arsenalCatalogSections"
              :key="section.id"
              class="catalog-section catalog-zone-chunk"
              :class="`catalog-zone-${section.id}`"
            >
              <div class="catalog-section-title">
                <div>
                  <span>{{ section.label }}</span>
                  <small class="catalog-zone-description">{{ section.description }}</small>
                </div>
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
                    </span>
                    <span class="piece-count">{{ pieceCount(piece.id) }}</span>
                  </button>
                  <button class="piece-test-button" @click="emitTestPiece(piece.id)">테스트</button>
                </div>
              </div>
            </section>
          </div>
        </section>

        <section class="card custom-piece-list-panel">
          <div class="section-header">
            <p class="section-kicker">사용자 제작 기물</p>
            <h2>내 커스텀 기물</h2>
          </div>
          <input v-model.trim="customPieceSearch" class="piece-search" type="search" placeholder="커스텀 기물 검색" />
          <p v-if="customCatalogSections.length === 0" class="catalog-empty">
            {{ customPieceSearch ? '검색 결과가 없습니다.' : '사용할 수 있는 커스텀 기물이 없습니다.' }}
          </p>
          <div v-else class="piece-catalog custom-piece-catalog">
            <section
              v-for="section in customCatalogSections"
              :key="section.id"
              class="catalog-section catalog-zone-chunk"
              :class="`catalog-zone-${section.id}`"
            >
              <div class="catalog-section-title">
                <div>
                  <span>{{ section.label }}</span>
                  <small class="catalog-zone-description">{{ section.description }}</small>
                </div>
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
                        v{{ piece.custom.version }} · 버전 고정
                        <template v-if="!piece.custom.active"> · 비활성화됨</template>
                      </small>
                    </span>
                    <span class="piece-count">{{ pieceCount(piece.id) }}</span>
                  </button>
                  <button
                    v-if="piece.custom && latestCustomVersion(piece.custom.id, piece.custom.version)"
                    class="piece-test-button"
                    @click="piece.custom && updatePinnedVersion(piece.id, piece.custom.id)"
                  >
                    최신 버전으로 업데이트
                  </button>
                  <button class="piece-test-button" @click="emitTestPiece(piece.id)">테스트</button>
                </div>
              </div>
            </section>
          </div>
        </section>
      </div>

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
        <p v-if="placementError" class="error">{{ placementError }}</p>
        <div class="placement-zone-list">
          <section
            v-for="zone in placementZoneSections"
            :key="zone.id"
            class="placement-zone-chunk"
            :class="`placement-zone-${zone.id}`"
          >
            <div class="placement-zone-header">
              <strong>{{ zone.label }}</strong>
              <span>{{ zone.description }}</span>
            </div>
            <div class="placement-board" :style="{ '--board-size': deck.boardSize }">
              <button
                v-for="square in zone.squares"
                :key="`${square.file}_${square.rank}`"
                class="placement-square"
                :class="squareClass(square.file, square.rank)"
                :title="squareRestriction(square.file, square.rank) ?? undefined"
                :disabled="deck.ruleset === 'standard' && !squareZone(square.file, square.rank) && !(placementTool === eraseTool && pieceAt(square.file, square.rank))"
                @click="onPlacementSquareClick(square.file, square.rank)"
                @dragover.prevent="onPlacementDragOver"
                @drop.prevent="onPlacementDrop($event, square.file, square.rank)"
              >
                <span class="square-label">{{ fileLabel(square.file) }}{{ square.rank + 1 }}</span>
                <span v-if="deck.ruleset === 'standard'" class="setup-zone-label">{{ squareZone(square.file, square.rank) === 'front' ? 'F' : squareZone(square.file, square.rank) === 'back' ? 'B' : '—' }}</span>
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
        <button
          type="button"
          class="pocket-drop-zone"
          :class="{ ready: pocketTargetPiece && canUseInPocket(pocketTargetPiece, deck.ruleset) }"
          :aria-disabled="!pocketTargetPiece || !canUseInPocket(pocketTargetPiece, deck.ruleset)"
          @click="onPocketClick"
          @dragover.prevent="onPocketDragOver"
          @drop.prevent="onPocketDrop($event)"
        >
          <span>{{ pocketDropMessage }}</span>
        </button>
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
            <button class="pocket-add-button" aria-label="포켓 기물 추가" :disabled="!canUseInPocket(piece.id, deck.ruleset)" @click="changePocketCount(piece.id, 1)">+</button>
          </div>
        </div>
        <div v-if="deckSummary.errors.length > 0" class="validation-list">
          <p v-for="message in deckSummary.errors" :key="message">{{ message }}</p>
        </div>
      </section>
    </div>

    <div v-if="importDialogOpen" class="deck-code-modal-backdrop" @click.self="closeImportDialog">
      <section class="card deck-code-modal" role="dialog" aria-modal="true" aria-labelledby="deck-code-dialog-title">
        <div class="section-header">
          <p class="section-kicker">Deck Code</p>
          <h2 id="deck-code-dialog-title">덱 코드 불러오기</h2>
        </div>

        <template v-if="!importCandidate">
          <label class="deck-code-input-label" for="deck-code-input">
            공유받은 덱 코드를 입력하세요 (DC1~DC4).
          </label>
          <textarea
            id="deck-code-input"
            v-model="importCode"
            class="text-input deck-code-input"
            rows="6"
            maxlength="65536"
            placeholder="DC4.… 또는 기존 DC1~DC3 코드"
            autofocus
          ></textarea>
          <p v-if="importError" class="error" role="alert">{{ importError }}</p>
          <div class="deck-code-modal-actions">
            <button class="btn-secondary" @click="closeImportDialog">취소</button>
            <button class="btn-start" :disabled="!importCode.trim()" @click="prepareImport">불러오기</button>
          </div>
        </template>

        <template v-else>
          <p>덱 코드의 구조를 검증했습니다. 게임 시작 시에는 플레이 가능 여부를 다시 검사합니다. 적용하기 전 내용을 확인해 주세요.</p>
          <dl class="deck-code-preview">
            <div><dt>룰</dt><dd>{{ importCandidate.deck.ruleset }}</dd></div>
            <div><dt>전용 맵</dt><dd>{{ importCandidate.deck.mapId }}</dd></div>
            <div><dt>Extra</dt><dd>{{ importCandidate.deck.extra?.length ?? 0 }}</dd></div>
            <div><dt>보드 크기</dt><dd>{{ importCandidate.deck.boardSize }} x {{ importCandidate.deck.boardSize }}</dd></div>
            <div><dt>점수</dt><dd>{{ importCandidate.totalScore }} / {{ importCandidate.scoreLimit }}</dd></div>
            <div><dt>시작 기물</dt><dd>{{ importCandidate.deck.starting.length }}</dd></div>
            <div><dt>포켓 기물</dt><dd>{{ totalPocketCount(importCandidate.deck) }}</dd></div>
          </dl>
          <div class="deck-code-modal-actions">
            <button class="btn-secondary" @click="importCandidate = null">뒤로</button>
            <button class="btn-start" @click="applyImportedDeck">현재 덱에 적용</button>
          </div>
        </template>
      </section>
    </div>
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { deckRulesets, parseDeckRuleset } from '../deckRulesets'
import { pieceAsset } from '../pieceAssets'
import { customPieceApi } from '../api/customPieceApi'
import type { DeckPieceType, PieceCatalogItem, SavedDeck } from '../types/deck'
import {
  baseZoneRanks,
  deploymentZoneAtSquare,
  canUseInPocket,
  canUseInMain,
  canUseInExtra,
  normalizeExtra,
  MAX_EXTRA_DECK_PIECES,
  createPresetDeck,
  deckPresets,
  emptyPocket,
  frontmostBaseRank,
  findPieceCatalogItem,
  isUniqueStartingPiece,
  pieceCatalog,
  pieceLabel,
  pieceScore,
  placementRestriction,
  presetLayoutForBoard,
  scoreLimit,
  totalPocketCount,
  validateSavedDeck,
  validateDeckForStorage,
  replaceCustomPieceCatalog,
} from '../composables/useDeckValidation'
import { createNewSavedDeck, deckStorageIdentity, useSavedDecks } from '../composables/useSavedDecks'
import { encodeDeckCode } from '../composables/useDeckCodeCodec'
import { importDeckCode, type DeckCodeImportResult } from '../composables/useDeckCode'
import { boardMaps, findBoardMap } from '../boardMaps'

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
const arsenalPieceSearch = ref('')
const customPieceSearch = ref('')
const placementTool = ref<DeckPieceType>('king')
const draggedPiece = ref<DeckPieceType | null>(null)
const saveError = ref<string | null>(null)
const saveNotice = ref<string | null>(null)
const placementError = ref<string | null>(null)
const deck = ref<SavedDeck>(createNewSavedDeck())
watch(deck, () => { saveNotice.value = null }, { deep: true, flush: 'sync' })
const deckLoaded = ref(false)
const loadError = ref<string | null>(null)
const saving = savedDecks.busy
let loadRevision = 0
const catalogLoadError = ref<string | null>(null)
const catalogRevision = ref(0)
const latestCustomPieces = ref<Awaited<ReturnType<typeof customPieceApi.list>>['items']>([])
const deckCodeNotice = ref<string | null>(null)
const deckCodeNoticeIsError = ref(false)
const importDialogOpen = ref(false)
const importCode = ref('')
const importError = ref<string | null>(null)
const importCandidate = ref<Extract<DeckCodeImportResult, { ok: true }> | null>(null)

async function loadDeck() {
  const revision = ++loadRevision
  deckLoaded.value = false
  loadError.value = null
  catalogLoadError.value = null
  saveError.value = null
  saveNotice.value = null
  if (deckStorageIdentity.value === undefined) {
    loadError.value = savedDecks.error.value
    return
  }
  try {
    const existing = props.deckId ? await savedDecks.getDeck(props.deckId) : createNewSavedDeck()
    if (!existing) throw new Error('덱을 찾을 수 없습니다.')
    if (revision !== loadRevision) return
    deck.value = cloneSavedDeck(existing)
    deckLoaded.value = true
    const { items } = await customPieceApi.list()
    const pinned = await Promise.all(
      (existing.customPieces ?? [])
        .filter(reference => !items.some(item => item.id === reference.id && item.version === reference.version))
        .map(reference => customPieceApi.getVersion(reference.id, reference.version).catch(() => null)),
    )
    if (revision !== loadRevision) return
    latestCustomPieces.value = items
    replaceCustomPieceCatalog([...items, ...pinned.filter(item => item !== null)])
    catalogRevision.value += 1
  } catch (cause) {
    if (revision === loadRevision) {
      const message = cause instanceof Error ? cause.message : String(cause)
      if (deckLoaded.value) catalogLoadError.value = message
      else loadError.value = message
    }
  }
}
watch([deckStorageIdentity, () => props.deckId], loadDeck, { immediate: true, flush: 'sync' })
watch(savedDecks.error, message => { if (deckStorageIdentity.value === undefined) loadError.value = message })

function cloneSavedDeck(source: SavedDeck): SavedDeck {
  return {
    ...(source.version === undefined ? {} : { version: source.version }),
    ruleset: parseDeckRuleset(source.ruleset),
    id: source.id,
    name: source.name,
    mapId: source.mapId,
    boardSize: source.boardSize,
    starting: source.starting.map(piece => ({
      pieceType: piece.pieceType,
      square: {
        file: piece.square.file,
        rank: piece.square.rank,
      },
    })),
    pocket: { ...source.pocket },
    extra: normalizeExtra(source.extra),
    createdAt: source.createdAt,
    updatedAt: source.updatedAt,
    customPieces: [...(source.customPieces ?? [])],
  }
}

function changeMap() {
  const map = findBoardMap(deck.value.mapId)
  if (!map || map.boardSize === deck.value.boardSize) return
  deck.value.boardSize = map.boardSize
  resetToClassic()
}

const deckSummary = computed(() => {
  catalogRevision.value
  return validateSavedDeck(deck.value)
})
const storageSummary = computed(() => {
  catalogRevision.value
  return validateDeckForStorage(deck.value)
})
const canSaveDeck = computed(() => storageSummary.value.valid)
const activePresets = computed(() => deck.value.ruleset === 'standard'
  ? [{ id: 'classic', name: 'Standard 기본 배치', description: '중앙 King 1기와 Front 전 칸의 Pawn, 빈 포켓으로 시작합니다.' }]
  : deckPresets.filter(preset => presetLayoutForBoard(preset, deck.value.boardSize)))
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
const extraPieces = computed(() => deck.value.extra ?? [])
const extraCatalog = computed(() => pieceCatalog.filter(piece => canUseInExtra(piece.id, deck.value.ruleset)))
function addExtra(pieceType: DeckPieceType) {
  if (!canUseInExtra(pieceType, deck.value.ruleset) || extraPieces.value.length >= MAX_EXTRA_DECK_PIECES) return
  deck.value.extra = [...extraPieces.value, pieceType]
}
function removeExtra(index: number) {
  deck.value.extra = extraPieces.value.filter((_, position) => position !== index)
}

const maxPocketCount = computed(() => Math.max(1, ...activePocketCatalog.value.map(piece => deck.value.pocket[piece.id] ?? 0)))
const pocketTargetPiece = computed(() => draggedPiece.value ?? (placementTool.value === eraseTool ? null : placementTool.value))
const pocketDropMessage = computed(() => {
  const pieceType = pocketTargetPiece.value
  if (!pieceType) return '기물을 선택한 뒤 여기를 누르거나, 드래그해서 포켓에 추가'
  if (!canUseInPocket(pieceType, deck.value.ruleset)) return `${pieceLabel(pieceType)}은 포켓에 넣을 수 없습니다.`
  return draggedPiece.value
    ? `${pieceLabel(pieceType)} 포켓에 추가`
    : `여기를 눌러 ${pieceLabel(pieceType)} 포켓에 추가`
})
function matchesPieceSearch(piece: (typeof pieceCatalog)[number], search: string): boolean {
  const query = search.toLowerCase()
  return !query || [piece.id, piece.name, piece.category, ...(piece.aliases ?? [])].join(' ').toLowerCase().includes(query)
}

function catalogSectionsFor(pieces: typeof pieceCatalog) {
  return (['front', 'back'] as const)
    .map(id => ({
      id,
      label: id === 'front' ? '앞줄 배치 기물' : '그 외 배치 기물',
      description: deck.value.ruleset === 'standard'
        ? (id === 'front' ? '중앙 Back을 둘러싼 Front 구역 전용' : '중앙 Back 구역 전용')
        : (id === 'front' ? '상대와 가까운 시작 줄 전용' : '나머지 시작 배치 줄 전용'),
      pieces: pieces.filter(piece => piece.deploymentZone === id && canUseInMain(piece.id, deck.value.ruleset)),
    }))
    .filter(section => section.pieces.length > 0)
}

const filteredArsenalCatalog = computed(() => {
  catalogRevision.value
  return pieceCatalog.filter(piece => !piece.custom && matchesPieceSearch(piece, arsenalPieceSearch.value))
})
const filteredCustomCatalog = computed(() => {
  catalogRevision.value
  return pieceCatalog.filter(piece => piece.custom && matchesPieceSearch(piece, customPieceSearch.value))
})
const arsenalCatalogSections = computed(() => catalogSectionsFor(filteredArsenalCatalog.value))
const customCatalogSections = computed(() => catalogSectionsFor(filteredCustomCatalog.value))
const placementZoneSections = computed(() => {
  if (deck.value.ruleset === 'standard') {
    return [{
      id: 'base', label: 'Standard Base / Home',
      description: 'F: Front 전용 · B: Back 전용 · 회색: 진영 밖',
      squares: baseZoneRanks(deck.value.boardSize, 'white', deck.value.ruleset).reverse()
        .flatMap(rank => Array.from({ length: deck.value.boardSize }, (_, file) => ({ file, rank }))),
    }]
  }
  const frontRank = frontmostBaseRank(deck.value.boardSize, 'white', deck.value.ruleset)
  const ranks = baseZoneRanks(deck.value.boardSize, 'white', deck.value.ruleset).reverse()
  return (['front', 'back'] as const).map(id => {
    const zoneRanks = ranks.filter(rank => id === 'front' ? rank === frontRank : rank !== frontRank)
    return {
      id,
      label: id === 'front' ? '앞줄' : '뒷줄',
      description: id === 'front'
        ? 'Front 기물 전용 배치 구역'
        : 'Back 기물 전용 배치 구역',
      squares: zoneRanks.flatMap(rank => (
        Array.from({ length: deck.value.boardSize }, (_, file) => ({ file, rank }))
      )),
    }
  })
})

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
  deck.value.extra = (deck.value.extra ?? []).map(type => type === oldPieceType ? nextPieceType : type)
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
    'prime-minister': '총',
    'cannon-rook': 'C',
    'tempest-queen': 'Q',
    'tempest-rook': 'T',
    'tempest-bishop': 'B',
    'tempest-knight': 'N',
    'bouncing-bishop': 'B',
    'bouncing-rook': 'R',
    'bouncing-queen': 'Q',
    'bouncing-pawn': '♙',
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
  const base = createPresetDeck(deck.value.boardSize, 'classic', deck.value.ruleset)
  deck.value.starting = base.starting
  deck.value.pocket = base.pocket
}

function applyPreset(presetId: string) {
  const base = createPresetDeck(deck.value.boardSize, presetId, deck.value.ruleset)
  deck.value.starting = base.starting
  deck.value.pocket = base.pocket
}

function resetDeck() {
  if (!window.confirm('시작 배치, 포켓, Extra Deck 기물을 모두 비우시겠습니까?')) return
  deck.value.extra = []
  deck.value.starting = []
  deck.value.pocket = emptyPocket()
  deck.value.customPieces = []
  placementError.value = null
  saveError.value = null
}

const canCopyDeckCode = computed(() => deck.value.ruleset === 'standard' || (deck.value.extra?.length ?? 0) > 0
  ? validateDeckForStorage(deck.value).valid : deckSummary.value.valid)

async function copyDeckCode() {
  deckCodeNotice.value = null
  if (!canCopyDeckCode.value) {
    deckCodeNoticeIsError.value = true
    deckCodeNotice.value = deckSummary.value.errors[0] ?? '유효한 덱만 공유할 수 있습니다.'
    return
  }
  try {
    if (!navigator.clipboard?.writeText) throw new Error('clipboard unavailable')
    const source = deck.value.ruleset === 'standard' || (deck.value.extra?.length ?? 0) > 0
      ? { ...deck.value, customPieces: collectDeckCustomPieces(true) } : deck.value
    const code = encodeDeckCode(source)
    await navigator.clipboard.writeText(code)
    deckCodeNoticeIsError.value = false
    deckCodeNotice.value = '덱 코드를 복사했습니다.'
  } catch (cause) {
    deckCodeNoticeIsError.value = true
    deckCodeNotice.value = cause instanceof Error && cause.message.startsWith('덱 코드')
      ? cause.message : '클립보드에 복사하지 못했습니다. 브라우저 권한을 확인해 주세요.'
  }
}

function openImportDialog() {
  importCode.value = ''
  importError.value = null
  importCandidate.value = null
  importDialogOpen.value = true
}

function closeImportDialog() {
  importDialogOpen.value = false
  importError.value = null
  importCandidate.value = null
}

function prepareImport() {
  importError.value = null
  importCandidate.value = null
  const result = importDeckCode(importCode.value, deck.value)
  if (!result.ok) {
    importError.value = result.message
    return
  }
  importCandidate.value = result
}

function applyImportedDeck() {
  if (!importCandidate.value) return
  deck.value = cloneSavedDeck(importCandidate.value.deck)
  placementTool.value = 'king'
  placementError.value = null
  saveError.value = null
  closeImportDialog()
  deckCodeNoticeIsError.value = false
  deckCodeNotice.value = '덱 코드를 현재 편집기에 적용했습니다. 저장하기 전까지 기존 저장본은 유지됩니다.'
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

function squareZone(file: number, rank: number) {
  return deploymentZoneAtSquare({ file, rank }, deck.value.boardSize, 'white', deck.value.ruleset)
}

function squareClass(file: number, rank: number): string[] {
  const activePiece = draggedPiece.value ?? (placementTool.value === eraseTool ? null : placementTool.value)
  return [
    (file + rank) % 2 === 1 ? 'light' : 'dark',
    deck.value.ruleset === 'standard' ? `standard-zone-${squareZone(file, rank) ?? 'outside'}` : '',
    pieceAt(file, rank) ? 'occupied' : 'empty',
    draggedPiece.value ? 'drop-ready' : '',
    activePiece && placementRestriction(activePiece, { file, rank }, deck.value.boardSize, 'white', deck.value.ruleset) ? 'restricted' : '',
  ].filter(Boolean)
}

function squareRestriction(file: number, rank: number): string | null {
  const activePiece = draggedPiece.value ?? (placementTool.value === eraseTool ? null : placementTool.value)
  return activePiece ? placementRestriction(activePiece, { file, rank }, deck.value.boardSize, 'white', deck.value.ruleset) : null
}

function placePieceAt(pieceType: DeckPieceType, file: number, rank: number) {
  const restriction = placementRestriction(pieceType, { file, rank }, deck.value.boardSize, 'white', deck.value.ruleset)
  if (restriction) {
    placementError.value = restriction
    return
  }
  placementError.value = null
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

function onPocketClick() {
  if (placementTool.value === eraseTool) return
  changePocketCount(placementTool.value, 1)
}

function onPocketDragOver(event: DragEvent) {
  if (!draggedPiece.value || !event.dataTransfer) return
  event.dataTransfer.dropEffect = canUseInPocket(draggedPiece.value, deck.value.ruleset) ? 'copy' : 'none'
}

function onPocketDrop(event: DragEvent) {
  const pieceType = getDraggedPiece(event)
  draggedPiece.value = null
  if (!pieceType || !canUseInPocket(pieceType, deck.value.ruleset)) return
  changePocketCount(pieceType, 1)
}

function changePocketCount(pieceType: DeckPieceType, delta: number) {
  if (delta > 0 && !canUseInPocket(pieceType, deck.value.ruleset)) return
  deck.value.pocket[pieceType] ??= 0
  deck.value.pocket[pieceType] = Math.max(0, deck.value.pocket[pieceType] + delta)
}

function emitTestPiece(pieceType: DeckPieceType) {
  emit('testPiece', {
    pieceType,
    boardSize: deck.value.boardSize,
  })
}

function collectDeckCustomPieces(preservePinned = false): SavedDeck['customPieces'] {
  const usedTypes = new Set([
    ...(deck.value.extra ?? []),
    ...deck.value.starting.map(piece => piece.pieceType),
    ...Object.entries(deck.value.pocket).filter(([, count]) => count > 0).map(([pieceType]) => pieceType),
  ])
  return [...usedTypes]
    .map(findPieceCatalogItem)
    .filter((piece): piece is PieceCatalogItem => Boolean(piece?.custom))
    .map(piece => (preservePinned ? deck.value.customPieces.find(ref =>
      ref.id === piece.custom!.id && ref.version === piece.custom!.version && ref.exposedPieceKey === piece.custom!.exposedPieceKey) : undefined) ?? ({
      id: piece.custom!.id,
      version: piece.custom!.version,
      contentHash: piece.custom!.contentHash,
      exposedPieceKey: piece.custom!.exposedPieceKey,
    }))
}

async function save() {
  if (!deckLoaded.value || saving.value) return
  saveError.value = null
  saveNotice.value = null
  if (!storageSummary.value.valid) {
    saveError.value = storageSummary.value.errors.join(' ')
    return
  }
  try {
    deck.value.customPieces = collectDeckCustomPieces()
    const revision = loadRevision
    const saved = await savedDecks.saveDeck(cloneSavedDeck(deck.value))
    if (revision !== loadRevision) return
    // Keep edits made during the request, but use the acknowledged version for the next save.
    if (saved.version === undefined) delete deck.value.version
    else deck.value.version = saved.version
    deck.value.updatedAt = saved.updatedAt
    saveNotice.value = '덱을 저장했습니다.'
    emit('saved')
  } catch (e: unknown) {
    saveError.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<style scoped>
.extra-deck-panel { margin: 16px 0; padding: 20px; }
.extra-deck-panel p { color: #aab6c8; }
.extra-slots, .extra-choices { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 12px; }
.extra-slot { display: flex; align-items: center; gap: 10px; padding: 12px; border: 1px solid #657690; border-radius: 8px; }
.extra-slot img { width: 40px; height: 40px; object-fit: contain; }
.extra-slot.empty { min-width: 130px; color: #aab6c8; border-style: dashed; }
.placement-square.standard-zone-outside { background: #363b45; color: #a3a9b3; }
.placement-square.standard-zone-back:not(.restricted) { box-shadow: inset 0 0 0 3px #547bb5; }
.setup-zone-label { position: absolute; top: 3px; right: 5px; font-size: 11px; font-weight: 700; }
</style>
