import { ref } from 'vue'
import type { Square } from '../types/game'

export function useSelection() {
  const selectedPieceId = ref<string | null>(null)
  const selectedPocketPieceId = ref<string | null>(null)
  const abilityMode = ref(false)
  const activeAbilityId = ref<string | null>(null)

  const legalTargetSquares = ref<Square[]>([])
  const movableSquares = ref<Square[]>([])
  const attackSquares = ref<Square[]>([])
  const dropSquares = ref<Square[]>([])

  function clearSelection() {
    selectedPieceId.value = null
    selectedPocketPieceId.value = null
    abilityMode.value = false
    activeAbilityId.value = null
    clearTargets()
  }

  function clearTargets() {
    legalTargetSquares.value = []
    movableSquares.value = []
    attackSquares.value = []
    dropSquares.value = []
  }

  return {
    selectedPieceId,
    selectedPocketPieceId,
    abilityMode,
    activeAbilityId,
    legalTargetSquares,
    movableSquares,
    attackSquares,
    dropSquares,
    clearSelection,
    clearTargets,
  }
}
