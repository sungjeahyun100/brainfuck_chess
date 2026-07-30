<template>
  <section class="cp-card cp-package">
    <div class="cp-row cp-between">
      <div>
        <h3>기물 동작 설정</h3>
        <p class="cp-muted">JSON 없이 기물 정의와 동작을 설정합니다. 체섬블리 명령만 코드 칸에 입력하세요.</p>
      </div>
      <button class="btn-secondary" type="button" @click="addDefinition">내부 기물 추가</button>
    </div>

    <p v-if="parseError" class="error cp-banner" role="alert">
      저장된 패키지를 UI로 불러오지 못했습니다: {{ parseError }}
    </p>

    <template v-else>
      <label>대표 기물
        <select v-model="draft.exposed_piece_key">
          <option v-for="definition in document.definitions" :key="definition.id" :value="definition.id">
            {{ definition.name || definition.id }} ({{ definition.id }})
          </option>
        </select>
        <small class="cp-muted">덱에 표시되는 기물입니다. 대표 기물의 이름과 점수는 왼쪽 기본 정보와 자동으로 같아집니다.</small>
      </label>

      <nav class="cp-definition-tabs" aria-label="기물 정의">
        <button
          v-for="(definition, index) in document.definitions"
          :key="index"
          type="button"
          :class="{ active: selectedIndex === index }"
          @click="selectedIndex = index"
        >{{ definition.name || definition.id || `기물 ${index + 1}` }}</button>
      </nav>

      <div v-if="selected" class="cp-definition-form">
        <div class="cp-section-heading">
          <h4>정의 기본값</h4>
          <button v-if="document.definitions.length > 1" class="btn-secondary danger" type="button" @click="removeDefinition">이 정의 삭제</button>
        </div>
        <div class="cp-fields">
          <label>식별자
            <input v-model.trim="selected.id" required pattern="[A-Za-z0-9_-]+" @change="renameDefinition" />
            <small class="cp-muted">코드의 transition과 승급 대상에서 참조하는 영문 ID입니다.</small>
          </label>
          <label>표시 이름
            <input v-model="selected.name" required :disabled="isExposed" />
            <small class="cp-muted">{{ isExposed ? '대표 기물이므로 기본 정보의 이름을 사용합니다.' : '변신·승급 후 화면에 표시할 이름입니다.' }}</small>
          </label>
          <label>{{ isExposed ? '기물 점수' : '내부 기물 점수' }}
            <input v-model.number="selected.score" type="number" min="0" :disabled="isExposed" />
            <small class="cp-muted">{{ isExposed ? '대표 기물이므로 기본 정보의 점수를 사용합니다.' : '내부 형태의 평가 및 비용 계산에 사용하는 값입니다.' }}</small>
          </label>
          <label>문법
            <select v-model="selected.dialect">
              <option value="classic">Classic</option>
              <option value="brainfuck-chess">Brainfuck Chess</option>
            </select>
            <small class="cp-muted">체섬블리 코드를 해석할 문법 계열입니다.</small>
          </label>
          <label>체섬블리 버전
            <input v-model="selected.chessembly_version" />
            <small class="cp-muted">코드가 대상으로 하는 Chessembly 호환 버전입니다.</small>
          </label>
          <label>기본 비주얼 키
            <input v-model.trim="selected.visual.default_asset_key" />
            <small class="cp-muted">업로드 이미지가 아닌 상태별 논리 이미지 키입니다.</small>
          </label>
        </div>

        <label>기본 이동 코드
          <textarea v-model="selected.chessembly_code" rows="7" spellcheck="false" placeholder="예: move(1, 0);" />
          <small class="cp-muted">기존 Chessembly 문법을 그대로 사용합니다.</small>
        </label>

        <details>
          <summary>확장 기능과 승급</summary>
          <div class="cp-fields cp-details-body">
            <label class="cp-wide">확장 기능
              <input :value="selected.extensions?.join(', ') ?? ''" placeholder="쉼표로 구분" @input="setExtensions" />
              <small class="cp-muted">이 정의가 요구하는 선택적 문법 확장 이름입니다. 여러 개는 쉼표로 구분합니다.</small>
            </label>
            <label>승급 조건
              <select :value="promotionType" @change="setPromotionType">
                <option value="">사용 안 함</option>
                <option value="first_rank">첫 랭크</option>
                <option value="last_rank">마지막 랭크</option>
                <option value="rank">지정 랭크</option>
              </select>
              <small class="cp-muted">지정한 랭크에 도착했을 때 승급 선택지를 활성화합니다.</small>
            </label>
            <label v-if="promotionType === 'rank'">승급 랭크
              <input v-model.number="promotionRank" type="number" min="0" />
            </label>
            <label class="cp-wide">승급 대상
              <div class="cp-check-list">
                <label v-for="definition in otherDefinitions" :key="definition.id">
                  <input type="checkbox" :checked="selected.promotion_pool?.includes(definition.id)" @change="togglePromotionTarget(definition.id)" />
                  {{ definition.name || definition.id }}
                </label>
              </div>
              <small class="cp-muted">같은 패키지 안에서 승급할 수 있는 내부 기물을 선택합니다.</small>
            </label>
          </div>
        </details>

        <details>
          <summary>상태 변수 ({{ selected.state_schema.length }})</summary>
          <div class="cp-details-body">
            <div v-for="(state, index) in selected.state_schema" :key="index" class="cp-array-row">
              <label>키 <input v-model.trim="state.key" /></label>
              <label>형식
                <select :value="valueType(state.default_value)" @change="changeStateType(index, $event)">
                  <option value="number">숫자</option><option value="boolean">참/거짓</option><option value="string">문자열</option>
                </select>
              </label>
              <label>기본값
                <select v-if="typeof state.default_value === 'boolean'" v-model="state.default_value">
                  <option :value="true">참</option><option :value="false">거짓</option>
                </select>
                <input v-else-if="typeof state.default_value === 'number'" v-model.number="state.default_value" type="number" />
                <input v-else v-model="state.default_value" />
              </label>
              <button class="btn-secondary danger" type="button" @click="selected.state_schema.splice(index, 1)">삭제</button>
            </div>
            <p class="cp-muted">상태 변수는 기물마다 따로 저장되며 이동 조건, 변신, 쿨다운형 동작에 사용할 수 있습니다.</p>
            <button class="btn-secondary" type="button" @click="selected.state_schema.push({ key: `state${selected.state_schema.length + 1}`, default_value: 0 })">상태 추가</button>
          </div>
        </details>

        <details>
          <summary>이동 레이어 ({{ selected.move_layers.length }})</summary>
          <div class="cp-details-body">
            <article v-for="(layer, index) in selected.move_layers" :key="index" class="cp-subcard">
              <div class="cp-section-heading"><h5>{{ layer.id || `레이어 ${index + 1}` }}</h5><button class="btn-secondary danger" type="button" @click="selected.move_layers.splice(index, 1)">삭제</button></div>
              <label>레이어 식별자 <input v-model.trim="layer.id" /></label>
              <label>체섬블리 코드 <textarea v-model="layer.chessembly_code" rows="5" spellcheck="false" /></label>
              <p class="cp-muted">레이어는 기본 이동과 별도로 조합할 수 있는 독립 이동 규칙입니다. 아래 이동 옵션에서 사용할 레이어를 선택합니다.</p>
            </article>
            <button class="btn-secondary" type="button" @click="addLayer">레이어 추가</button>
          </div>
        </details>

        <details>
          <summary>이동 옵션 ({{ selected.move_options.length }})</summary>
          <div class="cp-details-body">
            <article v-for="(option, index) in selected.move_options" :key="index" class="cp-subcard">
              <div class="cp-section-heading"><h5>{{ option.name || `옵션 ${index + 1}` }}</h5><button class="btn-secondary danger" type="button" @click="selected.move_options.splice(index, 1)">삭제</button></div>
              <div class="cp-fields">
                <label>식별자 <input v-model.trim="option.id" /></label>
                <label>이름 <input v-model="option.name" /></label>
                <label>종류 <select v-model="option.kind"><option value="normal">일반</option><option value="ability">능력</option></select></label>
                <label>실행 방식
                  <select v-model="option.execution_mode"><option value="move_modifier">이동 수정</option><option value="standalone_action">독립 행동</option></select>
                  <small class="cp-muted">이동 수정은 기본 이동과 합치고, 독립 행동은 선택한 레이어만 실행합니다.</small>
                </label>
                <label>쿨다운 턴
                  <input :value="option.cooldown?.turns ?? 0" type="number" min="0" @input="setCooldown(option, $event)" />
                  <small class="cp-muted">0이면 쿨다운이 없습니다.</small>
                </label>
                <label>쿨다운 기준
                  <select :value="option.cooldown?.clock ?? 'owner_turns'" :disabled="!option.cooldown" @change="setCooldownClock(option, $event)"><option value="owner_turns">소유자 턴</option><option value="global_turns">전체 턴</option></select>
                  <small class="cp-muted">소유자 턴은 내 턴마다, 전체 턴은 양쪽의 매 턴마다 감소합니다.</small>
                </label>
                <label class="cp-check"><input v-model="option.contributes_to_attack_map" type="checkbox" /> 공격 맵에 포함 <small class="cp-muted">체크하면 왕의 위험 칸과 공격 판정에도 이 옵션이 반영됩니다.</small></label>
              </div>
              <label>설명 <input v-model="option.description" /></label>
              <fieldset><legend>사용할 레이어</legend><div class="cp-check-list">
                <label v-for="layer in availableLayers" :key="layer"><input type="checkbox" :checked="option.layer_ids.includes(layer)" @change="toggleLayer(option, layer)" />{{ layer }}</label>
              </div></fieldset>
            </article>
            <button class="btn-secondary" type="button" @click="addOption">이동 옵션 추가</button>
          </div>
        </details>
      </div>
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { newCustomPieceDefinition, parseCustomPiecePackage, serializeCustomPiecePackage, type CustomPiecePackageDocument } from '../../composables/useCustomPieceDraft'
import type { CustomPieceInput } from '../../types/customPiece'
import type { MoveOptionDefinition, PieceStateValue } from '../../types/game'

