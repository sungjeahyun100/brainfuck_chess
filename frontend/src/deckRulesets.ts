/** Game rules, independent of board size, terrain, map IDs and GameMode. */
export type DeckRuleset = 'legacy' | 'standard'

export const deckRulesets: readonly { id: DeckRuleset; label: string }[] = [
  { id: 'legacy', label: 'Legacy' },
  { id: 'standard', label: 'Standard' },
]

export function isDeckRuleset(value: unknown): value is DeckRuleset {
  return value === 'legacy' || value === 'standard'
}

/** Only an absent field defaults to Legacy; null/unknown values are errors. */
export function parseDeckRuleset(value: unknown): DeckRuleset {
  if (value === undefined) return 'legacy'
  if (isDeckRuleset(value)) return value
  throw new Error('지원하지 않는 덱 룰입니다.')
}
