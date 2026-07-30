import type {
  CustomPieceInput,
  CustomPieceTestPiece,
  SimpleCustomPieceDraft,
} from '../types/customPiece'
import type { GameState, PieceDefinition } from '../types/game'

export interface CustomPiecePackageDocument {
  format: 'brainfuck-chess-piece-set-v1'
  definitions: PieceDefinition[]
}

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

/**
 * Existing advanced packages are deliberately not reinterpreted. Callers can
 * open them read-only instead of silently changing their meaning.
 */
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
    image: input.image,
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

export function customPieceDraftSnapshot(draft: SimpleCustomPieceDraft | CustomPieceInput): string {
  return JSON.stringify(draft)
}

export function validateCustomPieceDraft(draft: SimpleCustomPieceDraft): string {
  if (!draft.name.trim() || draft.name.trim().length > 80) return '이름은 1–80자여야 합니다.'
  if (!Number.isInteger(draft.score) || draft.score < 1 || draft.score > 30) return '점수는 1–30 사이의 정수여야 합니다.'
  if (!draft.movementCode.trim()) return '움직임 코드를 입력해 주세요.'
  if (draft.abilities.some(ability => !ability.name.trim())) return '기억할 값의 이름을 입력해 주세요.'
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
