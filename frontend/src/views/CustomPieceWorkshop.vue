<template>
  <main class="lobby cp-workshop">
    <header class="page-bar">
      <button class="btn-secondary" type="button" @click="goBack">← 로비</button>
      <div><p class="eyebrow">Chessembly Workshop</p><h1>커스텀 기물 제작소</h1></div>
      <button v-if="mode === 'library'" class="btn-start" type="button" @click="newPiece">새 기물</button>
    </header>

    <section v-if="mode === 'library'" class="cp-card">
      <div class="cp-row cp-between">
        <div><h2>내 커스텀 기물</h2><p class="cp-muted">상태, 행마 레이어와 선택형 능력을 포함한 기물을 만들 수 있습니다.</p></div>
        <button class="btn-secondary" type="button" :disabled="loading" @click="loadList">다시 불러오기</button>
      </div>
      <p v-if="loading" class="cp-status">목록을 불러오는 중…</p>
      <div v-else-if="listError" class="cp-empty" role="alert"><p class="error">{{ listError }}</p><button class="btn-secondary" @click="loadList">재시도</button></div>
      <div v-else-if="items.length === 0" class="cp-empty"><h3>아직 만든 기물이 없습니다</h3><p>첫 체섬블리 기물을 만들어 보세요.</p><button class="btn-start" @click="newPiece">새 기물 만들기</button></div>
      <div v-else class="cp-list">
        <article v-for="piece in items" :key="piece.id" class="cp-list-item">
          <img v-if="imageUrl(piece.image)" class="cp-thumb" :src="imageUrl(piece.image)" alt="" />
          <div class="cp-list-main"><h3>{{ piece.name }}</h3><p>{{ piece.score }}점 · 수정 {{ formatDate(piece.updated_at) }}</p><span class="cp-valid">✓ 서버 검증 완료 · v{{ piece.version }}</span></div>
          <div class="cp-actions"><button class="btn-secondary" @click="editPiece(piece.id)">수정</button><button class="btn-secondary" @click="duplicatePiece(piece)">복제</button><button class="btn-secondary danger" @click="removePiece(piece)">삭제</button></div>
        </article>
      </div>
    </section>

    <template v-else>
      <div class="cp-editor-heading">
        <button class="btn-secondary" type="button" @click="closeEditor">← 목록</button>
        <div><h2>{{ editingId ? '기물 편집' : duplicateSource ? '기물 복제' : '새 기물' }}</h2><p v-if="dirty" class="cp-stale">저장되지 않은 변경이 있습니다.</p></div>
        <button class="btn-start" type="button" :disabled="saving" @click="save">{{ saving ? '저장 중…' : '저장' }}</button>
      </div>

      <p v-if="editorError" class="error cp-banner" role="alert">{{ editorError }}</p>
      <div class="cp-editor-modes" role="tablist">
        <button type="button" :class="{ active: editorKind === 'simple' }" @click="requestSimpleMode">간단 편집</button>
        <button type="button" :class="{ active: editorKind === 'advanced' }" @click="requestAdvancedMode">고급 편집</button>
      </div>

      <div class="cp-editor-grid">
        <section class="cp-card cp-form">
          <h3>기본 정보</h3>
          <label>기물 이름<input v-model="activeDraft.name" maxlength="80" required /></label>
          <label>기물 설명 <span class="cp-optional">선택</span><textarea v-model="activeDraft.description" maxlength="2000" rows="3" /></label>
          <label>점수 (1–30)<input v-model.number="activeDraft.score" type="number" min="1" max="30" required /></label>
          <fieldset><legend>기물 이미지</legend><div class="cp-image-options"><button v-for="asset in builtInAssets" :key="asset" type="button" :class="{ active: isBuiltIn(asset) }" @click="selectBuiltIn(asset)"><img :src="pieceAsset(asset, 'white')" :alt="asset" /><span>{{ asset }}</span></button></div></fieldset>
          <label>이미지 업로드<input type="file" accept=".svg,.png,.jpg,.jpeg,image/svg+xml,image/png,image/jpeg" :disabled="uploading" @change="uploadFile" /></label>
          <p v-if="uploading" class="cp-status">서버에서 이미지를 검사하는 중…</p>
          <div class="cp-preview"><img v-if="previewUrl" :src="previewUrl" alt="선택한 기물 이미지" /><span v-else>업로드 이미지 저장됨</span><small>보드 크기 미리보기</small></div>
        </section>

        <section class="cp-card cp-validation-card">
          <div class="cp-row cp-between"><div><h3>검증</h3><p class="cp-muted">저장 전 서버에서 문법과 참조를 확인합니다.</p></div><button class="btn-start" type="button" :disabled="validating" @click="validateCode">{{ validating ? '검증 중…' : '서버 검증' }}</button></div>
          <p v-if="validation && validationCurrent" class="cp-valid">✓ 현재 정의가 검증되었습니다 · {{ validationTime }}</p>
          <p v-else-if="validation" class="cp-stale">정의가 변경되어 이전 검증 결과가 오래되었습니다.</p>
          <div v-if="validation" class="cp-validation">
            <h4>발견된 정의</h4><ul><li v-for="key in definitionKeys" :key="key"><strong>{{ key }}</strong><span v-if="validation.internal_piece_keys.includes(key)"> — 내부 기물</span></li></ul>
            <div v-for="diagnostic in validation.diagnostics" :key="`${diagnostic.code}-${diagnostic.message}`" class="error"><strong>{{ diagnostic.code }}</strong>: {{ diagnostic.message }}<span v-if="diagnostic.line"> ({{ diagnostic.line }}행 {{ diagnostic.column ?? 1 }}열)</span></div>
          </div>
        </section>
      </div>

      <CustomPiecePackageEditor v-if="editorKind === 'simple'" :draft="simpleDraft" @request-advanced="requestAdvancedMode" />
      <AdvancedCustomPiecePackageEditor v-else :draft="advancedDraft" @request-simple="requestSimpleMode" />
      <CustomPieceTestBoard :draft="engineDraft" :piece-keys="definitionKeys" />
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { customPieceApi, CustomPieceApiError } from '../api/customPieceApi'
import AdvancedCustomPiecePackageEditor from '../components/custom-piece/AdvancedCustomPiecePackageEditor.vue'
import CustomPiecePackageEditor from '../components/custom-piece/CustomPiecePackageEditor.vue'
import CustomPieceTestBoard from '../components/custom-piece/CustomPieceTestBoard.vue'
import { pieceAsset } from '../pieceAssets'
import {
  advancedDraftFromInput, buildAdvancedCustomPieceInput, buildCustomPieceInput,
  customPieceDraftSnapshot, newAdvancedCustomPieceDraft, newSimpleCustomPieceDraft,
  parseCustomPiecePackage, simpleDraftFromInput, validateAdvancedCustomPieceDraft,
  validateCustomPieceDraft,
} from '../composables/useCustomPieceDraft'
import type {
  AdvancedCustomPieceDraft, BuiltInPieceAsset, CustomPieceImage, CustomPieceInput,
  CustomPieceRecord, CustomPieceValidation, SimpleCustomPieceDraft,
} from '../types/customPiece'

