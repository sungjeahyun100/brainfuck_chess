<template>
  <section class="cp-card cp-package">
    <div class="cp-section-heading">
      <div>
        <h3>움직임</h3>
        <p class="cp-muted">
          체섬블리로 이 기물의 이동과 공격 방식을 작성합니다.
          테스트 보드와 실제 게임은 동일한 방식으로 이 코드를 실행합니다.
        </p>
      </div>
      <button type="button" class="btn-secondary" @click="emit('request-advanced')">
        고급 편집
      </button>
    </div>

    <label>움직임 코드
      <textarea
        v-model="draft.movementCode"
        rows="10"
        spellcheck="false"
        placeholder="move(0, 1);&#10;take(1, 1);&#10;take(-1, 1);"
      />
    </label>

    <details>
      <summary>사용 가능한 명령 보기</summary>
      <div class="cp-help-grid">
        <p><code>move(x, y);</code><span>빈 칸으로 이동합니다.</span></p>
        <p><code>take(x, y);</code><span>상대 기물을 공격합니다.</span></p>
        <p><code>take-move(x, y);</code><span>상대 기물을 잡고 그 칸으로 이동합니다.</span></p>
        <p><code>repeat(n) { ... }</code><span>동작을 정해진 횟수만큼 반복합니다.</span></p>
      </div>
      <small class="cp-muted">지원하지 않는 명령이나 문자는 서버 검증에서 오류로 처리됩니다.</small>
    </details>
  </section>

  <section class="cp-card cp-package">
    <div>
      <h3>특수 능력 <span class="cp-optional">선택</span></h3>
      <p class="cp-muted">간단한 값 기억은 여기서, 상태 전환·선택형 행마·쿨다운은 고급 편집에서 설정합니다.</p>
    </div>

    <div class="cp-ability-options">
      <button type="button" class="btn-secondary" @click="addMemory">방향이나 횟수를 기억</button>
      <button type="button" class="btn-secondary" @click="emit('request-advanced')">이동 후 다른 형태로 전환</button>
      <button type="button" class="btn-secondary" @click="emit('request-advanced')">여러 움직임 중 하나를 선택</button>
      <button type="button" class="btn-secondary" @click="emit('request-advanced')">상태 조건과 쿨다운 설정</button>
    </div>

    <article v-for="(ability, index) in draft.abilities" :key="index" class="cp-subcard">
      <div class="cp-section-heading">
        <div>
          <h4>기물이 값을 기억하게 하기</h4>
          <p class="cp-muted">방향, 충전 횟수, 사용 횟수 등을 기물마다 저장합니다.</p>
        </div>
        <button class="btn-secondary danger" type="button" @click="draft.abilities.splice(index, 1)">삭제</button>
      </div>
      <div class="cp-fields">
        <label>기억할 값 이름
          <input v-model="ability.name" placeholder="예: charge" />
        </label>
        <label>처음 값
          <input v-model.number="ability.initialValue" type="number" />
        </label>
      </div>
    </article>
  </section>
</template>

<script setup lang="ts">
import type { SimpleCustomPieceDraft } from '../../types/customPiece'

const props = defineProps<{ draft: SimpleCustomPieceDraft }>()
const emit = defineEmits<{ 'request-advanced': [] }>()

function addMemory() {
  props.draft.abilities.push({
    kind: 'remember_value',
    name: `memory-${props.draft.abilities.length + 1}`,
    initialValue: 0,
  })
}
</script>
