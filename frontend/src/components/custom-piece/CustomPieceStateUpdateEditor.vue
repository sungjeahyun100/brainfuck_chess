<template>
  <details class="rule-editor">
    <summary>{{ title }} <span class="cp-optional">{{ updates.length ? `${updates.length}개` : '선택' }}</span></summary>
    <div class="rule-list">
      <div v-for="(update, index) in updates" :key="index" class="rule-row">
        <input v-model="update.key" placeholder="바꿀 상태 이름" />
        <CustomPieceStateValueInput :value="update.value" />
        <button type="button" class="btn-secondary danger" @click="updates.splice(index, 1)">삭제</button>
      </div>
      <button type="button" class="btn-secondary" @click="updates.push(newStateUpdate())">+ 변경 추가</button>
    </div>
  </details>
</template>

<script setup lang="ts">
import { newStateUpdate, type StateUpdateEditor } from '../../composables/customPieceAbilityBuilder'
import CustomPieceStateValueInput from './CustomPieceStateValueInput.vue'

defineProps<{ title: string; updates: StateUpdateEditor[] }>()
</script>

<style scoped>
.rule-editor { background: rgba(255,255,255,.02); }
.rule-list { display: grid; gap: 10px; margin-top: 12px; }
.rule-row { display: grid; grid-template-columns: 1fr 1.3fr auto; gap: 8px; align-items: end; }
@media (max-width: 900px) { .rule-row { grid-template-columns: 1fr; } }
</style>
