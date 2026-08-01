<template>
  <section class="cp-card">
    <div class="cp-row cp-between">
      <div>
        <h3>서버 테스트 보드</h3>
        <p class="cp-muted">배치는 로컬 초안이며, 행마와 행동 적용은 서버 엔진 결과만 사용합니다.</p>
      </div>
      <div class="cp-actions">
        <button type="button" class="btn-secondary" @click="flipBoard">보드 돌리기</button>
        <button type="button" class="btn-secondary" @click="clearSelection">선택 해제</button>
        <button type="button" class="btn-secondary" @click="reset">초기화</button>
      </div>
    </div>

    <div class="cp-test-controls">
      <label>보드 크기
        <select v-model.number="boardSize" @change="reset"><option v-for="size in [8, 10, 12]" :key="size">{{ size }}</option></select>
      </label>
      <label>현재 차례
        <select v-model="currentPlayer"><option value="white">백</option><option value="black">흑</option></select>
      </label>
      <label>배치 기물
        <select v-model="placementKey">
          <optgroup label="커스텀 정의">
            <option v-for="key in pieceKeys" :key="key" :value="key">{{ key }}</option>
          </optgroup>
          <optgroup label="공식 기물">
            <option v-for="key in builtIns" :key="key" :value="key">{{ key }}</option>
          </optgroup>
        </select>
      </label>
      <label>진영
        <select v-model="placementOwner"><option value="white">백</option><option value="black">흑</option></select>
      </label>
    </div>

    <p v-if="status" class="cp-status" aria-live="polite">{{ status }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <dl v-if="selectedServerPiece" class="cp-test-state">
      <div><dt>서버 기물 타입</dt><dd>{{ selectedServerPiece.type_id }}</dd></div>
      <div><dt>현재 상태</dt><dd><code>{{ JSON.stringify(selectedServerPiece.state) }}</code></dd></div>
      <div><dt>현재 차례</dt><dd>{{ result?.state.current_player === 'white' ? '백' : '흑' }}</dd></div>
    </dl>

    <PlayBoard
      :board="displayBoard"
      :pieces="displayPieces"
      :definitions="displayDefinitions"
      :selected-piece-id="selectedPieceId"
      :movable-squares="movableSquares"
      :attack-squares="attackSquares"
      :drop-squares="dropSquares"
      :orientation="orientation"
      show-coordinates
      @square-click="selectSquare"
      @piece-drag-start="selectPiece"
      @square-drop="dropPiece"
    />
    <p class="cp-muted">실제 플레이 보드와 같은 표시를 사용합니다. 가능한 행동 조회만으로 보드 상태는 바뀌지 않습니다.</p>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { customPieceApi } from '../../api/customPieceApi'
import PlayBoard from '../Board.vue'
import { parseCustomPiecePackage, testPiecesFromServerState } from '../../composables/useCustomPieceDraft'
import type {
  CustomPieceInput,
  CustomPieceTestBoard,
  CustomPieceTestPiece,
  CustomPieceTestResult,
} from '../../types/customPiece'
import type {
  Board,
  Piece,
  PieceDefinition,
  PieceStateValue,
  PlayerId,
  Square,
  TurnAction,
} from '../../types/game'

const props = defineProps<{
  draft: CustomPieceInput
  pieceKeys: string[]
}>()

const builtIns = ['pawn', 'rook', 'bishop', 'knight', 'queen', 'king']
const boardSize = ref(8)
const currentPlayer = ref<PlayerId>('white')
const orientation = ref<PlayerId>('white')
const placementKey = ref('')
const placementOwner = ref<PlayerId>('white')
const pieces = ref<CustomPieceTestPiece[]>([])
const selectedPieceId = ref<string | null>(null)
const result = ref<CustomPieceTestResult | null>(null)
const status = ref('')
const error = ref('')
let nextId = 1
let pendingOptions: Promise<boolean> | null = null
let pendingOptionsPieceId: string | null = null
let requestRevision = 0
let draftRefreshTimer: ReturnType<typeof setTimeout> | null = null

const selectedServerPiece = computed(() =>
  selectedPieceId.value ? result.value?.state.pieces[selectedPieceId.value] : undefined,
)
const displayBoard = computed<Board>(() => {
  if (result.value) return result.value.state.board
  const squares: Record<string, string | null> = {}
  for (let rank = 0; rank < boardSize.value; rank += 1) {
    for (let file = 0; file < boardSize.value; file += 1) {
      squares[`${file}_${rank}`] = null
    }
  }
  for (const piece of pieces.value) squares[`${piece.square.file}_${piece.square.rank}`] = piece.id
  return { size: boardSize.value, squares }
})
const displayPieces = computed<Record<string, Piece>>(() => {
  if (result.value) return result.value.state.pieces
  return Object.fromEntries(pieces.value.map(piece => [piece.id, {
    id: piece.id,
    owner: piece.owner,
    type_id: piece.piece_key,
    current_square: piece.square,
    in_pocket: false,
    captured: false,
    has_moved: false,
    state: piece.state ?? {},
    move_option_cooldowns: {},
  }]))
})
const displayDefinitions = computed<Record<string, PieceDefinition>>(() => {
  if (result.value) return result.value.state.piece_definitions
  try {
    return Object.fromEntries(parseCustomPiecePackage(props.draft.raw_script).definitions.map(
      definition => [definition.id, definition],
    ))
  } catch {
    return {}
  }
})
const movableSquares = computed(() => result.value?.legal_moves.map(move => move.to) ?? [])
const attackSquares = computed(() => result.value?.attacks ?? [])
const dropSquares = computed(() => result.value?.legal_drops.map(drop => drop.to) ?? [])

watch(() => props.pieceKeys, (keys) => {
  if (!placementKey.value || (!keys.includes(placementKey.value) && !builtIns.includes(placementKey.value))) {
    placementKey.value = keys[0] ?? 'knight'
  }
}, { immediate: true })

watch(() => JSON.stringify(props.draft), () => {
  requestRevision += 1
  pendingOptions = null
  pendingOptionsPieceId = null
  result.value = null
  error.value = ''
  reconcilePieceStates()
  status.value = '편집 중인 기물 변경사항을 테스트 보드에 반영했습니다.'

  if (draftRefreshTimer) clearTimeout(draftRefreshTimer)
  const selected = selectedPieceId.value
  if (selected && pieces.value.some(piece => piece.id === selected)) {
    draftRefreshTimer = setTimeout(() => {
      draftRefreshTimer = null
      void loadOptions(selected)
    }, 250)
  }
})

onBeforeUnmount(() => {
  requestRevision += 1
  if (draftRefreshTimer) clearTimeout(draftRefreshTimer)
})

function draftDefinitions(): Record<string, PieceDefinition> {
  try {
    return Object.fromEntries(parseCustomPiecePackage(props.draft.raw_script).definitions.map(
      definition => [definition.id, definition],
    ))
  } catch {
    return {}
  }
}

function sameStateType(left: PieceStateValue, right: PieceStateValue): boolean {
  return typeof left === typeof right
}

function reconcilePieceStates() {
  const definitions = draftDefinitions()
  pieces.value = pieces.value.map(piece => {
    const definition = definitions[piece.piece_key]
    if (!definition) return piece
    const previous = piece.state ?? {}
    const state = Object.fromEntries(definition.state_schema.map(schema => {
      const current = previous[schema.key]
      return [
        schema.key,
        current !== undefined && sameStateType(current, schema.default_value)
          ? current
          : schema.default_value,
      ]
    }))
    return { ...piece, state }
  })
}

function pieceAt(square: Square) {
  return pieces.value.find(piece => sameSquare(piece.square, square))
}

function sameSquare(left: Square, right: Square) {
  return left.file === right.file && left.rank === right.rank
}

function isLegal(square: Square) {
  return Boolean(result.value?.legal_moves.some(move => sameSquare(move.to, square))
    || result.value?.legal_drops.some(drop => sameSquare(drop.to, square)))
}

async function selectSquare(square: Square) {
  error.value = ''
  const occupant = pieceAt(square)
  if (selectedPieceId.value && isLegal(square)) {
    const action = result.value?.legal_moves.find(move => sameSquare(move.to, square))
      ?? result.value?.legal_drops.find(drop => sameSquare(drop.to, square))
    if (action) await applyAction(action)
    return
  }
  if (selectedPieceId.value && !occupant) {
    error.value = '서버가 반환한 가능한 행동에 포함되지 않은 칸입니다.'
    return
  }
  if (occupant) {
    selectedPieceId.value = occupant.id
    await loadOptions(occupant.id)
    return
  }
  if (!placementKey.value) return
  const definition = draftDefinitions()[placementKey.value]
  pieces.value.push({
    id: `test-${nextId++}`,
    piece_key: placementKey.value,
    owner: placementOwner.value,
    square: { ...square },
    state: definition
      ? Object.fromEntries(definition.state_schema.map(schema => [schema.key, schema.default_value]))
      : {},
  })
  result.value = null
  status.value = `${placementKey.value} 배치됨`
}

async function selectPiece(pieceId: string) {
  const piece = pieces.value.find(candidate => candidate.id === pieceId)
  if (!piece) return
  selectedPieceId.value = pieceId
  await loadOptions(pieceId)
}

async function dropPiece(square: Square | null, pieceId: string) {
  if (!square) return
  selectedPieceId.value = pieceId
  if (!isLegal(square) && !await loadOptions(pieceId)) return
  if (!isLegal(square)) {
    error.value = '선택한 기물이 행마법상 이동할 수 없는 칸입니다.'
    return
  }
  await selectSquare(square)
}

function board(): CustomPieceTestBoard {
  return {
    board_size: boardSize.value,
    current_player: currentPlayer.value,
    pieces: pieces.value,
  }
}

function loadOptions(pieceId: string): Promise<boolean> {
  if (pendingOptions && pendingOptionsPieceId === pieceId) return pendingOptions

  const revision = requestRevision
  pendingOptionsPieceId = pieceId
  const request = (async () => {
    status.value = '서버에서 가능한 행동을 계산하는 중…'
    try {
      const nextResult = await customPieceApi.testOptions(props.draft, board(), pieceId)
      if (revision !== requestRevision) return false
      result.value = nextResult
      status.value = `가능한 이동 ${nextResult.legal_moves.length}개 · 공격 칸 ${nextResult.attacks.length}개`
      return true
    } catch (caught) {
      if (revision !== requestRevision) return false
      error.value = caught instanceof Error ? caught.message : '행마 계산에 실패했습니다.'
      status.value = ''
      return false
    }
  })()
  pendingOptions = request
  void request.finally(() => {
    if (pendingOptions === request) {
      pendingOptions = null
      pendingOptionsPieceId = null
    }
  })
  return request
}

async function applyAction(action: TurnAction) {
  const revision = requestRevision
  status.value = '서버에서 행동을 적용하는 중…'
  try {
    const previousCount = pieces.value.length
    const previousPiece = pieces.value.find(piece => piece.id === action.piece_id)
    const applied = await customPieceApi.testAction(props.draft, board(), action)
    if (revision !== requestRevision) return
    result.value = applied
    currentPlayer.value = applied.state.current_player
    pieces.value = testPiecesFromServerState(applied.state)
    selectedPieceId.value = action.piece_id
    const nextPiece = pieces.value.find(piece => piece.id === action.piece_id)
    const changes = [
      pieces.value.length < previousCount ? '포획 발생' : '',
      previousPiece && nextPiece && previousPiece.piece_key !== nextPiece.piece_key
        ? `타입 전환: ${previousPiece.piece_key} → ${nextPiece.piece_key}`
        : '',
    ].filter(Boolean)
    status.value = `행동 적용 완료${changes.length ? ` · ${changes.join(' · ')}` : ''}. 서버 상태로 갱신했습니다.`
  } catch (caught) {
    if (revision !== requestRevision) return
    error.value = caught instanceof Error ? caught.message : '행동 적용에 실패했습니다.'
    status.value = ''
  }
}

function reset() {
  requestRevision += 1
  pieces.value = []
  selectedPieceId.value = null
  result.value = null
  error.value = ''
  status.value = '보드를 초기화했습니다.'
  nextId = 1
}

function clearSelection() {
  requestRevision += 1
  selectedPieceId.value = null
  result.value = null
  error.value = ''
  status.value = '선택을 해제했습니다.'
}

function flipBoard() {
  orientation.value = orientation.value === 'white' ? 'black' : 'white'
}

</script>