const props = defineProps<{ draft: CustomPieceInput }>()
const document = ref<CustomPiecePackageDocument>({ format: 'brainfuck-chess-piece-set-v1', definitions: [] })
const parseError = ref('')
const selectedIndex = ref(0)
let syncing = false

watch(() => props.draft.raw_script, raw => {
  if (syncing) return
  try {
    document.value = parseCustomPiecePackage(raw)
    parseError.value = ''
    selectedIndex.value = Math.min(selectedIndex.value, document.value.definitions.length - 1)
  } catch (caught) {
    parseError.value = caught instanceof Error ? caught.message : '알 수 없는 형식입니다.'
  }
}, { immediate: true })

watch(document, value => {
  if (parseError.value) return
  syncing = true
  props.draft.raw_script = serializeCustomPiecePackage(value)
  queueMicrotask(() => { syncing = false })
}, { deep: true })

const selected = computed(() => document.value.definitions[selectedIndex.value])
const isExposed = computed(() => selected.value.id === props.draft.exposed_piece_key)
const otherDefinitions = computed(() => document.value.definitions.filter((_, index) => index !== selectedIndex.value))
const availableLayers = computed(() => ['default', ...selected.value.move_layers.map(layer => layer.id)].filter((id, index, all) => id && all.indexOf(id) === index))
const promotionType = computed(() => selected.value.promotion?.condition.type ?? '')
const promotionRank = computed({
  get: () => selected.value.promotion?.condition.type === 'rank' ? selected.value.promotion.condition.rank : 0,
  set: rank => { selected.value.promotion = { condition: { type: 'rank', rank } } },
})