const emit = defineEmits<{ back: [] }>()
const builtInAssets: BuiltInPieceAsset[] = ['pawn', 'rook', 'bishop', 'knight', 'queen', 'king']
const mode = ref<'library' | 'editor'>('library')
const editorKind = ref<'simple' | 'advanced'>('simple')
const items = ref<CustomPieceRecord[]>([])
const loading = ref(false), saving = ref(false), validating = ref(false), uploading = ref(false)
const listError = ref(''), editorError = ref(''), validationSnapshot = ref(''), validationTime = ref(''), uploadedPreview = ref('')
const editingId = ref<string | null>(null), expectedVersion = ref<number | null>(null), duplicateSource = ref(false)
const savedSnapshot = ref('')
const validation = ref<CustomPieceValidation | null>(null)
const simpleDraft = reactive<SimpleCustomPieceDraft>(newSimpleCustomPieceDraft())
const advancedDraft = reactive<AdvancedCustomPieceDraft>(newAdvancedCustomPieceDraft())
const activeDraft = computed(() => editorKind.value === 'simple' ? simpleDraft : advancedDraft)
const snapshot = computed(() => customPieceDraftSnapshot({ editorKind: editorKind.value, draft: activeDraft.value }))
const engineDraft = computed<CustomPieceInput>(() => {
  if (editorKind.value === 'simple') return buildCustomPieceInput(simpleDraft)
  try { return buildAdvancedCustomPieceInput(advancedDraft) }
  catch { return { name: advancedDraft.name, description: advancedDraft.description, score: advancedDraft.score, image: { ...advancedDraft.image }, raw_script: advancedDraft.rawScript, exposed_piece_key: advancedDraft.exposedPieceKey } }
})
const dirty = computed(() => mode.value === 'editor' && snapshot.value !== savedSnapshot.value)
const validationCurrent = computed(() => validationSnapshot.value === snapshot.value)
const definitionKeys = computed(() => {
  try {
    const definitions = validation.value?.preview_definitions ?? parseCustomPiecePackage(engineDraft.value.raw_script).definitions
    return definitions.map(definition => definition.id.includes(':') ? definition.id.slice(definition.id.lastIndexOf(':') + 1) : definition.id)
  } catch { return [] }
})
const previewUrl = computed(() => uploadedPreview.value || imageUrl(activeDraft.value.image))
watch(snapshot, () => { editorError.value = '' })
onMounted(() => { window.addEventListener('beforeunload', warnBeforeUnload); void loadList() })
onBeforeUnmount(() => { window.removeEventListener('beforeunload', warnBeforeUnload); revokePreview() })

