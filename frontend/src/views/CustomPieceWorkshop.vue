<template>
  <main class="lobby cp-workshop">
    <header class="page-bar">
      <button class="btn-secondary" type="button" @click="goBack">← 로비</button>
      <div>
        <p class="eyebrow">Chessembly Workshop</p>
        <h1>커스텀 기물 제작소</h1>
      </div>
      <button v-if="mode === 'library'" class="btn-start" type="button" @click="newPiece">새 기물</button>
    </header>

    <section v-if="mode === 'library'" class="cp-card">
      <div class="cp-row cp-between">
        <div><h2>내 커스텀 기물</h2><p class="cp-muted">덱 및 실제 매치 연결은 다음 단계에서 제공됩니다.</p></div>
        <button class="btn-secondary" type="button" :disabled="loading" @click="loadList">다시 불러오기</button>
      </div>
      <p v-if="loading" class="cp-status" aria-live="polite">목록을 불러오는 중…</p>
      <div v-else-if="listError" class="cp-empty" role="alert">
        <p class="error">{{ listError }}</p><button class="btn-secondary" type="button" @click="loadList">재시도</button>
      </div>
      <div v-else-if="items.length === 0" class="cp-empty">
        <h3>아직 만든 기물이 없습니다</h3><p>첫 체섬블리 기물을 만들어 보세요.</p>
        <button class="btn-start" type="button" @click="newPiece">새 기물 만들기</button>
      </div>
      <div v-else class="cp-list">
        <article v-for="piece in items" :key="piece.id" class="cp-list-item">
          <img v-if="imageUrl(piece.image)" class="cp-thumb" :src="imageUrl(piece.image)" alt="" />
          <div class="cp-list-main">
            <h3>{{ piece.name }}</h3>
            <p>{{ piece.score }}점 · 수정 {{ formatDate(piece.updated_at) }}</p>
            <span class="cp-valid">✓ 서버 검증 완료 · v{{ piece.version }}</span>
          </div>
          <div class="cp-actions">
            <button class="btn-secondary" type="button" @click="editPiece(piece.id)">수정</button>
            <button class="btn-secondary" type="button" @click="duplicatePiece(piece)">복제</button>
            <button class="btn-secondary danger" type="button" @click="removePiece(piece)">삭제</button>
          </div>
        </article>
      </div>
    </section>

    <template v-else>
      <div class="cp-editor-heading">
        <button class="btn-secondary" type="button" @click="closeEditor">← 목록</button>
        <div><h2>{{ editingId ? '기물 편집' : duplicateSource ? '기물 복제' : '새 기물' }}</h2><p v-if="dirty" class="cp-stale">저장되지 않은 변경이 있습니다.</p></div>
        <button class="btn-start" type="button" :disabled="saving" @click="save">
          {{ saving ? '저장 중…' : '저장' }}
        </button>
      </div>

      <p v-if="editorError" class="error cp-banner" role="alert">{{ editorError }}</p>
      <div class="cp-editor-grid">
        <section class="cp-card cp-form">
          <h3>기본 정보</h3>
          <label>이름
            <input v-model="draft.name" maxlength="80" required />
            <small class="cp-muted">목록과 덱에 표시되며 대표 기물 정의의 이름에도 자동 반영됩니다.</small>
          </label>
          <label>설명
            <textarea v-model="draft.description" maxlength="2000" rows="3" />
            <small class="cp-muted">기물의 특징과 사용 방법을 설명합니다. 게임 동작에는 영향을 주지 않습니다.</small>
          </label>
          <label>기물 점수 (1–30)
            <input v-model.number="draft.score" type="number" min="1" max="30" required />
            <small class="cp-muted">덱 구성 비용이며 대표 기물 정의의 점수에도 자동 반영됩니다.</small>
          </label>

          <fieldset>
            <legend>기본 제공 이미지</legend>
            <div class="cp-image-options">
              <button v-for="asset in builtInAssets" :key="asset" type="button" :class="{ active: isBuiltIn(asset) }" @click="selectBuiltIn(asset)">
                <img :src="pieceAsset(asset, 'white')" :alt="asset" /><span>{{ asset }}</span>
              </button>
            </div>
          </fieldset>
          <label>이미지 업로드 (SVG, PNG, JPG/JPEG · 최대 512KiB)
            <input type="file" accept=".svg,.png,.jpg,.jpeg,image/svg+xml,image/png,image/jpeg" :disabled="uploading" @change="uploadFile" />
            <small class="cp-muted">기본 이미지 대신 목록과 게임 보드에서 사용할 그림입니다.</small>
          </label>
          <p v-if="uploading" class="cp-status">서버에서 이미지를 검사하는 중…</p>
          <div class="cp-preview">
            <img v-if="previewUrl" :src="previewUrl" alt="선택한 기물 이미지 미리보기" />
            <span v-else>업로드 이미지 저장됨</span>
            <small>보드 크기 미리보기</small>
          </div>
        </section>

        <section class="cp-card cp-validation-card">
          <div class="cp-row cp-between"><div><h3>검증</h3><p class="cp-muted">저장 전 서버에서 문법과 설정 참조를 확인합니다.</p></div><button class="btn-start" type="button" :disabled="validating" @click="validateCode">{{ validating ? '검증 중…' : '서버 검증' }}</button></div>
          <p v-if="validation && validationCurrent" class="cp-valid">✓ 현재 코드가 서버에서 검증되었습니다 · {{ validationTime }}</p>
          <p v-else-if="validation" class="cp-stale">코드 또는 검증 입력이 변경되어 이전 결과가 오래되었습니다.</p>
          <div v-if="validation" class="cp-validation" aria-live="polite">
            <h4>발견된 정의</h4>
            <ul v-if="definitionKeys.length"><li v-for="key in definitionKeys" :key="key"><strong>{{ key }}</strong><span v-if="validation.internal_piece_keys.includes(key)"> — 내부 의존 기물</span></li></ul>
            <p v-else class="cp-muted">발견된 정의가 없습니다.</p>
            <div v-for="diagnostic in validation.diagnostics" :key="`${diagnostic.code}-${diagnostic.message}`" class="error">
              <strong>{{ diagnostic.code }}</strong>: {{ diagnostic.message }}
              <span v-if="diagnostic.line"> ({{ diagnostic.line }}행 {{ diagnostic.column ?? 1 }}열)</span>
              <span v-if="diagnostic.limit_exceeded"> — 실행 제한 초과</span>
            </div>
          </div>
        </section>
      </div>

      <CustomPiecePackageEditor :draft="draft" />

      <CustomPieceTestBoard
        :draft="draft"
        :piece-keys="definitionKeys"
        :enabled="Boolean(validation?.valid && validationCurrent)"
      />
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { customPieceApi, CustomPieceApiError } from '../api/customPieceApi'
import CustomPiecePackageEditor from '../components/custom-piece/CustomPiecePackageEditor.vue'
import CustomPieceTestBoard from '../components/custom-piece/CustomPieceTestBoard.vue'
import { pieceAsset } from '../pieceAssets'
import { customPieceDraftSnapshot, newCustomPieceScript, validateCustomPieceDraft } from '../composables/useCustomPieceDraft'
import type {
  BuiltInPieceAsset,
  CustomPieceImage,
  CustomPieceInput,
  CustomPieceRecord,
  CustomPieceValidation,
} from '../types/customPiece'

