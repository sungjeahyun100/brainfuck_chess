<template>
  <details class="rule-editor">
    <summary>{{ title }} <span class="cp-optional">{{ conditions.length ? `${conditions.length}개` : '선택' }}</span></summary>
    <div class="rule-list">
      <p v-if="states.length === 0" class="cp-muted">먼저 “기물이 기억할 값”을 하나 이상 추가해 주세요.</p>
      <div v-for="(condition, index) in conditions" :key="index" class="rule-row">
        <select v-model="condition.key" @change="matchValueType(condition)">
          <option value="">상태 선택</option>
          <option v-for="state in states" :key="state.key" :value="state.key">{{ state.key }}</option>
        </select>
        <select v-model="condition.operator">
          <option value="equals">값이 같을 때</option>
          <option value="not_equals">값이 다를 때</option>
        </select>
        <CustomPieceStateValueInput :value="condition.expectedValue" />
        <button type="button" class="btn-secondary danger" @click="conditions.splice(index, 1)">삭제</button>
      </div>
      <button type="button" class="btn-secondary" :disabled="states.length === 0" @click="addCondition">+ 조건 추가</button>
    </div>
  </details>
</template>

<script setup lang="ts">
import {
  newStateCondition,
  type StateConditionEditor,
  type StateVariableEditor,
} from '../../composables/customPieceAbilityBuilder'
import CustomPieceStateValueInput from './CustomPieceStateValueInput.vue'

const props = defineProps<{ title: string; conditions: StateConditionEditor[]; states: StateVariableEditor[] }>()

function addCondition() {
  const condition = newStateCondition()
  const first = props.states[0]
  if (first) {
    condition.key = first.key
    condition.expectedValue.type = first.initialValue.type
  }
  props.conditions.push(condition)
}

function matchValueType(condition: StateConditionEditor) {
  const state = props.states.find(candidate => candidate.key === condition.key)
  if (state) condition.expectedValue.type = state.initialValue.type
}
</script>

<style scoped>
.rule-editor { background: rgba(255,255,255,.02); }
.rule-list { display: grid; gap: 10px; margin-top: 12px; }
.rule-row { display: grid; grid-template-columns: 1fr 1fr 1.3fr auto; gap: 8px; align-items: end; }
@media (max-width: 900px) { .rule-row { grid-template-columns: 1fr; } }
</style>