function warnBeforeUnload(event: BeforeUnloadEvent) { if (dirty.value) { event.preventDefault(); event.returnValue = '' } }
async function loadList() { loading.value = true; listError.value = ''; try { items.value = (await customPieceApi.list()).items.sort((a, b) => b.updated_at - a.updated_at) } catch (e) { listError.value = message(e) } finally { loading.value = false } }
function openDraft(input: CustomPieceInput, id: string | null, version: number | null, duplicated = false) {
  const simple = simpleDraftFromInput(input)
  if (simple) { editorKind.value = 'simple'; Object.assign(simpleDraft, structuredClone(simple)); Object.assign(advancedDraft, newAdvancedCustomPieceDraft(simple)) }
  else { editorKind.value = 'advanced'; Object.assign(advancedDraft, advancedDraftFromInput(input)); Object.assign(simpleDraft, newSimpleCustomPieceDraft()) }
  editingId.value = id; expectedVersion.value = version; duplicateSource.value = duplicated
  validation.value = null; validationSnapshot.value = ''; editorError.value = ''; savedSnapshot.value = duplicated ? '' : snapshot.value; mode.value = 'editor'
}
function newPiece() { const fresh = newSimpleCustomPieceDraft(); Object.assign(simpleDraft, fresh); Object.assign(advancedDraft, newAdvancedCustomPieceDraft(fresh)); openDraft(buildCustomPieceInput(simpleDraft), null, null) }
function duplicatePiece(piece: CustomPieceRecord) { openDraft(toInput(piece), null, null, true) }
async function editPiece(id: string) { try { const piece = await customPieceApi.get(id); openDraft(toInput(piece), piece.id, piece.version) } catch (e) { listError.value = message(e) } }
function requestAdvancedMode() { if (editorKind.value === 'simple') { Object.assign(advancedDraft, advancedDraftFromInput(buildCustomPieceInput(simpleDraft))); editorKind.value = 'advanced'; validation.value = null } }
function requestSimpleMode() {
  if (editorKind.value === 'simple') return
  try {
    const simple = simpleDraftFromInput(buildAdvancedCustomPieceInput(advancedDraft))
    if (!simple) { editorError.value = '현재 정의에는 상태 레이어, 선택형 능력 또는 내부 기물이 있어 간단 편집으로 바꿀 수 없습니다.'; return }
    Object.assign(simpleDraft, simple); editorKind.value = 'simple'; validation.value = null
  } catch (e) { editorError.value = message(e) }
}
async function validateCode() {
  editorError.value = editorKind.value === 'simple' ? validateCustomPieceDraft(simpleDraft) : validateAdvancedCustomPieceDraft(advancedDraft)
  if (editorError.value) return
  validating.value = true
  try { validation.value = await customPieceApi.validate(engineDraft.value); validationSnapshot.value = snapshot.value; validationTime.value = new Date().toLocaleTimeString('ko-KR'); if (!validation.value.valid) editorError.value = '코드 검증 오류를 확인해 주세요.' }
  catch (e) { editorError.value = message(e) } finally { validating.value = false }
}
async function save() {
  if (saving.value) return
  editorError.value = editorKind.value === 'simple' ? validateCustomPieceDraft(simpleDraft) : validateAdvancedCustomPieceDraft(advancedDraft)
  if (editorError.value) return
  saving.value = true
  try {
    const saved = editingId.value && expectedVersion.value ? await customPieceApi.update(editingId.value, engineDraft.value, expectedVersion.value) : await customPieceApi.create(engineDraft.value)
    editingId.value = saved.id; expectedVersion.value = saved.version; duplicateSource.value = false
    const input = toInput(saved), simple = simpleDraftFromInput(input)
    if (simple) { editorKind.value = 'simple'; Object.assign(simpleDraft, simple) } else { editorKind.value = 'advanced'; Object.assign(advancedDraft, advancedDraftFromInput(input)) }
    savedSnapshot.value = snapshot.value; await loadList(); mode.value = 'library'
  } catch (e) { editorError.value = e instanceof CustomPieceApiError && e.kind === 'conflict' ? `${e.message} 현재 편집 내용은 유지됩니다.` : message(e) } finally { saving.value = false }
}
async function removePiece(piece: CustomPieceRecord) { if (!window.confirm(`“${piece.name}”을 삭제하시겠습니까?`)) return; try { await customPieceApi.delete(piece.id, piece.version); await loadList() } catch (e) { listError.value = message(e) } }
async function uploadFile(event: Event) {
  const input = event.target as HTMLInputElement, file = input.files?.[0]; if (!file) return
  if (!['image/svg+xml', 'image/png', 'image/jpeg'].includes(file.type) || file.size > 512 * 1024) { editorError.value = 'SVG, PNG, JPG/JPEG 파일만 사용할 수 있으며 크기는 512KiB 이하여야 합니다.'; input.value = ''; return }
  uploading.value = true
  try { const uploaded = await customPieceApi.uploadImage(file); activeDraft.value.image = { kind: 'uploaded', asset_id: uploaded.asset_id }; revokePreview(); uploadedPreview.value = URL.createObjectURL(file) }
  catch (e) { editorError.value = message(e) } finally { uploading.value = false; input.value = '' }
}
function selectBuiltIn(asset: BuiltInPieceAsset) { activeDraft.value.image = { kind: 'built_in', asset_key: asset }; revokePreview() }
function isBuiltIn(asset: BuiltInPieceAsset) { return activeDraft.value.image.kind === 'built_in' && activeDraft.value.image.asset_key === asset }
function imageUrl(image: CustomPieceImage) { return image.kind === 'built_in' ? pieceAsset(image.asset_key, 'white') : undefined }
function formatDate(seconds: number) { return new Date(seconds * 1000).toLocaleString('ko-KR') }
function toInput(piece: CustomPieceRecord): CustomPieceInput { return { name: piece.name, description: piece.description, score: piece.score, image: piece.image, raw_script: piece.raw_script, exposed_piece_key: piece.exposed_piece_key } }
function message(error: unknown) { return error instanceof Error ? error.message : '요청을 처리하지 못했습니다.' }
function revokePreview() { if (uploadedPreview.value) URL.revokeObjectURL(uploadedPreview.value); uploadedPreview.value = '' }
function closeEditor() { if (!dirty.value || window.confirm('저장되지 않은 변경을 버리시겠습니까?')) mode.value = 'library' }
function goBack() { if (!dirty.value || window.confirm('저장되지 않은 변경을 버리시겠습니까?')) emit('back') }
</script>