const emit = defineEmits<{ back: [] }>()
const builtInAssets: BuiltInPieceAsset[] = ['pawn', 'rook', 'bishop', 'knight', 'queen', 'king']
const emptyDraft = (): CustomPieceInput => ({
  name: '',
  description: '',
  score: 1,
  image: { kind: 'built_in', asset_key: 'knight' },
  raw_script: newCustomPieceScript(),
  exposed_piece_key: 'hero',
})

const mode = ref<'library' | 'editor'>('library')
const items = ref<CustomPieceRecord[]>([])
const loading = ref(false)
const listError = ref('')
const editorError = ref('')
const saving = ref(false)
const validating = ref(false)
const uploading = ref(false)
const editingId = ref<string | null>(null)
const expectedVersion = ref<number | null>(null)
const duplicateSource = ref(false)
const savedSnapshot = ref('')
const validationSnapshot = ref('')
const validation = ref<CustomPieceValidation | null>(null)
const validationTime = ref('')
const uploadedPreview = ref('')
const draft = reactive<CustomPieceInput>(emptyDraft())

const snapshot = computed(() => customPieceDraftSnapshot(draft))
const dirty = computed(() => mode.value === 'editor' && snapshot.value !== savedSnapshot.value)
const validationCurrent = computed(() => validationSnapshot.value === snapshot.value)
const definitionKeys = computed(() => validation.value?.preview_definitions.map(definition =>
  definition.id.includes(':') ? definition.id.slice(definition.id.lastIndexOf(':') + 1) : definition.id,
) ?? [])
const previewUrl = computed(() => uploadedPreview.value || imageUrl(draft.image))

