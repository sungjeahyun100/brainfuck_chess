import { reactive } from 'vue'
import type {
  DeckPreset,
  DeckPresetLayout,
  DeckPieceType,
  DeckSummary,
  LobbyDeck,
  LobbyPlacement,
  PieceCatalogItem,
  SavedDeck,
} from '../types/deck'
import type { CustomPieceRecord } from '../types/customPiece'

export const boardSizes = [8, 9, 10, 11, 12] as const

export const pieceCatalog = reactive<PieceCatalogItem[]>([
  { id: 'king', name: 'King', score: 0, category: 'royal', canPocket: false, uniqueStarting: true },
  { id: 'queen', name: 'Queen', score: 0, category: 'major', canPocket: true },
  { id: 'cannon-rook', name: 'Cannon Rook', score: 0, category: 'variant', canPocket: true, aliases: ['cannon', 'po rook', '포 룩'] },
  { id: 'amazon', name: 'Amazon', score: 0, category: 'variant', canPocket: true },
  { id: 'tempest-queen', name: 'Tempest Queen', score: 0, category: 'variant', canPocket: true, aliases: ['storm queen'] },
  { id: 'tempest-rook', name: 'Tempest Rook', score: 0, category: 'variant', canPocket: true, aliases: ['storm rook'] },
  { id: 'tempest-bishop', name: 'Tempest Bishop', score: 0, category: 'variant', canPocket: true, aliases: ['storm bishop'] },
  { id: 'tempest-knight', name: 'Tempest Knight', score: 0, category: 'variant', canPocket: true, aliases: ['storm knight'] },
  { id: 'bouncing-bishop', name: 'Bouncing Bishop', score: 0, category: 'variant', canPocket: true, aliases: ['bounce bishop'] },
  { id: 'windmill', name: 'Windmill', score: 0, category: 'variant', canPocket: true, aliases: ['풍차'] },
  { id: 'tempest-pawn', name: 'Tempest Pawn', score: 0, category: 'pawn', canPocket: true, aliases: ['storm pawn'] },
  { id: 'rook', name: 'Rook', score: 0, category: 'major', canPocket: true },
  { id: 'bishop', name: 'Bishop', score: 0, category: 'minor', canPocket: true },
  { id: 'knight', name: 'Knight', score: 0, category: 'minor', canPocket: true },
  { id: 'paratrooper', name: '공수부대 대원', score: 0, category: 'variant', canPocket: true, aliases: ['Paratrooper', '공수부대'] },
  { id: 'pawn', name: 'Pawn', score: 0, category: 'pawn', canPocket: true },
])

export function customDeckPieceType(piece: Pick<CustomPieceRecord, 'id' | 'version' | 'exposed_piece_key'>): string {
  return `custom:${piece.id}:v${piece.version}:${piece.exposed_piece_key}`
}

function customCatalogItem(record: CustomPieceRecord): PieceCatalogItem {
  return {
    id: customDeckPieceType(record),
    name: record.name,
    score: record.score,
    category: 'custom',
    canPocket: true,
    aliases: [record.description, record.exposed_piece_key],
    custom: {
      id: record.id,
      version: record.version,
      contentHash: record.content_hash,
      exposedPieceKey: record.exposed_piece_key,
      image: record.image,
      active: record.active,
    },
  }
}

export function replaceCustomPieceCatalog(records: CustomPieceRecord[]): void {
  for (let index = pieceCatalog.length - 1; index >= 0; index -= 1) {
    if (pieceCatalog[index].custom) pieceCatalog.splice(index, 1)
  }
  pieceCatalog.push(...records.map(customCatalogItem))
}

export function upsertCustomPieceCatalog(record: CustomPieceRecord): void {
  const item = customCatalogItem(record)
  const index = pieceCatalog.findIndex(piece => piece.id === item.id)
  if (index >= 0) pieceCatalog.splice(index, 1, item)
  else pieceCatalog.push(item)
}

export function deactivateCustomPieceCatalog(id: string): void {
  for (const piece of pieceCatalog) {
    if (piece.custom?.id === id) piece.custom.active = false
  }
}

export function applyPieceScores(scores: Record<string, number>): void {
  for (const piece of pieceCatalog) {
    const score = scores[piece.id]
    if (!Number.isInteger(score) || score < 0) {
      throw new Error(`엔진 기물 점수가 누락되었거나 잘못되었습니다: ${piece.id}`)
    }
    piece.score = score
  }
}

export const pocketCatalog = pieceCatalog.filter(piece => piece.canPocket)

