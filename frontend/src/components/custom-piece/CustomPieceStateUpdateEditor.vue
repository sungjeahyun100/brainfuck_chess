<template>
  <details class="rule-editor">
    <summary>{{ title }} <span class="cp-optional">{{ updates.length ? `${updates.length}개` : '선택' }}</span></summary>
    <div class="rule-list">
      <p v-if="states.length === 0" class="cp-muted">먼저 “기물이 기억할 값”을 하나 이상 추가해 주세요.</p>
      <div v-for="(update, index) in updates" :key="index" class="rule-row">
        <select v-model="update.key" @change="matchValueType(update)">
          <option value="">상태 선택</option>
          <option v-for="state in states" :key="state.key" :value="state.key">{{ state.key }}</option>
        </select>
        <CustomPieceStateValueInput :value="update.value" />
        <button type="button" class="btn-secondary danger" @click="updates.splice(index, 1)">삭제</button>
      </div>
      <button type="button" class="btn-secondary" :disabled="states.length === 0" @click="addUpdate">+ 변경 추가</button>
    </div>
  </details>
</template>

<script setup lang="ts">
import {
  newStateUpdate,
  type StateUpdateEditor,
  type StateVariableEditor,
} from '../../composables/customPieceAbilityBuilder'
import CustomPieceStateValueInput from './CustomPieceStateValueInput.vue'

const props = defineProps<{ title: string; updates: StateUpdateEditor[]; states: StateVariableEditor[] }>()

function addUpdate() {
  const update = newStateUpdate()
  const first = props.states[0]
  if (first) {
    update.key = first.key
    update.value.type = first.initialValue.type
  }
  props.updates.push(update)
}

function matchValueType(update: StateUpdateEditor) {
  const state = props.states.find(candidate => candidate.key === update.key)
  if (state) update.value.type = state.initialValue.type
}
</script>

<style scoped>
.rule-editor { background: rgba(255,255,255,.02); }
.rule-list { display: grid; gap: 10px; margin-top: 12px; }
.rule-row { display: grid; grid-template-columns: 1fr 1.3fr auto; gap: 8px; align-items: end; }
@media (max-width: 900px) { .rule-row { grid-template-columns: 1fr; } }
</style>
