import { parseDeckRuleset } from './deckRulesets.ts'

// Semantic engine versions, independent of the record JSON and Deck Code versions.
export const LEGACY_RULES_VERSION = 'deck-chess-1'
export const STANDARD_RULES_VERSION = 'deck-chess-standard-1'
export type RecordRulesVersionError = 'unsupported_development_standard_record' | 'unsupported_rules_version'

export function recordRulesVersionError(ruleset: unknown, version: unknown): RecordRulesVersionError | null {
  let parsed
  try { parsed = parseDeckRuleset(ruleset) } catch { return 'unsupported_rules_version' }
  if (parsed === 'legacy' && version === LEGACY_RULES_VERSION) return null
  if (parsed === 'standard' && version === STANDARD_RULES_VERSION) return null
  return parsed === 'standard' && version === LEGACY_RULES_VERSION
    ? 'unsupported_development_standard_record' : 'unsupported_rules_version'
}

export function assertRecordRulesVersion(record: { initial_state: { ruleset?: unknown }; ruleset_version: unknown }): void {
  const error = recordRulesVersionError(record.initial_state.ruleset, record.ruleset_version)
  if (error) throw new Error(error)
}
