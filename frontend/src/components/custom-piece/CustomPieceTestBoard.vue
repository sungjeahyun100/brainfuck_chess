<template>
  <section class="cp-card">
    <div class="cp-row cp-between">
      <div>
        <h3>서버 테스트 보드</h3>
        <p class="cp-muted">배치는 로컬 초안이며, 행마와 행동 적용은 서버 엔진 결과만 사용합니다.</p>
      </div>
      <div class="cp-actions">
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

    <div class="cp-board" :style="{ '--board-size': boardSize }" aria-label="커스텀 기물 테스트 보드">
      <button
        v-for="square in squares"
        :key="`${square.file}-${square.rank}`"
        type="button"
        class="cp-square"
        :class="{
          selected: pieceAt(square)?.id === selectedPieceId,
          legal: isLegal(square),
          attacked: isAttacked(square),
        }"
        :aria-label="`${square.file + 1}, ${square.rank + 1}`"
        @click="selectSquare(square)"
      >
        <span v-if="pieceAt(square)" class="cp-board-piece">
          {{ pieceAt(square)!.owner === 'white' ? '○' : '●' }}{{ shortKey(pieceAt(square)!.piece_key) }}
        </span>
      </button>
    </div>
    <p class="cp-muted">가능 행동은 테두리, 공격 범위는 점선으로 표시됩니다. 가능한 행동 조회만으로 보드 상태는 바뀌지 않습니다.</p>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { customPieceApi } from '../../api/customPieceApi'
import { testPiecesFromServerState } from '../../composables/useCustomPieceDraft'
import type {
  CustomPieceInput,
  CustomPieceTestBoard,
  CustomPieceTestPiece,
  CustomPieceTestResult,
} from '../../types/customPiece'
import type { PlayerId, Square, TurnAction } from '../../types/game'

const props = defineProps<{
  draft: CustomPieceInput
  pieceKeys: string[]
  enabled: boolean
}>()

const builtIns = ['pawn', 'rook', 'bishop', 'knight', 'queen', 'king']
const boardSize = ref(8)
const currentPlayer = ref<PlayerId>('white')
const placementKey = ref('')
const placementOwner = ref<PlayerId>('white')
const pieces = ref<CustomPieceTestPiece[]>([])
const selectedPieceId = ref<string | null>(null)
const result = ref<CustomPieceTestResult | null>(null)
const status = ref('')
const error = ref('')
let nextId = 1

const squares = computed(() => Array.from({ length: boardSize.value ** 2 }, (_, index) => ({
  file: index % boardSize.value,
  rank: Math.floor(index / boardSize.value),
})))
const selectedServerPiece = computed(() =>
  selectedPieceId.value ? result.value?.state.pieces[selectedPieceId.value] : undefined,
)

watch(() => props.pieceKeys, (keys) => {
  if (!placementKey.value || (!keys.includes(placementKey.value) && !builtIns.includes(placementKey.value))) {
    placementKey.value = keys[0] ?? 'knight'
  }
}, { immediate: true })

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

function isAttacked(square: Square) {
  return Boolean(result.value?.attacks.some(attack => sameSquare(attack, square)))
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
  pieces.value.push({
    id: `test-${nextId++}`,
    piece_key: placementKey.value,
    owner: placementOwner.value,
    square: { ...square },
  })
  result.value = null
  status.value = `${placementKey.value} 배치됨`
}

function board(): CustomPieceTestBoard {
  return {
    board_size: boardSize.value,
    current_player: currentPlayer.value,
    pieces: pieces.value,
  }
}

async function loadOptions(pieceId: string) {
  if (!props.enabled) {
    error.value = '현재 코드 검증을 먼저 완료하세요.'
    return
  }
  status.value = '서버에서 가능한 행동을 계산하는 중…'
  try {
    result.value = await customPieceApi.testOptions(props.draft, board(), pieceId)
    status.value = `가능한 이동 ${result.value.legal_moves.length}개 · 공격 칸 ${result.value.attacks.length}개`
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '행마 계산에 실패했습니다.'
    status.value = ''
  }
}

async function applyAction(action: TurnAction) {
  status.value = '서버에서 행동을 적용하는 중…'
  try {
    const previousCount = pieces.value.length
    const previousPiece = pieces.value.find(piece => piece.id === action.piece_id)
    const applied = await customPieceApi.testAction(props.draft, board(), action)
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
    error.value = caught instanceof Error ? caught.message : '행동 적용에 실패했습니다.'
    status.value = ''
  }
}

function reset() {
  pieces.value = []
  selectedPieceId.value = null
  result.value = null
  error.value = ''
  status.value = '보드를 초기화했습니다.'
  nextId = 1
}

function clearSelection() {
  selectedPieceId.value = null
  result.value = null
  error.value = ''
  status.value = '선택을 해제했습니다.'
}

function shortKey(key: string) {
  return key.slice(0, 3)
}
</script>
