import type {
  AdvancedCustomPieceDraft,
  CustomPieceInput,
  CustomPieceTestPiece,
  SimpleCustomPieceDraft,
} from '../types/customPiece'
import type { GameState, PieceDefinition } from '../types/game'

export interface CustomPiecePackageDocument {
  format: 'brainfuck-chess-piece-set-v1'
  definitions: PieceDefinition[]
}

export type AdvancedTemplateKind = 'windmill' | 'cannon-rook' | 'bouncing-bishop'

const BISHOP_CODE = `take-move(1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(-1, -1) repeat(1);`

const ROOK_CODE = `take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);`

const CANNON_CODE = `do peek(0, 1) while take-move(0, 1) repeat(1);
do peek(1, 0) while take-move(1, 0) repeat(1);
do peek(0, -1) while take-move(0, -1) repeat(1);
do peek(-1, 0) while take-move(-1, 0) repeat(1);`

const BOUNCING_BISHOP_CODE = `do
take-move(1, 1)
while
edge(1, 1) {
  take-move(-1, 1) repeat(1)
} {
  take-move(1, -1) repeat(1)
};

do
take-move(-1, 1)
while
edge(-1, 1) {
  take-move(1, 1) repeat(1)
} {
  take-move(-1, -1) repeat(1)
};

do
take-move(1, -1)
while
edge(1, -1) {
  take-move(1, 1) repeat(1)
} {
  take-move(-1, -1) repeat(1)
};

do
take-move(-1, -1)
while
edge(-1, -1) {
  take-move(1, -1) repeat(1)
} {
  take-move(-1, 1) repeat(1)
};`

export function newCustomPieceDefinition(id = 'main'): PieceDefinition {
  return {
    id,
    name: id === 'main' ? '' : id,
    score: 1,
    chessembly_code: 'move(0, 1);\ntake(1, 1);\ntake(-1, 1);',
    chessembly_version: '1.0',
    is_king: false,
    can_capture_on_drop: false,
    promotion_pool: [],
    state_schema: [],
    move_layers: [],
    move_options: [],
    visual: { default_asset_key: 'knight', variants: [] },
  }
}

export function newCustomPieceScript(): string {
  return serializeCustomPiecePackage({
    format: 'brainfuck-chess-piece-set-v1',
    definitions: [newCustomPieceDefinition()],
  })
}

export function newSimpleCustomPieceDraft(): SimpleCustomPieceDraft {
  return {
    name: '',
    description: '',
    score: 1,
    image: { kind: 'built_in', asset_key: 'knight' },
    movementCode: 'move(0, 1);\ntake(1, 1);\ntake(-1, 1);',
    abilities: [],
  }
}

export function newAdvancedCustomPieceDraft(
  source: SimpleCustomPieceDraft = newSimpleCustomPieceDraft(),
): AdvancedCustomPieceDraft {
  const input = buildCustomPieceInput(source)
  return {
    name: source.name,
    description: source.description,
    score: source.score,
    image: { ...source.image },
    rawScript: input.raw_script,
    exposedPieceKey: input.exposed_piece_key,
  }
}

/** Converts user-facing fields into the existing immutable package protocol. */
export function buildCustomPieceInput(draft: SimpleCustomPieceDraft): CustomPieceInput {
  const definition = newCustomPieceDefinition('main')
  definition.name = draft.name.trim()
  definition.score = draft.score
  definition.chessembly_code = draft.movementCode
  definition.state_schema = draft.abilities.map((ability, index) => ({
    key: safeStateKey(ability.name, index),
    default_value: ability.initialValue,
  }))
  return {
    name: draft.name,
    description: draft.description,
    score: draft.score,
    image: { ...draft.image },
    raw_script: serializeCustomPiecePackage({
      format: 'brainfuck-chess-piece-set-v1',
      definitions: [definition],
    }),
    exposed_piece_key: 'main',
  }
}

/** Preserves state, layer, option, visual-variant and internal-definition fields. */
export function buildAdvancedCustomPieceInput(draft: AdvancedCustomPieceDraft): CustomPieceInput {
  const document = parseCustomPiecePackage(draft.rawScript)
  const exposed = document.definitions.find(definition => definition.id === draft.exposedPieceKey)
  if (!exposed) throw new Error(`대표 기물 \`${draft.exposedPieceKey}\` 정의를 찾을 수 없습니다.`)

  exposed.name = draft.name.trim()
  exposed.score = draft.score
  for (const definition of document.definitions) {
    definition.is_king = false
    definition.can_capture_on_drop = false
  }

  return {
    name: draft.name,
    description: draft.description,
    score: draft.score,
    image: { ...draft.image },
    raw_script: serializeCustomPiecePackage(document),
    exposed_piece_key: draft.exposedPieceKey,
  }
}