export const catalogCategoryLabels: Record<string, string> = {
  custom: '내 커스텀 기물',
  royal: 'Royal',
  major: 'Major',
  variant: 'Variant',
  minor: 'Minor',
  pawn: 'Pawn',
}

function createPawnLine(size: number, count = size): (DeckPieceType | null)[] {
  return Array.from({ length: size }, (_, index) => (index < count ? 'pawn' : null))
}

function createPresetLayout(
  backline: (DeckPieceType | null)[],
  pawnCount = backline.length,
  pocket: Partial<Record<DeckPieceType, number>> = {},
): DeckPresetLayout {
  return {
    backline,
    pawns: createPawnLine(backline.length, pawnCount),
    pocket,
  }
}

export const deckPresets: DeckPreset[] = [
  {
    id: 'classic',
    name: '기본 체스 덱',
    description: '익숙한 기물 중심의 표준 배치입니다.',
    layouts: {
      8: createPresetLayout(['rook', 'knight', 'bishop', 'queen', 'king', 'bishop', 'knight', 'rook']),
      9: createPresetLayout(['rook', 'knight', 'bishop', 'queen', 'king', 'queen', 'bishop', 'knight', 'rook']),
      10: createPresetLayout(['rook', 'knight', 'bishop', 'queen', 'bishop', 'king', 'queen', 'bishop', 'knight', 'rook']),
      11: createPresetLayout(['rook', 'knight', 'bishop', 'rook', 'queen', 'king', 'queen', 'rook', 'bishop', 'knight', 'rook']),
      12: createPresetLayout(['rook', 'knight', 'bishop', 'rook', 'queen', 'bishop', 'king', 'queen', 'rook', 'bishop', 'knight', 'rook']),
    },
  },
  {
    id: 'swarm',
    name: '물량 덱',
    description: '낮은 점수 기물과 Pawn을 많이 쓰는 덱입니다.',
    layouts: {
      8: createPresetLayout(['knight', 'knight', 'knight', 'king', 'knight', 'knight', 'knight', 'knight'], 8, { pawn: 10 }),
      9: createPresetLayout(['knight', 'knight', 'knight', 'knight', 'king', 'knight', 'knight', 'knight', 'knight'], 9, { pawn: 14 }),
      10: createPresetLayout(['knight', 'knight', 'knight', 'knight', 'king', 'knight', 'knight', 'knight', 'knight', 'knight'], 10, { pawn: 18 }),
      11: createPresetLayout(['knight', 'knight', 'knight', 'knight', 'knight', 'king', 'knight', 'knight', 'knight', 'knight', 'knight'], 11, { pawn: 24 }),
      12: createPresetLayout(['knight', 'knight', 'knight', 'knight', 'knight', 'king', 'knight', 'knight', 'knight', 'knight', 'knight', 'knight'], 12, { pawn: 30 }),
    },
  },
  {
    id: 'pocket',
    name: '포켓 덱',
    description: '시작 기물을 줄이고 포켓 운용을 늘린 덱입니다.',
    layouts: {
      8: createPresetLayout([null, null, null, 'king', null, null, null, null], 4, { rook: 2, bishop: 2, knight: 2, queen: 1, pawn: 4 }),
      9: createPresetLayout([null, null, null, null, 'king', null, null, null, null], 5, { rook: 2, bishop: 2, knight: 2, queen: 1, pawn: 8 }),
      10: createPresetLayout([null, null, null, null, 'king', null, null, null, null, null], 6, { rook: 2, bishop: 2, knight: 2, queen: 2, pawn: 10 }),
      11: createPresetLayout([null, null, null, null, null, 'king', null, null, null, null, null], 7, { rook: 3, bishop: 2, knight: 2, queen: 2, pawn: 14 }),
      12: createPresetLayout([null, null, null, null, null, 'king', null, null, null, null, null, null], 8, { rook: 3, bishop: 3, knight: 3, queen: 2, amazon: 1, pawn: 18 }),
    },
  },
]

export function scoreLimit(boardSize: number): number {
  return boardSize * boardSize - 25
}

export function emptyPocket(): Record<DeckPieceType, number> {
  return Object.fromEntries(pocketCatalog.map(piece => [piece.id, 0]))
}

export function pieceScore(pieceType: DeckPieceType): number {
  return pieceCatalog.find(piece => piece.id === pieceType)?.score ?? 0
}

export function canUseInPocket(pieceType: DeckPieceType): boolean {
  return pieceCatalog.find(piece => piece.id === pieceType)?.canPocket === true
}

export function pieceLabel(pieceType: DeckPieceType): string {
  return pieceCatalog.find(piece => piece.id === pieceType)?.name ?? pieceType
}

