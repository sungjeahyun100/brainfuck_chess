<template>
  <section class="cp-card cp-package ability-builder">
    <div class="cp-section-heading">
      <div>
        <h3>특수 능력 구성</h3>
        <p class="cp-muted">상태와 행마를 카드로 조합하면 저장할 때 엔진용 JSON 정의로 자동 변환됩니다.</p>
      </div>
      <button type="button" class="btn-secondary" @click="emit('request-simple')">간단 편집 시도</button>
    </div>

    <div class="cp-fields">
      <label>대표 기물 키
        <input v-model="draft.exposedPieceKey" placeholder="main" @change="reloadFromDraft" />
        <small class="cp-muted">내부 기물이 여러 개일 때 덱에 표시할 기물입니다.</small>
      </label>
    </div>

    <div>
      <h4>빠른 시작</h4>
      <div class="cp-ability-options">
        <button type="button" class="btn-secondary" @click="applyTemplate('windmill')">교대형 · 윈드밀</button>
        <button type="button" class="btn-secondary" @click="applyTemplate('cannon-rook')">선택형 · 캐논 룩</button>
        <button type="button" class="btn-secondary" @click="applyTemplate('bouncing-bishop')">선택형 · 바운싱 비숍</button>
      </div>
    </div>

    <div v-if="unsupportedReason" class="cp-banner">
      <strong>카드 편집기로 완전히 표현할 수 없는 정의입니다.</strong>
      <p>{{ unsupportedReason }}</p>
      <button type="button" class="btn-secondary" @click="expertMode = true">JSON 전문가 모드로 열기</button>
    </div>

    <template v-if="session && !expertMode">
      <section class="builder-section">
        <div class="cp-section-heading">
          <div><h4>기본 표시와 일반 이동</h4><p class="cp-muted">기본 이미지와 일반 이동 버튼의 설명입니다.</p></div>
        </div>
        <div class="cp-fields">
          <label>기본 이미지 키<input v-model="session.model.defaultAssetKey" placeholder="bishop" /></label>
          <label>일반 이동 이름<input v-model="session.model.normalOptionName" placeholder="일반 이동" /></label>
          <label class="cp-wide">일반 이동 설명<input v-model="session.model.normalOptionDescription" placeholder="이 기물의 기본 이동입니다." /></label>
        </div>
      </section>

      <section class="builder-section">
        <div class="cp-section-heading">
          <div><h4>기물이 기억할 값</h4><p class="cp-muted">현재 형태, 방향, 충전 여부처럼 기물마다 따로 저장되는 값입니다.</p></div>
          <button type="button" class="btn-secondary" @click="addState">+ 기억할 값</button>
        </div>
        <p v-if="session.model.states.length === 0" class="cp-muted">상태를 쓰지 않는 기물은 비워 두어도 됩니다.</p>
        <article v-for="(state, index) in session.model.states" :key="index" class="cp-subcard value-card">
          <div class="cp-fields">
            <label>값 이름<input v-model="state.key" placeholder="mode" /></label>
            <label>종류
              <select v-model="state.initialValue.type">
                <option value="text">문자</option><option value="number">숫자</option><option value="boolean">참/거짓</option>
              </select>
            </label>
            <label>처음 값
              <input v-if="state.initialValue.type === 'text'" v-model="state.initialValue.textValue" placeholder="bishop" />
              <input v-else-if="state.initialValue.type === 'number'" v-model.number="state.initialValue.numberValue" type="number" />
              <select v-else v-model="state.initialValue.booleanValue"><option :value="true">참</option><option :value="false">거짓</option></select>
            </label>
          </div>
          <button type="button" class="btn-secondary danger compact" @click="session.model.states.splice(index, 1)">삭제</button>
        </article>
      </section>

      <section class="builder-section">
        <div class="cp-section-heading">
          <div>
            <h4>일반 이동 형태</h4>
            <p class="cp-muted">상태에 따라 활성화되는 형태를 만듭니다. 이동 후 상태를 바꾸면 윈드밀처럼 형태가 교대합니다.</p>
          </div>
          <button type="button" class="btn-secondary" @click="addNormalForm">+ 이동 형태</button>
        </div>

        <article v-for="(form, formIndex) in session.model.normalForms" :key="formIndex" class="cp-subcard behavior-card">
          <div class="cp-section-heading">
            <div><h5>이동 형태 {{ formIndex + 1 }}</h5><small class="cp-muted">예: 비숍 형태, 룩 형태</small></div>
            <button v-if="session.model.normalForms.length > 1" type="button" class="btn-secondary danger" @click="session.model.normalForms.splice(formIndex, 1)">삭제</button>
          </div>
          <div class="cp-fields">
            <label>형태 키<input v-model="form.id" placeholder="bishop-mode" /></label>
            <label>이 형태의 이미지 키 <span class="cp-optional">선택</span><input v-model="form.assetKey" placeholder="rook" /></label>
            <label class="cp-wide">행마 코드<textarea v-model="form.movementCode" rows="8" spellcheck="false" /></label>
          </div>
          <CustomPieceStateConditionEditor title="이 형태가 활성화되는 조건" :conditions="form.enabledWhen" />
          <CustomPieceStateUpdateEditor title="이동을 마친 뒤 바꿀 값" :updates="form.onCommit" />
        </article>
      </section>

      <section class="builder-section">
        <div class="cp-section-heading">
          <div>
            <h4>선택해서 사용하는 특수 능력</h4>
            <p class="cp-muted">캐논 룩의 포 이동이나 바운싱 비숍의 반사 이동처럼 별도 버튼으로 고르는 행마입니다.</p>
          </div>
          <button type="button" class="btn-secondary" @click="addAbility">+ 특수 능력</button>
        </div>
        <p v-if="session.model.abilities.length === 0" class="cp-muted">선택형 능력이 없다면 비워 두어도 됩니다.</p>

        <article v-for="(ability, abilityIndex) in session.model.abilities" :key="abilityIndex" class="cp-subcard behavior-card">
          <div class="cp-section-heading">
            <div><h5>{{ ability.name || `특수 능력 ${abilityIndex + 1}` }}</h5><small class="cp-muted">게임 중 이동 옵션으로 표시됩니다.</small></div>
            <button type="button" class="btn-secondary danger" @click="session.model.abilities.splice(abilityIndex, 1)">삭제</button>
          </div>
          <div class="cp-fields">
            <label>능력 키<input v-model="ability.id" placeholder="bounce-move" /></label>
            <label>능력 이름<input v-model="ability.name" placeholder="반사 이동" /></label>
            <label class="cp-wide">설명<input v-model="ability.description" placeholder="가장자리에서 반사되는 이동입니다." /></label>
            <label class="cp-wide">능력 행마 코드<textarea v-model="ability.movementCode" rows="8" spellcheck="false" /></label>
          </div>

          <div class="ability-settings">
            <label class="cp-check"><input v-model="ability.cooldownEnabled" type="checkbox" /> 사용 후 쿨다운</label>
            <template v-if="ability.cooldownEnabled">
              <label>대기 턴<input v-model.number="ability.cooldownTurns" type="number" min="1" /></label>
              <label>턴 계산 기준
                <select v-model="ability.cooldownClock"><option value="owner_turns">소유자의 턴</option><option value="global_turns">양쪽 전체 턴</option></select>
              </label>
            </template>
            <label class="cp-check"><input v-model="ability.contributesToAttackMap" type="checkbox" /> 공격 범위에 포함</label>
            <label>실행 방식
              <select v-model="ability.executionMode"><option value="move_modifier">기물을 이동</option><option value="standalone_action">제자리 행동</option></select>
            </label>
          </div>

          <CustomPieceStateConditionEditor title="이 능력을 사용할 수 있는 상태 조건" :conditions="ability.enabledWhen" />
          <CustomPieceStateUpdateEditor title="능력 사용 뒤 바꿀 값" :updates="ability.onCommit" />
        </article>
      </section>

      <details class="json-preview">
        <summary>자동 생성된 JSON 보기</summary>
        <p class="cp-muted">저장과 서버 전송에는 아래 형식이 사용됩니다. 일반적으로 직접 수정할 필요가 없습니다.</p>
        <textarea :value="draft.rawScript" rows="20" readonly spellcheck="false" />
        <button type="button" class="btn-secondary" @click="openExpertMode">JSON 전문가 모드</button>
      </details>
    </template>

    <section v-if="expertMode" class="expert-editor">
      <div class="cp-section-heading">
        <div><h4>JSON 전문가 모드</h4><p class="cp-muted">카드 편집기가 지원하지 않는 구조를 보존하거나 직접 수정할 때만 사용합니다.</p></div>
        <button type="button" class="btn-secondary" @click="tryReturnToCards">카드 편집으로 돌아가기</button>
      </div>
      <textarea v-model="expertText" rows="30" spellcheck="false" />
      <button type="button" class="btn-start" @click="applyExpertJson">JSON 적용</button>
    </section>
  </section>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import {
  loadAbilityBuilder,
  newNormalForm,
  newSelectableAbility,
  newStateVariable,
  serializeAbilityBuilder,
  type AbilityBuilderSession,
} from '../../composables/customPieceAbilityBuilder'
import {
  customPieceTemplate,
  serializeCustomPiecePackage,
  type AdvancedTemplateKind,
} from '../../composables/useCustomPieceDraft'
import type { AdvancedCustomPieceDraft } from '../../types/customPiece'
import CustomPieceStateConditionEditor from './CustomPieceStateConditionEditor.vue'
import CustomPieceStateUpdateEditor from './CustomPieceStateUpdateEditor.vue'

