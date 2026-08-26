import type { TimeControlId } from './types/game'

export interface TimeControlOption {
  id: TimeControlId
  label: string
  initialTimeMs: number | null
  incrementMs: number
  mode: 'countdown' | 'unlimited'
}

export const TIME_CONTROLS: readonly TimeControlOption[] = [
  { id: 'five_zero', label: '5 + 0', initialTimeMs: 300_000, incrementMs: 0, mode: 'countdown' },
  { id: 'ten_zero', label: '10 + 0', initialTimeMs: 600_000, incrementMs: 0, mode: 'countdown' },
  { id: 'five_three', label: '5 + 3', initialTimeMs: 300_000, incrementMs: 3_000, mode: 'countdown' },
  { id: 'ten_five', label: '10 + 5', initialTimeMs: 600_000, incrementMs: 5_000, mode: 'countdown' },
  { id: 'fifteen_ten', label: '15 + 10', initialTimeMs: 900_000, incrementMs: 10_000, mode: 'countdown' },
  { id: 'unlimited', label: '무제한', initialTimeMs: null, incrementMs: 0, mode: 'unlimited' },
] as const

export const CLOCK_URGENCY_THRESHOLDS_MS = { low: 60_000, critical: 10_000 } as const

export function timeControlLabel(id: TimeControlId): string {
  return TIME_CONTROLS.find(option => option.id === id)?.label ?? id
}