export function isUniqueStartingPiece(pieceType: DeckPieceType): boolean {
  return pieceCatalog.find(piece => piece.id === pieceType)?.uniqueStarting === true
}

export function totalPocketCount(deck: LobbyDeck): number {
  return Object.values(deck.pocket).reduce((sum, count) => sum + count, 0)
}

export function presetLayoutForBoard(preset: DeckPreset, boardSize: number): DeckPresetLayout | null {
  return preset.layouts[boardSize] ?? null
}

export function createPresetStarting(boardSize: number, layout: DeckPresetLayout): LobbyPlacement[] {
  const offset = Math.max(0, Math.floor((boardSize - layout.backline.length) / 2))

  return [
    ...layout.backline
      .map((pieceType, index) => {
        const file = offset + index
        return pieceType && file < boardSize ? { pieceType, square: { file, rank: 0 } } : null
      })
      .filter((placement): placement is LobbyPlacement => placement !== null),
    ...layout.pawns
      .map((pieceType, index) => {
        const file = offset + index
        return pieceType && file < boardSize ? { pieceType, square: { file, rank: 1 } } : null
      })
      .filter((placement): placement is LobbyPlacement => placement !== null),
  ]
}

export function createPresetDeck(boardSize: number, presetId = 'classic'): LobbyDeck {
  const preset = deckPresets.find(entry => entry.id === presetId) ?? deckPresets[0]
  const layout = presetLayoutForBoard(preset, boardSize)
  const pocket = emptyPocket()

  if (layout) {
    for (const [pieceType, count] of Object.entries(layout.pocket)) {
      pocket[pieceType] = count ?? 0
    }
  }

  return {
    starting: layout ? createPresetStarting(boardSize, layout) : [],
    pocket,
  }
}

export function calculateDeckScore(deck: LobbyDeck): number {
  return deck.starting.reduce((sum, piece) => sum + pieceScore(piece.pieceType), 0)
    + Object.entries(deck.pocket).reduce((sum, [pieceType, count]) => sum + pieceScore(pieceType) * count, 0)
}

function isInBaseZone(piece: LobbyPlacement, boardSize: number): boolean {
  return piece.square.file >= 0
    && piece.square.file < boardSize
    && (piece.square.rank === 0 || piece.square.rank === 1)
}

export function validateLobbyDeck(deck: LobbyDeck, boardSize: number, name = '덱'): DeckSummary {
  const totalScore = calculateDeckScore(deck)
  const limit = scoreLimit(boardSize)
  const errors: string[] = []
  const normalizedName = name.trim()

  if (!normalizedName) {
    errors.push('덱 이름은 비어 있을 수 없습니다.')
  }

  const kingCount = deck.starting.filter(piece => piece.pieceType === 'king').length
  if (kingCount !== 1) {
    errors.push('King은 시작 기물에 정확히 1개 있어야 합니다.')
  }

  if ((deck.pocket.king ?? 0) > 0) {
    errors.push('King은 포켓에 들어갈 수 없습니다.')
  }

  if (totalScore > limit) {
    errors.push(`덱 점수가 제한 점수보다 ${totalScore - limit}점 높습니다.`)
  }

  if (deck.starting.some(piece => !isInBaseZone(piece, boardSize))) {
    errors.push('시작 기물은 해당 보드 크기의 기본 진영 안에만 배치할 수 있습니다.')
  }
  const usedTypes = [
    ...deck.starting.map(piece => piece.pieceType),
    ...Object.entries(deck.pocket).filter(([, count]) => count > 0).map(([pieceType]) => pieceType),
  ]
  for (const pieceType of new Set(usedTypes)) {
    const catalogPiece = pieceCatalog.find(piece => piece.id === pieceType)
    if (!catalogPiece) {
      errors.push(`사용할 수 없는 기물 버전입니다: ${pieceType}`)
    } else if (catalogPiece.custom && !catalogPiece.custom.active) {
      errors.push(`${catalogPiece.name} v${catalogPiece.custom.version}은 비활성화되어 새 게임에 사용할 수 없습니다.`)
    }
  }

  return {
    totalScore,
    scoreLimit: limit,
    valid: errors.length === 0,
    errors,
  }
}

export function validateSavedDeck(deck: SavedDeck): DeckSummary {
  return validateLobbyDeck(deck, deck.boardSize, deck.name)
}

export function validateDeckForGame(deck: SavedDeck, boardSize: number): DeckSummary {
  const summary = validateSavedDeck(deck)
  if (deck.boardSize !== boardSize) {
    return {
      ...summary,
      valid: false,
      errors: [...summary.errors, '덱의 보드 크기와 게임의 보드 크기가 다릅니다.'],
    }
  }

  return summary
}