watch(
  [() => props.draft.name, () => props.draft.score, () => props.draft.exposed_piece_key, document],
  () => syncExposedDefinition(),
  { deep: true, immediate: true },
)

function addDefinition() {
  const used = new Set(document.value.definitions.map(item => item.id))
  let number = document.value.definitions.length + 1
  while (used.has(`piece-${number}`)) number += 1
  document.value.definitions.push(newCustomPieceDefinition(`piece-${number}`))
  selectedIndex.value = document.value.definitions.length - 1
}
function removeDefinition() {
  const [removed] = document.value.definitions.splice(selectedIndex.value, 1)
  if (props.draft.exposed_piece_key === removed.id) props.draft.exposed_piece_key = document.value.definitions[0].id
  for (const definition of document.value.definitions) {
    definition.promotion_pool = definition.promotion_pool?.filter(id => id !== removed.id)
  }
  selectedIndex.value = Math.max(0, selectedIndex.value - 1)
}
function renameDefinition(event: Event) {
  const input = event.target as HTMLInputElement
  const previousId = input.defaultValue
  const nextId = selected.value.id
  if (props.draft.exposed_piece_key === previousId) props.draft.exposed_piece_key = nextId
  for (const definition of document.value.definitions) {
    definition.promotion_pool = definition.promotion_pool?.map(id => id === previousId ? nextId : id)
  }
  input.defaultValue = nextId
  syncExposedDefinition()
}
function syncExposedDefinition() {
  const exposed = document.value.definitions.find(item => item.id === props.draft.exposed_piece_key)
  if (!exposed) return
  exposed.name = props.draft.name
  exposed.score = props.draft.score
}
function setExtensions(event: Event) {
  selected.value.extensions = (event.target as HTMLInputElement).value.split(',').map(value => value.trim()).filter(Boolean)
}
function setPromotionType(event: Event) {
  const type = (event.target as HTMLSelectElement).value
  selected.value.promotion = type ? { condition: type === 'rank' ? { type, rank: 0 } : { type } as { type: 'first_rank' | 'last_rank' } } : undefined
  if (!type) selected.value.promotion_pool = []
}
function togglePromotionTarget(id: string) {
  const pool = selected.value.promotion_pool ?? (selected.value.promotion_pool = [])
  const index = pool.indexOf(id)
  index < 0 ? pool.push(id) : pool.splice(index, 1)
}
function valueType(value: PieceStateValue) { return typeof value }
function changeStateType(index: number, event: Event) {
  const type = (event.target as HTMLSelectElement).value
  selected.value.state_schema[index].default_value = type === 'number' ? 0 : type === 'boolean' ? false : ''
}
function addLayer() {
  const number = selected.value.move_layers.length + 1
  selected.value.move_layers.push({ id: `layer-${number}`, chessembly_code: '', enabled_when: [], on_commit: [] })
}
function addOption() {
  const number = selected.value.move_options.length + 1
  selected.value.move_options.push({
    id: `option-${number}`, name: `옵션 ${number}`, description: '', kind: 'ability',
    layer_ids: [], execution_mode: 'standalone_action', contributes_to_attack_map: false,
  })
}
function toggleLayer(option: MoveOptionDefinition, layer: string) {
  const index = option.layer_ids.indexOf(layer)
  index < 0 ? option.layer_ids.push(layer) : option.layer_ids.splice(index, 1)
}
function setCooldown(option: MoveOptionDefinition, event: Event) {
  const turns = Number((event.target as HTMLInputElement).value)
  option.cooldown = turns > 0 ? { turns, clock: option.cooldown?.clock ?? 'owner_turns' } : undefined
}
function setCooldownClock(option: MoveOptionDefinition, event: Event) {
  if (option.cooldown) option.cooldown.clock = (event.target as HTMLSelectElement).value as 'owner_turns' | 'global_turns'
}
</script>