export function advancedDraftFromInput(input: CustomPieceInput): AdvancedCustomPieceDraft {
  parseCustomPiecePackage(input.raw_script)
  return {
    name: input.name,
    description: input.description,
    score: input.score,
    image: { ...input.image },
    rawScript: input.raw_script,
    exposedPieceKey: input.exposed_piece_key,
  }
}

/** Converts only definitions representable without losing meaning. */
export function simpleDraftFromInput(input: CustomPieceInput): SimpleCustomPieceDraft | null {
  let document: CustomPiecePackageDocument
  try {
    document = parseCustomPiecePackage(input.raw_script)
  } catch {
    return null
  }
  if (document.definitions.length !== 1) return null
  const definition = document.definitions[0]
  if (
    definition.id !== input.exposed_piece_key
    || definition.move_layers.length > 0
    || definition.move_options.length > 0
    || (definition.promotion_pool?.length ?? 0) > 0
    || definition.promotion
    || (definition.extensions?.length ?? 0) > 0
    || definition.visual.variants.length > 0
    || definition.state_schema.some(state => typeof state.default_value !== 'number')
  ) return null
  return {
    name: input.name,
    description: input.description,
    score: input.score,
    image: { ...input.image },
    movementCode: definition.chessembly_code,
    abilities: definition.state_schema.map(state => ({
      kind: 'remember_value',
      name: state.key,
      initialValue: state.default_value as number,
    })),
  }
}

function safeStateKey(name: string, index: number): string {
  const normalized = name.trim().replace(/[^A-Za-z0-9_-]/g, '-').replace(/-+/g, '-')
  return normalized && /^[A-Za-z_]/.test(normalized) ? normalized : `memory-${index + 1}`
}

export function parseCustomPiecePackage(rawScript: string): CustomPiecePackageDocument {
  const value: unknown = JSON.parse(rawScript)
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('패키지가 객체가 아닙니다.')
  const document = value as Partial<CustomPiecePackageDocument>
  if (document.format !== 'brainfuck-chess-piece-set-v1') throw new Error('지원하지 않는 패키지 형식입니다.')
  if (!Array.isArray(document.definitions) || document.definitions.length === 0) {
    throw new Error('기물 정의가 하나 이상 필요합니다.')
  }
  return {
    format: document.format,
    definitions: document.definitions.map((definition, index) =>
      normalizeCustomPieceDefinition(definition, index),
    ),
  }
}

export function serializeCustomPiecePackage(document: CustomPiecePackageDocument): string {
  return JSON.stringify(document, null, 2)
}

function normalizeCustomPieceDefinition(value: PieceDefinition, index: number): PieceDefinition {
  if (!value || typeof value !== 'object') throw new Error(`${index + 1}번째 기물 정의가 올바르지 않습니다.`)
  const fallback = newCustomPieceDefinition(`piece-${index + 1}`)
  return {
    ...fallback,
    ...value,
    id: typeof value.id === 'string' ? value.id : fallback.id,
    name: typeof value.name === 'string' ? value.name : fallback.name,
    score: typeof value.score === 'number' ? value.score : fallback.score,
    chessembly_code: typeof value.chessembly_code === 'string' ? value.chessembly_code : '',
    chessembly_version: typeof value.chessembly_version === 'string' ? value.chessembly_version : '1.0',
    extensions: Array.isArray(value.extensions) ? value.extensions : [],
    promotion_pool: Array.isArray(value.promotion_pool) ? value.promotion_pool : [],
    state_schema: Array.isArray(value.state_schema) ? value.state_schema : [],
    move_layers: Array.isArray(value.move_layers) ? value.move_layers : [],
    move_options: Array.isArray(value.move_options) ? value.move_options : [],
    visual: {
      default_asset_key: value.visual?.default_asset_key || value.id || fallback.id,
      variants: Array.isArray(value.visual?.variants) ? value.visual.variants : [],
    },
    is_king: false,
    can_capture_on_drop: false,
  }
}

