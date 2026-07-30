import type { CustomPieceInput, CustomPieceTestPiece } from '../types/customPiece'
import type { GameState, PieceDefinition } from '../types/game'

export interface CustomPiecePackageDocument {
  format: 'brainfuck-chess-piece-set-v1'
  definitions: PieceDefinition[]
}

export function newCustomPieceDefinition(id = 'hero'): PieceDefinition {
  return {
    id,
    name: id === 'hero' ? 'Hero' : id,
    score: 1,
    chessembly_code: 'move(1, 0);',
    chessembly_version: '1.0',
    dialect: 'classic',
    extensions: [],
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

export function customPieceDraftSnapshot(draft: CustomPieceInput): string {
  return JSON.stringify(draft)
}

export function validateCustomPieceDraft(draft: CustomPieceInput): string {
  if (!draft.name.trim() || draft.name.trim().length > 80) return '이름은 1–80자여야 합니다.'
  if (!Number.isInteger(draft.score) || draft.score < 1 || draft.score > 30) return '점수는 1–30 사이의 정수여야 합니다.'
  if (!draft.raw_script.trim()) return '커스텀 기물 패키지 JSON을 입력해 주세요.'
  try {
    JSON.parse(draft.raw_script)
  } catch {
    return '코드는 JSON 패키지 형식이어야 합니다. 체섬블리 코드는 definitions[].chessembly_code에 입력해 주세요.'
  }
  if (!/^[A-Za-z0-9_-]+$/.test(draft.exposed_piece_key)) return '대표 기물 식별자를 입력해 주세요.'
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