watch(() => draft.raw_script, () => { editorError.value = '' })

onMounted(() => {
  window.addEventListener('beforeunload', warnBeforeUnload)
  void loadList()
})
onBeforeUnmount(() => {
  window.removeEventListener('beforeunload', warnBeforeUnload)
  revokePreview()
})

function warnBeforeUnload(event: BeforeUnloadEvent) {
  if (!dirty.value) return
  event.preventDefault()
  event.returnValue = ''
}

async function loadList() {
  loading.value = true
  listError.value = ''
  try {
    items.value = (await customPieceApi.list()).items.sort((a, b) => b.updated_at - a.updated_at)
  } catch (caught) {
    listError.value = message(caught)
  } finally {
    loading.value = false
  }
}

function openDraft(input: CustomPieceInput, id: string | null, version: number | null, duplicated = false) {
  Object.assign(draft, structuredClone(input))
  editingId.value = id
  expectedVersion.value = version
  duplicateSource.value = duplicated
  validation.value = null
  validationSnapshot.value = ''
  editorError.value = ''
  savedSnapshot.value = duplicated ? '' : JSON.stringify(draft)
  mode.value = 'editor'
}

function newPiece() { openDraft(emptyDraft(), null, null) }
function duplicatePiece(piece: CustomPieceRecord) { openDraft(toInput(piece), null, null, true) }
async function editPiece(id: string) {
  editorError.value = ''
  try {
    const piece = await customPieceApi.get(id)
    openDraft(toInput(piece), piece.id, piece.version)
  } catch (caught) {
    listError.value = message(caught)
  }
}

async function validateCode() {
  editorError.value = validateCustomPieceDraft(draft)
  if (editorError.value) return
  validating.value = true
  try {
    validation.value = await customPieceApi.validate({ ...draft })
    validationSnapshot.value = snapshot.value
    validationTime.value = new Date().toLocaleTimeString('ko-KR')
    if (!validation.value.valid) editorError.value = '코드 검증 오류를 확인해 주세요.'
  } catch (caught) {
    editorError.value = message(caught)
  } finally {
    validating.value = false
  }
}

async function save() {
  if (saving.value) return
  editorError.value = validateCustomPieceDraft(draft)
  if (editorError.value) return
  saving.value = true
  try {
    const saved = editingId.value && expectedVersion.value
      ? await customPieceApi.update(editingId.value, { ...draft }, expectedVersion.value)
      : await customPieceApi.create({ ...draft })
    editingId.value = saved.id
    expectedVersion.value = saved.version
    duplicateSource.value = false
    Object.assign(draft, toInput(saved))
    savedSnapshot.value = JSON.stringify(draft)
    await loadList()
    mode.value = 'library'
  } catch (caught) {
    editorError.value = caught instanceof CustomPieceApiError && caught.kind === 'conflict'
      ? `${caught.message} 현재 편집 내용은 유지됩니다.`
      : message(caught)
  } finally {
    saving.value = false
  }
}

