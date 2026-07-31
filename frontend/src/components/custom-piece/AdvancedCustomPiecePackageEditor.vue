<template>
  <section class="cp-card cp-package">
    <div class="cp-section-heading">
      <div>
        <h3>고급 기물 정의</h3>
        <p class="cp-muted">
          기존 패키지 형식을 그대로 편집합니다. 상태, 이동 레이어, 선택형 능력,
          쿨다운과 내부 기물 정의를 자유롭게 조합할 수 있습니다.
        </p>
      </div>
      <button type="button" class="btn-secondary" @click="emit('request-simple')">
        간단 편집 시도
      </button>
    </div>

    <div class="cp-fields">
      <label>대표 기물 키
        <input v-model="draft.exposedPieceKey" placeholder="main" />
        <small class="cp-muted">definitions 안에서 덱에 노출할 기물의 id입니다.</small>
      </label>
    </div>

    <div>
      <h4>예제 불러오기</h4>
      <div class="cp-ability-options">
        <button type="button" class="btn-secondary" @click="applyTemplate('windmill')">윈드밀</button>
        <button type="button" class="btn-secondary" @click="applyTemplate('cannon-rook')">캐논 룩</button>
        <button type="button" class="btn-secondary" @click="applyTemplate('bouncing-bishop')">바운싱 비숍</button>
      </div>
      <small class="cp-muted">현재 고급 정의 전체를 예제로 교체합니다.</small>
    </div>

    <label>기물 패키지 JSON
      <textarea
        v-model="draft.rawScript"
        rows="28"
        spellcheck="false"
        aria-label="고급 기물 패키지 JSON"
      />
    </label>

    <details open>
      <summary>핵심 필드 설명</summary>
      <div class="cp-help-grid cp-advanced-help">
        <p><code>state_schema</code><span>기물 인스턴스가 기억할 값과 초기값입니다.</span></p>
        <p><code>move_layers</code><span>서로 독립적인 체섬블리 행마 프로그램입니다.</span></p>
        <p><code>enabled_when</code><span>특정 상태에서만 레이어를 활성화합니다.</span></p>
        <p><code>on_commit</code><span>그 레이어로 실제 이동한 뒤 상태를 변경합니다.</span></p>
        <p><code>move_options</code><span>일반 이동과 선택형 능력 버튼을 구성합니다.</span></p>
        <p><code>cooldown</code><span>능력 사용 뒤 소유자 턴 또는 전체 턴 기준 대기 시간을 둡니다.</span></p>
        <p><code>definitions</code><span>transition 등에 쓰이는 내부 기물을 함께 선언할 수 있습니다.</span></p>
        <p><code>visual.variants</code><span>상태에 따른 논리 이미지 키를 지정합니다.</span></p>
      </div>
      <p class="cp-muted">
        왕 지정과 착수 포획 같은 권한 필드는 저장 시 비활성화되며, 이번 고급 편집 범위에 포함되지 않습니다.
      </p>
    </details>
  </section>
</template>

<script setup lang="ts">
import {
  customPieceTemplate,
  serializeCustomPiecePackage,
  type AdvancedTemplateKind,
} from '../../composables/useCustomPieceDraft'
import type { AdvancedCustomPieceDraft } from '../../types/customPiece'

const props = defineProps<{ draft: AdvancedCustomPieceDraft }>()
const emit = defineEmits<{ 'request-simple': [] }>()

function applyTemplate(kind: AdvancedTemplateKind) {
  if (!window.confirm('현재 고급 기물 정의를 선택한 예제로 교체하시겠습니까?')) return
  props.draft.rawScript = serializeCustomPiecePackage(customPieceTemplate(kind))
  props.draft.exposedPieceKey = 'main'
}
</script>