const props = defineProps<{ draft: AdvancedCustomPieceDraft }>()
const emit = defineEmits<{ 'request-simple': [] }>()
const session = ref<AbilityBuilderSession | null>(null)
const unsupportedReason = ref('')
const expertMode = ref(false)
const expertText = ref('')
const lastSerialized = ref('')

watch(() => props.draft.rawScript, rawScript => {
  if (rawScript === lastSerialized.value) return
  loadRawScript(rawScript)
}, { immediate: true })

watch(session, current => {
  if (!current || expertMode.value) return
  try {
    const serialized = serializeAbilityBuilder(current)
    lastSerialized.value = serialized
    props.draft.rawScript = serialized
    unsupportedReason.value = ''
  } catch (error) {
    unsupportedReason.value = error instanceof Error ? error.message : '기물 정의를 변환하지 못했습니다.'
  }
}, { deep: true })

function loadRawScript(rawScript: string) {
  const loaded = loadAbilityBuilder(rawScript, props.draft.exposedPieceKey)
  session.value = loaded.session
  unsupportedReason.value = loaded.unsupportedReason
  expertText.value = rawScript
  expertMode.value = !loaded.session
}

function reloadFromDraft() { loadRawScript(props.draft.rawScript) }
function addState() { session.value?.model.states.push(newStateVariable(session.value.model.states.length)) }
function addNormalForm() { session.value?.model.normalForms.push(newNormalForm(session.value.model.normalForms.length)) }
function addAbility() { session.value?.model.abilities.push(newSelectableAbility(session.value.model.abilities.length)) }

