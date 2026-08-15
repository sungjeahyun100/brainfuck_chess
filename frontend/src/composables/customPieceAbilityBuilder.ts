import type {
  CooldownClock,
  MoveOptionExecutionMode,
  PieceDefinition,
  PieceStateCondition,
  PieceStatePredicate,
  PieceStateUpdateDefinition,
  PieceStateValue,
} from '../types/game'
import {
  parseCustomPiecePackage,
  serializeCustomPiecePackage,
  type CustomPiecePackageDocument,
} from './useCustomPieceDraft.ts'

export type EditableValueType = 'number' | 'boolean' | 'text'

export interface EditableValue {
  type: EditableValueType
  numberValue: number
  booleanValue: boolean
  textValue: string
}

export interface StateVariableEditor {
  key: string
  initialValue: EditableValue
}

export interface StateConditionEditor {
  key: string
  operator: 'equals' | 'not_equals'
  expectedValue: EditableValue
}

export interface StateUpdateEditor {
  key: string
  value: EditableValue
}

export interface MovementFormEditor {
  id: string
  movementCode: string
  enabledWhen: StateConditionEditor[]
  onCommit: StateUpdateEditor[]
  assetKey: string
}

export interface SelectableAbilityEditor extends MovementFormEditor {
  name: string
  description: string
  cooldownEnabled: boolean
  cooldownTurns: number
  cooldownClock: CooldownClock
  contributesToAttackMap: boolean
  executionMode: MoveOptionExecutionMode
}

export interface AbilityBuilderModel {
  defaultAssetKey: string
  normalOptionName: string
  normalOptionDescription: string
  states: StateVariableEditor[]
  normalForms: MovementFormEditor[]
  abilities: SelectableAbilityEditor[]
}

export interface AbilityBuilderSession {
  document: CustomPiecePackageDocument
  exposedPieceKey: string
  model: AbilityBuilderModel
}

export interface AbilityBuilderLoadResult {
  session: AbilityBuilderSession | null
  unsupportedReason: string
}

export function loadAbilityBuilder(rawScript: string, exposedPieceKey: string): AbilityBuilderLoadResult {
  let document: CustomPiecePackageDocument
  try {
    document = parseCustomPiecePackage(rawScript)
  } catch (error) {
    return {
      session: null,
      unsupportedReason: error instanceof Error ? error.message : '기물 패키지를 읽지 못했습니다.',
    }
  }
  const definition = document.definitions.find(candidate => candidate.id === exposedPieceKey)
  if (!definition) {
    return { session: null, unsupportedReason: `대표 기물 \`${exposedPieceKey}\` 정의를 찾을 수 없습니다.` }
  }

  const normalOptions = definition.move_options.filter(option => option.kind === 'normal')
  if (normalOptions.length > 1) {
    return { session: null, unsupportedReason: '일반 이동 옵션이 여러 개인 패키지는 아직 카드 편집기로 표현할 수 없습니다.' }
  }
  const normalOption = normalOptions[0]
  const allLayers = new Map(definition.move_layers.map(layer => [layer.id, layer]))
  const normalLayerIds = normalOption?.layer_ids.length
    ? normalOption.layer_ids
    : definition.move_layers.length
      ? definition.move_layers.map(layer => layer.id)
      : ['normal_move']

  const referencedLayerIds = new Set<string>()
  const normalForms: MovementFormEditor[] = []
  for (const layerId of normalLayerIds) {
    const layer = allLayers.get(layerId)
    if (!layer && definition.move_layers.length) {
      return { session: null, unsupportedReason: `일반 이동이 존재하지 않는 레이어 \`${layerId}\`를 참조합니다.` }
    }
    referencedLayerIds.add(layerId)
    const enabledWhen = layer?.enabled_when ?? []
    normalForms.push({
      id: layerId,
      movementCode: layer?.chessembly_code ?? definition.chessembly_code,
      enabledWhen: enabledWhen.map(conditionFromEngine),
      onCommit: (layer?.on_commit ?? []).map(updateFromEngine),
      assetKey: matchingAssetKey(definition, enabledWhen),
    })
  }

  const abilities: SelectableAbilityEditor[] = []
  for (const option of definition.move_options.filter(candidate => candidate.kind === 'ability')) {
    if (option.layer_ids.length !== 1) {
      return { session: null, unsupportedReason: `능력 \`${option.name}\`이 여러 이동 레이어를 동시에 사용합니다.` }
    }
    const layerId = option.layer_ids[0]
    const layer = allLayers.get(layerId)
    if (!layer) {
      return { session: null, unsupportedReason: `능력 \`${option.name}\`의 이동 레이어를 찾을 수 없습니다.` }
    }
    if (referencedLayerIds.has(layerId)) {
      return { session: null, unsupportedReason: `이동 레이어 \`${layerId}\`가 일반 이동과 능력에서 동시에 사용됩니다.` }
    }
    referencedLayerIds.add(layerId)
    abilities.push({
      id: option.id,
      name: option.name,
      description: option.description,
      movementCode: layer.chessembly_code,
      enabledWhen: layer.enabled_when.map(conditionFromEngine),
      onCommit: layer.on_commit.map(updateFromEngine),
      assetKey: '',
      cooldownEnabled: Boolean(option.cooldown),
      cooldownTurns: option.cooldown?.turns ?? 1,
      cooldownClock: option.cooldown?.clock ?? 'owner_turns',
      contributesToAttackMap: option.contributes_to_attack_map,
      executionMode: option.execution_mode,
    })
  }

  const unreferenced = definition.move_layers.filter(layer => !referencedLayerIds.has(layer.id))
  if (unreferenced.length) {
    return {
      session: null,
      unsupportedReason: `어떤 이동 옵션에서도 사용하지 않는 레이어가 있습니다: ${unreferenced.map(layer => layer.id).join(', ')}`,
    }
  }

  return {
    unsupportedReason: '',
    session: {
      document,
      exposedPieceKey,
      model: {
        defaultAssetKey: definition.visual.default_asset_key,
        normalOptionName: normalOption?.name ?? '일반 이동',
        normalOptionDescription: normalOption?.description ?? '',
        states: definition.state_schema.map(state => ({
          key: state.key,
          initialValue: editableValueFromEngine(state.default_value),
        })),
        normalForms,
        abilities,
      },
    },
  }
}