export function customPieceTemplate(kind: AdvancedTemplateKind): CustomPiecePackageDocument {
  const definition = newCustomPieceDefinition('main')
  definition.name = kind === 'windmill' ? 'Windmill' : kind === 'cannon-rook' ? 'Cannon Rook' : 'Bouncing Bishop'
  definition.score = kind === 'windmill' ? 4 : 7

  if (kind === 'windmill') {
    definition.chessembly_code = BISHOP_CODE
    definition.state_schema = [{ key: 'mode', default_value: 'bishop' }]
    definition.move_layers = [
      {
        id: 'bishop_mode', chessembly_code: BISHOP_CODE,
        enabled_when: [{ key: 'mode', condition: { equals: 'bishop' } }],
        on_commit: [{ key: 'mode', value: 'rook' }],
      },
      {
        id: 'rook_mode', chessembly_code: ROOK_CODE,
        enabled_when: [{ key: 'mode', condition: { equals: 'rook' } }],
        on_commit: [{ key: 'mode', value: 'bishop' }],
      },
    ]
    definition.move_options = [{
      id: 'normal', name: '일반 이동', description: '이동할 때마다 비숍/룩 모드를 전환합니다.',
      kind: 'normal', layer_ids: ['bishop_mode', 'rook_mode'], execution_mode: 'move_modifier',
      contributes_to_attack_map: true,
    }]
    definition.visual = {
      default_asset_key: 'bishop',
      variants: [{
        id: 'rook_mode', enabled_when: [{ key: 'mode', condition: { equals: 'rook' } }],
        asset_key: 'rook', priority: 10,
      }],
    }
  } else if (kind === 'cannon-rook') {
    definition.chessembly_code = ROOK_CODE
    definition.move_layers = [
      { id: 'rook_move', chessembly_code: ROOK_CODE, enabled_when: [], on_commit: [] },
      { id: 'cannon_move', chessembly_code: CANNON_CODE, enabled_when: [], on_commit: [] },
    ]
    definition.move_options = [
      {
        id: 'normal', name: '일반 이동', description: '', kind: 'normal',
        layer_ids: ['rook_move'], execution_mode: 'move_modifier', contributes_to_attack_map: true,
      },
      {
        id: 'cannon_move', name: '포 이동', description: '기물 하나를 뛰어넘는 이동입니다.', kind: 'ability',
        layer_ids: ['cannon_move'], execution_mode: 'move_modifier', contributes_to_attack_map: true,
        cooldown: { turns: 3, clock: 'owner_turns' },
      },
    ]
    definition.visual.default_asset_key = 'rook'
  } else {
    definition.chessembly_code = BISHOP_CODE
    definition.move_layers = [
      { id: 'bishop_move', chessembly_code: BISHOP_CODE, enabled_when: [], on_commit: [] },
      { id: 'bounce_move', chessembly_code: BOUNCING_BISHOP_CODE, enabled_when: [], on_commit: [] },
    ]
    definition.move_options = [
      {
        id: 'normal', name: '일반 이동', description: '', kind: 'normal',
        layer_ids: ['bishop_move'], execution_mode: 'move_modifier', contributes_to_attack_map: true,
      },
      {
        id: 'bounce_move', name: '반사 이동', description: '가장자리에서 반사되는 이동입니다.', kind: 'ability',
        layer_ids: ['bounce_move'], execution_mode: 'move_modifier', contributes_to_attack_map: true,
        cooldown: { turns: 2, clock: 'owner_turns' },
      },
    ]
    definition.visual.default_asset_key = 'bishop'
  }

  return { format: 'brainfuck-chess-piece-set-v1', definitions: [definition] }
}

export function customPieceDraftSnapshot(draft: SimpleCustomPieceDraft | AdvancedCustomPieceDraft | CustomPieceInput): string {
  return JSON.stringify(draft)
}

export function validateCustomPieceDraft(draft: SimpleCustomPieceDraft): string {
  if (!draft.name.trim() || draft.name.trim().length > 80) return '이름은 1–80자여야 합니다.'
  if (!Number.isInteger(draft.score) || draft.score < 1 || draft.score > 30) return '점수는 1–30 사이의 정수여야 합니다.'
  if (!draft.movementCode.trim()) return '움직임 코드를 입력해 주세요.'
  if (draft.abilities.some(ability => !ability.name.trim())) return '기억할 값의 이름을 입력해 주세요.'
  return ''
}

export function validateAdvancedCustomPieceDraft(draft: AdvancedCustomPieceDraft): string {
  if (!draft.name.trim() || draft.name.trim().length > 80) return '이름은 1–80자여야 합니다.'
  if (!Number.isInteger(draft.score) || draft.score < 1 || draft.score > 30) return '점수는 1–30 사이의 정수여야 합니다.'
  if (!draft.exposedPieceKey.trim()) return '대표 기물 키를 입력해 주세요.'
  try {
    buildAdvancedCustomPieceInput(draft)
  } catch (error) {
    return error instanceof Error ? error.message : '고급 기물 정의가 올바르지 않습니다.'
  }
  return ''
}

export function testPiecesFromServerState(state: GameState): CustomPieceTestPiece[] {
  return Object.values(state.pieces)
    .filter(piece => !piece.captured && !piece.in_pocket && piece.current_square)
    .map(piece => ({
      id: piece.id,
      piece_key: piece.type_id.includes(':') ? piece.type_id.slice(piece.type_id.lastIndexOf(':') + 1) : piece.type_id,
      owner: piece.owner,
      square: piece.current_square!,
      state: piece.state,
    }))
}