<style>
.cp-workshop { max-width: 1400px; }
.cp-card { background: var(--panel); border: 1px solid var(--line); border-radius: 14px; padding: 20px; margin-bottom: 18px; }
.cp-row, .cp-actions, .cp-editor-heading { display: flex; align-items: center; gap: 12px; }
.cp-between, .cp-editor-heading { justify-content: space-between; }
.cp-muted { color: var(--muted); }.cp-status { color: #8ed0ff; }.cp-valid { color: #79d69d; }.cp-stale { color: #ffd37c; }
.cp-empty { padding: 46px 20px; text-align: center; }.cp-list { display: grid; gap: 12px; }
.cp-list-item { display: grid; grid-template-columns: 64px 1fr auto; gap: 16px; align-items: center; padding: 14px; border: 1px solid var(--line); border-radius: 10px; }
.cp-thumb, .cp-preview img { width: 56px; height: 56px; object-fit: contain; }.cp-editor-heading { margin-bottom: 16px; }
.cp-editor-modes { display: flex; gap: 8px; margin-bottom: 16px; }.cp-editor-modes button { border: 1px solid var(--line); border-radius: 999px; padding: 8px 14px; background: #111927; color: var(--text); }.cp-editor-modes button.active { border-color: var(--accent); background: rgba(73,209,125,.12); }
.cp-editor-grid { display: grid; grid-template-columns: minmax(280px,.8fr) minmax(320px,1fr); gap: 18px; }.cp-form,.cp-validation-card,.cp-package { display: flex; flex-direction: column; gap: 14px; }
.cp-form label,.cp-package label { display: grid; gap: 6px; font-weight: 650; }.cp-form input,.cp-form textarea,.cp-package input:not([type=checkbox]),.cp-package textarea { width: 100%; }
.cp-package textarea { font: 14px/1.55 ui-monospace,SFMono-Regular,Consolas,monospace; resize: vertical; }.cp-optional { color: var(--muted); font-size: .8em; }
.cp-ability-options,.cp-check-list { display: flex; flex-wrap: wrap; gap: 10px; }.cp-help-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 8px 16px; }.cp-help-grid p { display: grid; gap: 4px; }
.cp-section-heading { display: flex; justify-content: space-between; align-items: center; gap: 12px; }.cp-fields { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 12px; }.cp-subcard { padding: 14px; border: 1px solid var(--line); border-radius: 8px; }
.cp-image-options { display: grid; grid-template-columns: repeat(3,1fr); gap: 8px; }.cp-image-options button { border: 1px solid var(--line); border-radius: 8px; background: #111927; color: var(--text); padding: 8px; }.cp-image-options button.active { outline: 2px solid var(--accent); }.cp-image-options img { width: 36px; height: 36px; display: block; margin: auto; }
.cp-preview { display: flex; align-items: center; gap: 12px; min-height: 70px; padding: 8px; border: 1px dashed var(--line); }.cp-banner { padding: 12px; border: 1px solid rgba(255,125,125,.4); border-radius: 8px; }.cp-validation { padding: 12px; border-radius: 8px; background: rgba(255,255,255,.04); }
@media (max-width:850px) { .cp-editor-grid { grid-template-columns: 1fr; }.cp-fields { grid-template-columns: 1fr; }.cp-list-item { grid-template-columns: 48px 1fr; }.cp-list-item .cp-actions { grid-column: 1/-1; flex-wrap: wrap; } }
</style>
