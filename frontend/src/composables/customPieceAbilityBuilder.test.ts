import assert from 'node:assert/strict'
import test from 'node:test'

import {
  loadAbilityBuilder,
  newSelectableAbility,
  newStateCondition,
  newStateUpdate,
  serializeAbilityBuilder,
} from './customPieceAbilityBuilder.ts'
import {
  customPieceTemplate,
  parseCustomPiecePackage,
  serializeCustomPiecePackage,
} from './useCustomPieceDraft.ts'

test('windmill becomes two intuitive movement forms and round-trips to engine JSON', () => {
  const source = serializeCustomPiecePackage(customPieceTemplate('windmill'))
  const loaded = loadAbilityBuilder(source, 'main')
  assert.equal(loaded.unsupportedReason, '')
  assert.ok(loaded.session)
  assert.equal(loaded.session.model.states[0].key, 'mode')
  assert.deepEqual(loaded.session.model.normalForms.map(form => form.id), ['bishop_mode', 'rook_mode'])
  assert.equal(loaded.session.model.normalForms[0].onCommit[0].value.textValue, 'rook')

  loaded.session.model.normalForms[0].assetKey = 'custom-bishop'
  const document = parseCustomPiecePackage(serializeAbilityBuilder(loaded.session))
  const definition = document.definitions[0]
  assert.equal(definition.move_options[0].kind, 'normal')
  assert.deepEqual(definition.move_options[0].layer_ids, ['bishop_mode', 'rook_mode'])
  assert.equal(definition.visual.variants[0].asset_key, 'custom-bishop')
})

test('selectable movement ability exposes cooldown and state effects', () => {
  const source = serializeCustomPiecePackage(customPieceTemplate('cannon-rook'))
  const loaded = loadAbilityBuilder(source, 'main')
  assert.ok(loaded.session)
  const ability = loaded.session.model.abilities[0]
  assert.equal(ability.name, '포 이동')
  assert.equal(ability.cooldownTurns, 3)
  assert.equal(ability.cooldownClock, 'owner_turns')

  const condition = newStateCondition()
  condition.key = 'charged'
  condition.expectedValue.type = 'boolean'
  condition.expectedValue.booleanValue = true
  ability.enabledWhen.push(condition)
  const update = newStateUpdate()
  update.key = 'charged'
  update.value.type = 'boolean'
  update.value.booleanValue = false
  ability.onCommit.push(update)

  const definition = parseCustomPiecePackage(serializeAbilityBuilder(loaded.session)).definitions[0]
  const abilityOption = definition.move_options.find(option => option.kind === 'ability')!
  const abilityLayer = definition.move_layers.find(layer => layer.id === abilityOption.layer_ids[0])!
  assert.deepEqual(abilityOption.cooldown, { turns: 3, clock: 'owner_turns' })
  assert.deepEqual(abilityLayer.enabled_when, [{ key: 'charged', condition: { equals: true } }])
  assert.deepEqual(abilityLayer.on_commit, [{ key: 'charged', value: false }])
})

test('new ability cards serialize as one layer and one move option', () => {
  const source = serializeCustomPiecePackage(customPieceTemplate('windmill'))
  const loaded = loadAbilityBuilder(source, 'main')
  assert.ok(loaded.session)
  const ability = newSelectableAbility(0)
  ability.id = 'dash'
  ability.name = '돌진'
  ability.movementCode = 'take-move(0, 2);'
  ability.cooldownTurns = 2
  loaded.session.model.abilities.push(ability)

  const definition = parseCustomPiecePackage(serializeAbilityBuilder(loaded.session)).definitions[0]
  const option = definition.move_options.find(candidate => candidate.id === 'dash')!
  assert.deepEqual(option.layer_ids, ['dash'])
  assert.equal(definition.move_layers.find(layer => layer.id === 'dash')?.chessembly_code, 'take-move(0, 2);')
})

test('structures that would lose meaning fall back to expert JSON mode', () => {
  const document = customPieceTemplate('cannon-rook')
  const ability = document.definitions[0].move_options.find(option => option.kind === 'ability')!
  ability.layer_ids.push('rook_move')
  const loaded = loadAbilityBuilder(serializeCustomPiecePackage(document), 'main')
  assert.equal(loaded.session, null)
  assert.match(loaded.unsupportedReason, /여러 이동 레이어/)
})