async function removePiece(piece: CustomPieceRecord) {
  if (!window.confirm(`“${piece.name}”을 삭제하시겠습니까? 과거 버전은 서버 복구용으로 보존됩니다.`)) return
  try {
    await customPieceApi.delete(piece.id, piece.version)
    await loadList()
  } catch (caught) {
    listError.value = message(caught)
  }
}

async function uploadFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  if (!['image/svg+xml', 'image/png', 'image/jpeg'].includes(file.type) || file.size > 512 * 1024) {
    editorError.value = 'SVG, PNG, JPG/JPEG 파일만 사용할 수 있으며 크기는 512KiB 이하여야 합니다.'
    input.value = ''
    return
  }
  uploading.value = true
  editorError.value = ''
  try {
    const uploaded = await customPieceApi.uploadImage(file)
    draft.image = { kind: 'uploaded', asset_id: uploaded.asset_id }
    revokePreview()
    uploadedPreview.value = URL.createObjectURL(file)
  } catch (caught) {
    editorError.value = message(caught)
  } finally {
    uploading.value = false
    input.value = ''
  }
}

function selectBuiltIn(asset: BuiltInPieceAsset) {
  draft.image = { kind: 'built_in', asset_key: asset }
  revokePreview()
}
function isBuiltIn(asset: BuiltInPieceAsset) {
  return draft.image.kind === 'built_in' && draft.image.asset_key === asset
}
function imageUrl(image: CustomPieceImage) {
  return image.kind === 'built_in' ? pieceAsset(image.asset_key, 'white') : undefined
}
function formatDate(seconds: number) { return new Date(seconds * 1000).toLocaleString('ko-KR') }
function toInput(piece: CustomPieceRecord): CustomPieceInput {
  return {
    name: piece.name, description: piece.description, score: piece.score,
    image: piece.image, raw_script: piece.raw_script, exposed_piece_key: piece.exposed_piece_key,
  }
}
function message(caught: unknown) { return caught instanceof Error ? caught.message : '요청을 처리하지 못했습니다.' }
function revokePreview() {
  if (uploadedPreview.value) URL.revokeObjectURL(uploadedPreview.value)
  uploadedPreview.value = ''
}
function closeEditor() {
  if (dirty.value && !window.confirm('저장되지 않은 변경을 버리고 목록으로 돌아가시겠습니까?')) return
  mode.value = 'library'
}
function goBack() {
  if (dirty.value && !window.confirm('저장되지 않은 변경을 버리고 로비로 돌아가시겠습니까?')) return
  emit('back')
}
</script>

