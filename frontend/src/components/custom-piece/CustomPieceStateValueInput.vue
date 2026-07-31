<template>
  <div class="inline-value-editor">
    <select v-model="value.type" aria-label="값 종류">
      <option value="text">문자</option>
      <option value="number">숫자</option>
      <option value="boolean">참/거짓</option>
    </select>
    <input v-if="value.type === 'text'" v-model="value.textValue" placeholder="값" />
    <input v-else-if="value.type === 'number'" v-model.number="value.numberValue" type="number" />
    <select v-else v-model="value.booleanValue" aria-label="참 또는 거짓">
      <option :value="true">참</option>
      <option :value="false">거짓</option>
    </select>
  </div>
</template>

<script setup lang="ts">
import type { EditableValue } from '../../composables/customPieceAbilityBuilder'

defineProps<{ value: EditableValue }>()
</script>

<style scoped>
.inline-value-editor { display: grid; grid-template-columns: .8fr 1.2fr; gap: 6px; }
@media (max-width: 900px) { .inline-value-editor { grid-template-columns: 1fr 1fr; } }
</style>