export function serializeAbilityBuilder(session: AbilityBuilderSession): string {
  const document = structuredClone(session.document)
  const index = document.definitions.findIndex(definition => definition.id === session.exposedPieceKey)
  if (index < 0) throw new Error(`대표 기물 \`${session.exposedPieceKey}\` 정의를 찾을 수 없습니다.`)
  const definition = document.definitions[index]
  const normalLayers = session.model.normalForms.map(form => ({
    id: safeId(form.id, 'normal-move'),
    chessembly_code: form.movementCode,
    enabled_when: form.enabledWhen.map(conditionToEngine),
    on_commit: form.onCommit.map(updateToEngine),
  }))
  const abilityLayers = session.model.abilities.map(ability => ({
    id: safeId(ability.id, 'ability'),
    chessembly_code: ability.movementCode,
    enabled_when: ability.enabledWhen.map(conditionToEngine),
    on_commit: ability.onCommit.map(updateToEngine),
  }))

  definition.chessembly_code = normalLayers[0]?.chessembly_code ?? ''
  definition.state_schema = session.model.states.map(state => ({
    key: safeId(state.key, 'state'),
    default_value: editableValueToEngine(state.initialValue),
  }))
  definition.move_layers = [...normalLayers, ...abilityLayers]
  definition.move_options = [
    {
      id: 'normal',
      name: session.model.normalOptionName.trim() || '일반 이동',
      description: session.model.normalOptionDescription,
      kind: 'normal',
      layer_ids: normalLayers.map(layer => layer.id),
      execution_mode: 'move_modifier',
      contributes_to_attack_map: true,
    },
    ...session.model.abilities.map((ability, index) => ({
      id: abilityLayers[index].id,
      name: ability.name.trim() || `특수 능력 ${index + 1}`,
      description: ability.description,
      kind: 'ability' as const,
      layer_ids: [abilityLayers[index].id],
      execution_mode: ability.executionMode,
      contributes_to_attack_map: ability.contributesToAttackMap,
      cooldown: ability.cooldownEnabled
        ? { turns: Math.max(1, Math.trunc(ability.cooldownTurns)), clock: ability.cooldownClock }
        : undefined,
    })),
  ]
  definition.visual = {
    default_asset_key: session.model.defaultAssetKey.trim() || definition.id,
    variants: session.model.normalForms
      .filter(form => form.assetKey.trim() && form.enabledWhen.length)
      .map((form, index) => ({
        id: `${safeId(form.id, 'form')}-visual`,
        enabled_when: form.enabledWhen.map(conditionToEngine),
        asset_key: form.assetKey.trim(),
        priority: 10 + index,
      })),
  }
  definition.is_king = false
  definition.can_capture_on_drop = false
  document.definitions[index] = definition
  return serializeCustomPiecePackage(document)
}