function applyTemplate(kind: AdvancedTemplateKind) {
  if (!window.confirm('현재 특수 능력 구성을 선택한 예제로 교체하시겠습니까?')) return
  props.draft.exposedPieceKey = 'main'
  const rawScript = serializeCustomPiecePackage(customPieceTemplate(kind))
  props.draft.rawScript = rawScript
  lastSerialized.value = ''
  expertMode.value = false
  loadRawScript(rawScript)
}

function openExpertMode() { expertText.value = props.draft.rawScript; expertMode.value = true }
function applyExpertJson() { props.draft.rawScript = expertText.value; lastSerialized.value = ''; loadRawScript(expertText.value) }

async function tryReturnToCards() {
  const loaded = loadAbilityBuilder(expertText.value, props.draft.exposedPieceKey)
  if (!loaded.session) { unsupportedReason.value = loaded.unsupportedReason; return }
  session.value = loaded.session
  unsupportedReason.value = ''
  expertMode.value = false
  await nextTick()
  props.draft.rawScript = serializeAbilityBuilder(loaded.session)
}
</script>

<style scoped>
.ability-builder { gap: 22px; }
.builder-section { display: grid; gap: 14px; padding-top: 4px; }
.builder-section + .builder-section { border-top: 1px solid var(--line); padding-top: 20px; }
.value-card { grid-template-columns: 1fr auto; align-items: end; }
.behavior-card { gap: 14px; }
.compact { align-self: end; }
.ability-settings { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); align-items: end; gap: 12px; }
.ability-settings label:not(.cp-check) { display: grid; gap: 6px; }
.json-preview textarea, .expert-editor textarea { width: 100%; font: 13px/1.5 ui-monospace, SFMono-Regular, Consolas, monospace; }
.expert-editor { display: grid; gap: 14px; border-top: 1px solid var(--line); padding-top: 18px; }
@media (max-width: 900px) { .ability-settings { grid-template-columns: 1fr 1fr; } }
</style>
