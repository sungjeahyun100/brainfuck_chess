<template>
  <details class="rule-editor">
    <summary>{{ title }} <span class="cp-optional">{{ conditions.length ? `${conditions.length}개` : '선택' }}</span></summary>
    <div class="rule-list">
      <div v-for="(condition, index) in conditions" :key="index" class="rule-row">
        <input v-model="condition.key" placeholder="상태 이름 (예: mode)" />
        <select v-model="condition.operator">
          <option value="equals">값이 같을 때</option>
          <option value="not_equals">값이 다를 때</option>
        </select>
        <CustomPieceStateValueInput :value="condition.expectedValue" />
        <button type="button" class="btn-secondary danger" @click="conditions.splice(index, 1)">삭제</button>
      </div>
      <button type="button" class="btn-secondary" @click="conditions.push(newStateCondition())">+ 조건 추가</button>
    </div>
  </details>
</template>

<script setup lang="ts">
import { newStateCondition, type StateConditionEditor } from '../../composables/customPieceAbilityBuilder'
import CustomPieceStateValueInput from './CustomPieceStateValueInput.vue'

defineProps<{ title: string; conditions: StateConditionEditor[] }>()
</script>

<style scoped>
.rule-editor { background: rgba(255,255,255,.02); }
.rule-list { display: grid; gap: 10px; margin-top: 12px; }
.rule-row { display: grid; grid-template-columns: 1fr 1fr 1.3fr auto; gap: 8px; align-items: end; }
@media (max-width: 900px) { .rule-row { grid-template-columns: 1fr; } }
</style>