export function newStateVariable(index: number): StateVariableEditor {
  return { key: `state-${index + 1}`, initialValue: newEditableValue('text') }
}

export function newNormalForm(index: number): MovementFormEditor {
  return {
    id: `form-${index + 1}`,
    movementCode: 'move(0, 1);',
    enabledWhen: [],
    onCommit: [],
    assetKey: '',
  }
}

export function newSelectableAbility(index: number): SelectableAbilityEditor {
  return {
    ...newNormalForm(index),
    id: `ability-${index + 1}`,
    name: `특수 능력 ${index + 1}`,
    description: '',
    cooldownEnabled: true,
    cooldownTurns: 1,
    cooldownClock: 'owner_turns',
    contributesToAttackMap: true,
    executionMode: 'move_modifier',
  }
}

export function newStateCondition(): StateConditionEditor {
  return { key: '', operator: 'equals', expectedValue: newEditableValue('text') }
}

export function newStateUpdate(): StateUpdateEditor {
  return { key: '', value: newEditableValue('text') }
}

export function changeEditableValueType(value: EditableValue, type: EditableValueType) {
  value.type = type
}

function newEditableValue(type: EditableValueType): EditableValue {
  return { type, numberValue: 0, booleanValue: false, textValue: '' }
}

function editableValueFromEngine(value: PieceStateValue): EditableValue {
  if (typeof value === 'number') return { ...newEditableValue('number'), numberValue: value }
  if (typeof value === 'boolean') return { ...newEditableValue('boolean'), booleanValue: value }
  return { ...newEditableValue('text'), textValue: value }
}

function editableValueToEngine(value: EditableValue): PieceStateValue {
  if (value.type === 'number') return Number.isFinite(value.numberValue) ? value.numberValue : 0
  if (value.type === 'boolean') return value.booleanValue
  return value.textValue
}

function conditionFromEngine(predicate: PieceStatePredicate): StateConditionEditor {
  if ('equals' in predicate.condition) {
    return { key: predicate.key, operator: 'equals', expectedValue: editableValueFromEngine(predicate.condition.equals) }
  }
  return { key: predicate.key, operator: 'not_equals', expectedValue: editableValueFromEngine(predicate.condition.not_equals) }
}

function conditionToEngine(condition: StateConditionEditor): PieceStatePredicate {
  const value = editableValueToEngine(condition.expectedValue)
  const engineCondition: PieceStateCondition = condition.operator === 'equals'
    ? { equals: value }
    : { not_equals: value }
  return { key: safeId(condition.key, 'state'), condition: engineCondition }
}

function updateFromEngine(update: PieceStateUpdateDefinition): StateUpdateEditor {
  return { key: update.key, value: editableValueFromEngine(update.value) }
}

function updateToEngine(update: StateUpdateEditor): PieceStateUpdateDefinition {
  return { key: safeId(update.key, 'state'), value: editableValueToEngine(update.value) }
}

function matchingAssetKey(definition: PieceDefinition, enabledWhen: PieceStatePredicate[]): string {
  const variant = definition.visual.variants.find(candidate =>
    JSON.stringify(candidate.enabled_when) === JSON.stringify(enabledWhen),
  )
  return variant?.asset_key ?? ''
}

function safeId(value: string, fallback: string): string {
  const normalized = value.trim().replace(/[^A-Za-z0-9_-]/g, '-').replace(/-+/g, '-')
  return normalized && /^[A-Za-z_]/.test(normalized) ? normalized : fallback
}