<style>
.cp-workshop { max-width: 1400px; }
.cp-card { background: var(--panel); border: 1px solid var(--line); border-radius: 14px; padding: 20px; margin-bottom: 18px; }
.cp-row, .cp-actions, .cp-editor-heading { display: flex; align-items: center; gap: 12px; }
.cp-between { justify-content: space-between; }
.cp-muted { color: var(--muted); }
.cp-status { color: #8ed0ff; }
.cp-valid { color: #79d69d; }
.cp-stale { color: #ffd37c; }
.cp-empty { padding: 46px 20px; text-align: center; }
.cp-list { display: grid; gap: 12px; }
.cp-list-item { display: grid; grid-template-columns: 64px 1fr auto; gap: 16px; align-items: center; padding: 14px; border: 1px solid var(--line); border-radius: 10px; }
.cp-thumb, .cp-preview img { width: 56px; height: 56px; object-fit: contain; }
.cp-list-main h3, .cp-list-main p { margin: 3px 0; }
.cp-editor-heading { justify-content: space-between; margin-bottom: 16px; }
.cp-editor-grid { display: grid; grid-template-columns: minmax(280px, .8fr) minmax(320px, 1fr); gap: 18px; }
.cp-form, .cp-validation-card { display: flex; flex-direction: column; gap: 14px; }
.cp-form label, .cp-package label, .cp-test-controls label { display: grid; gap: 6px; font-weight: 650; }
.cp-form input, .cp-form textarea, .cp-package input:not([type="checkbox"]), .cp-package select, .cp-package textarea, .cp-test-controls select { width: 100%; }
.cp-package { display: grid; gap: 16px; }
.cp-package textarea { font: 14px/1.55 ui-monospace, SFMono-Regular, Consolas, monospace; tab-size: 2; resize: vertical; }
.cp-definition-tabs { display: flex; gap: 8px; flex-wrap: wrap; border-bottom: 1px solid var(--line); padding-bottom: 10px; }
.cp-definition-tabs button { border: 1px solid var(--line); border-radius: 999px; padding: 8px 13px; background: #111927; color: var(--text); }
.cp-definition-tabs button.active { border-color: var(--accent); background: rgba(73, 209, 125, .12); }
.cp-definition-form { display: grid; gap: 14px; }
.cp-section-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.cp-section-heading h4, .cp-section-heading h5 { margin: 0; }
.cp-fields { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
.cp-wide { grid-column: 1 / -1; }
.cp-package details { border: 1px solid var(--line); border-radius: 10px; padding: 12px; }
.cp-package summary { cursor: pointer; font-weight: 750; }
.cp-details-body { display: grid; gap: 12px; margin-top: 14px; }
.cp-array-row { display: grid; grid-template-columns: 1fr .7fr 1fr auto; align-items: end; gap: 10px; }
.cp-subcard { display: grid; gap: 12px; padding: 14px; border: 1px solid var(--line); border-radius: 8px; background: rgba(255,255,255,.025); }
.cp-check-list { display: flex; flex-wrap: wrap; gap: 10px 18px; }
.cp-check-list label, .cp-check { display: flex !important; align-items: center; gap: 7px !important; }
.cp-check-list input, .cp-check input { width: auto; }
.cp-image-options { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.cp-image-options button { border: 1px solid var(--line); border-radius: 8px; background: #111927; color: var(--text); padding: 8px; }
.cp-image-options button.active { outline: 2px solid var(--accent); }
.cp-image-options img { width: 36px; height: 36px; display: block; margin: auto; }
.cp-preview { display: flex; align-items: center; gap: 12px; min-height: 70px; padding: 8px; border: 1px dashed var(--line); }
.cp-banner { padding: 12px; border: 1px solid rgba(255,125,125,.4); border-radius: 8px; }
.cp-validation { padding: 12px; border-radius: 8px; background: rgba(255,255,255,.04); }
.cp-test-controls { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin: 14px 0; }
.cp-board { display: grid; grid-template-columns: repeat(var(--board-size), minmax(28px, 1fr)); width: min(100%, 650px); aspect-ratio: 1; border: 2px solid #667085; }
.cp-square { min-width: 0; border: 0; color: #111; background: #d7c6a5; font-size: clamp(8px, 1.5vw, 14px); }
.cp-square:nth-child(2n) { background: #879b78; }
.cp-square.legal { box-shadow: inset 0 0 0 4px #49d17d; }
.cp-square.attacked { outline: 2px dashed #e25454; outline-offset: -5px; }
.cp-square.selected { box-shadow: inset 0 0 0 4px #f7c948; }
.cp-board-piece { font-weight: 800; text-transform: uppercase; }
.cp-test-state { display: flex; flex-wrap: wrap; gap: 10px 24px; padding: 10px 12px; background: rgba(255,255,255,.04); }
.cp-test-state div { display: flex; gap: 8px; }
.cp-test-state dt { color: var(--muted); }
.cp-test-state dd { margin: 0; }
@media (max-width: 850px) {
  .cp-editor-grid { grid-template-columns: 1fr; }
  .cp-fields { grid-template-columns: 1fr 1fr; }
  .cp-array-row { grid-template-columns: 1fr 1fr; }
  .cp-list-item { grid-template-columns: 48px 1fr; }
  .cp-list-item .cp-actions { grid-column: 1 / -1; flex-wrap: wrap; }
  .cp-test-controls { grid-template-columns: repeat(2, 1fr); }
}
</style>
