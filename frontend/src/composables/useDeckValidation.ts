import { reactive } from 'vue'
import type {
  DeckPreset,
  DeckPresetLayout,
  DeckPieceType,
  DeckSummary,
  LobbyDeck,
  LobbyPlacement,
  PieceCatalogItem,
  PieceCatalogMetadata,
  SavedDeck,
} from '../types/deck'
import type { CustomPieceRecord } from '../types/customPiece'
import { parseCustomPiecePackage } from './useCustomPieceDraft.ts'
import { findBoardMap, normalizeBoardMapId } from '../boardMaps.ts'

export const boardSizes = [8, 9, 10, 11, 12] as const

export function baseZoneDepth(boardSize: number): number {
  return boardSize >= 10 ? 3 : 2
}

export type SetupSide = 'white' | 'black'

export function baseZoneRanks(boardSize: number, side: SetupSide = 'white'): number[] {
  const depth = baseZoneDepth(boardSize)
  return Array.from(
    { length: depth },
    (_, index) => side === 'white' ? index : boardSize - 1 - index,
  )
}

export function frontmostBaseRank(boardSize: number, side: SetupSide = 'white'): number {
  const forward = side === 'white' ? 1 : -1
  return baseZoneRanks(boardSize, side)
    .reduce((front, rank) => rank * forward > front * forward ? rank : front)
}

const builtInPieceCatalog: Omit<PieceCatalogItem, 'deploymentZone'>[] = [
  { id: 'king', name: 'King', score: 0, category: 'royal', canPocket: false, uniqueStarting: true },
  { id: 'queen', name: 'Queen', score: 0, category: 'major', canPocket: true },
  { id: 'cannon-rook', name: 'Cannon Rook', score: 0, category: 'variant', canPocket: true, aliases: ['cannon', 'po rook', '포 룩'] },
  { id: 'amazon', name: 'Amazon', score: 0, category: 'variant', canPocket: true },
  { id: 'guhang', name: '구행', score: 0, category: 'variant', canPocket: true, aliases: ['Guhang'] },
  { id: 'tempest-queen', name: 'Tempest Queen', score: 0, category: 'variant', canPocket: true, aliases: ['storm queen'] },
  { id: 'tempest-rook', name: 'Tempest Rook', score: 0, category: 'variant', canPocket: true, aliases: ['storm rook'] },
  { id: 'tempest-bishop', name: 'Tempest Bishop', score: 0, category: 'variant', canPocket: true, aliases: ['storm bishop'] },
  { id: 'tempest-knight', name: 'Tempest Knight', score: 0, category: 'variant', canPocket: true, aliases: ['storm knight'] },
  { id: 'bouncing-bishop', name: 'Bouncing Bishop', score: 0, category: 'variant', canPocket: true, aliases: ['bounce bishop'] },
  { id: 'bouncing-rook', name: 'Bouncing Rook', score: 0, category: 'variant', canPocket: true, aliases: ['bounce rook'] },
  { id: 'bouncing-queen', name: 'Bouncing Queen', score: 0, category: 'variant', canPocket: true, aliases: ['bounce queen'] },
  { id: 'bouncing-pawn', name: 'Bouncing Pawn', score: 0, category: 'pawn', canPocket: true, aliases: ['bounce pawn'] },
  { id: 'windmill', name: 'Windmill', score: 0, category: 'variant', canPocket: true, aliases: ['풍차'] },
  { id: 'tempest-pawn', name: 'Tempest Pawn', score: 0, category: 'pawn', canPocket: true, aliases: ['storm pawn'] },
  { id: 'dozer', name: 'Dozer', score: 0, category: 'pawn', canPocket: true, aliases: ['도저'] },
  { id: 'rook', name: 'Rook', score: 0, category: 'major', canPocket: true },
  { id: 'bishop', name: 'Bishop', score: 0, category: 'minor', canPocket: true },
  { id: 'knight', name: 'Knight', score: 0, category: 'minor', canPocket: true },
  { id: 'nightrider', name: 'Nightrider', score: 0, category: 'variant', canPocket: true, aliases: ['나이트라이더'] },
  { id: 'paratrooper', name: '공수부대 대원', score: 0, category: 'variant', canPocket: true, aliases: ['Paratrooper', '공수부대'] },
  { id: 'alternating-soldier', name: '교대병', score: 0, category: 'variant', canPocket: true },
  { id: 'airborne', name: '공수부대', score: 0, category: 'variant', canPocket: true },
  { id: 'green-camp', name: '그린캠프', score: 0, category: 'variant', canPocket: true },
  { id: 'mortar', name: '박격포병', score: 0, category: 'variant', canPocket: true },
  { id: 'tank', name: '탱크', score: 0, category: 'variant', canPocket: true },
  { id: 'bomber', name: '폭격기', score: 0, category: 'variant', canPocket: true },
  { id: 'machine-gunner', name: '기관총 사수', score: 0, category: 'variant', canPocket: true },
  { id: 'surface-to-air-missile', name: '지대공 미사일', score: 0, category: 'variant', canPocket: true, aliases: ['SAM', '격추'] },
  { id: 'pawn', name: 'Pawn', score: 0, category: 'pawn', canPocket: true },
]

export const pieceCatalog = reactive<PieceCatalogItem[]>(
  builtInPieceCatalog.map(piece => ({ ...piece, deploymentZone: 'back' })),
)

const archivedCustomCatalog = new Map<DeckPieceType, PieceCatalogItem>()

export const pocketCatalog = reactive<PieceCatalogItem[]>(pieceCatalog.filter(piece => piece.canPocket))

function syncPocketCatalog(): void {
  pocketCatalog.splice(0, pocketCatalog.length, ...pieceCatalog.filter(piece => piece.canPocket))
}

function rememberCustomCatalogItem(item: PieceCatalogItem): void {
  if (item.custom) archivedCustomCatalog.set(item.id, item)
}

export function findPieceCatalogItem(pieceType: DeckPieceType): PieceCatalogItem | undefined {
  return pieceCatalog.find(piece => piece.id === pieceType) ?? archivedCustomCatalog.get(pieceType)
}

/** Maps direction-specific engine IDs back to the neutral catalog entry used by the UI. */
export function neutralPieceCatalogId(pieceType: DeckPieceType): DeckPieceType {
  if (findPieceCatalogItem(pieceType)) return pieceType
  for (const suffix of ['-white', '-black']) {
    if (!pieceType.endsWith(suffix)) continue
    const neutral = pieceType.slice(0, -suffix.length)
    if (findPieceCatalogItem(neutral)) return neutral
  }
  return pieceType
}

export function customDeckPieceType(piece: Pick<CustomPieceRecord, 'id' | 'version' | 'exposed_piece_key'>): string {
  return `custom:${piece.id}:v${piece.version}:${piece.exposed_piece_key}`
}

function customCatalogItem(record: CustomPieceRecord): PieceCatalogItem {
  let deploymentZone: PieceCatalogItem['deploymentZone'] = 'back'
  try {
    const document = parseCustomPiecePackage(record.raw_script)
    deploymentZone = document.definitions.find(
      definition => definition.id === record.exposed_piece_key,
    )?.deployment_zone ?? 'back'
  } catch {
    // The server has already validated stored packages. Archived legacy test
    // fixtures and records without this metadata remain safely back-only.
  }
  return {
    id: customDeckPieceType(record),
    name: record.name,
    score: record.score,
    category: 'custom',
    canPocket: true,
    deploymentZone,
    aliases: [record.description, record.exposed_piece_key],
    custom: {
      id: record.id,
      version: record.version,
      contentHash: record.content_hash,
      exposedPieceKey: record.exposed_piece_key,
      image: record.image,
      assetKey: record.resolved_image_asset_key,
      active: record.active,
    },
  }
}

export function replaceCustomPieceCatalog(records: CustomPieceRecord[]): void {
  archivedCustomCatalog.clear()
  for (let index = pieceCatalog.length - 1; index >= 0; index -= 1) {
    if (pieceCatalog[index].custom) pieceCatalog.splice(index, 1)
  }

  const latestByPieceId = new Map<string, PieceCatalogItem>()
  for (const record of records) {
    const item = customCatalogItem(record)
    rememberCustomCatalogItem(item)
    const current = latestByPieceId.get(record.id)
    if (!current || current.custom!.version < record.version) {
      latestByPieceId.set(record.id, item)
    }
  }
  pieceCatalog.push(...latestByPieceId.values())
  syncPocketCatalog()
}

export function upsertCustomPieceCatalog(record: CustomPieceRecord): void {
  for (let index = pieceCatalog.length - 1; index >= 0; index -= 1) {
    if (pieceCatalog[index].custom?.id === record.id) {
      rememberCustomCatalogItem(pieceCatalog[index])
      pieceCatalog.splice(index, 1)
    }
  }
  const item = customCatalogItem(record)
  rememberCustomCatalogItem(item)
  pieceCatalog.push(item)
  syncPocketCatalog()
}

export function deactivateCustomPieceCatalog(id: string): void {
  for (const piece of pieceCatalog) {
    if (piece.custom?.id === id) piece.custom.active = false
  }
  for (const piece of archivedCustomCatalog.values()) {
    if (piece.custom?.id === id) piece.custom.active = false
  }
  syncPocketCatalog()
}

export function applyPieceScores(scores: Record<string, number>): void {
  for (const piece of pieceCatalog) {
    if (piece.custom) continue
    const score = scores[piece.id]
    if (!Number.isInteger(score) || score < 0) {
      throw new Error(`엔진 기물 점수가 누락되었거나 잘못되었습니다: ${piece.id}`)
    }
    piece.score = score
  }
}

export function applyPieceMetadata(metadata: Record<string, PieceCatalogMetadata>): void {
  for (const piece of pieceCatalog) {
    if (piece.custom) continue
    const definition = metadata[piece.id]
    if (
      !definition
      || !Number.isInteger(definition.score)
      || definition.score < 0
      || !['front', 'back'].includes(definition.deployment_zone)
    ) {
      throw new Error(`엔진 기물 정보가 누락되었거나 잘못되었습니다: ${piece.id}`)
    }
    piece.score = definition.score
    piece.deploymentZone = definition.deployment_zone
  }
}

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
      8: createPresetLayout([null, null, null, 'king', null, null, null, null], 8, { rook: 2, bishop: 2, knight: 2, queen: 1 }),
      9: createPresetLayout([null, null, null, null, 'king', null, null, null, null], 9, { rook: 2, bishop: 2, knight: 2, queen: 1, pawn: 4 }),
      10: createPresetLayout([null, null, null, null, 'king', null, null, null, null, null], 10, { rook: 2, bishop: 2, knight: 2, queen: 2, pawn: 6 }),
      11: createPresetLayout([null, null, null, null, null, 'king', null, null, null, null, null], 11, { rook: 3, bishop: 2, knight: 2, queen: 2, pawn: 10 }),
      12: createPresetLayout([null, null, null, null, null, 'king', null, null, null, null, null, null], 12, { rook: 3, bishop: 3, knight: 3, queen: 2, amazon: 1, pawn: 14 }),
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
  return findPieceCatalogItem(pieceType)?.score ?? 0
}

export function canUseInPocket(pieceType: DeckPieceType): boolean {
  return findPieceCatalogItem(pieceType)?.canPocket === true
}

export function pieceLabel(pieceType: DeckPieceType): string {
  return findPieceCatalogItem(pieceType)?.name ?? pieceType
}

export function isUniqueStartingPiece(pieceType: DeckPieceType): boolean {
  return findPieceCatalogItem(pieceType)?.uniqueStarting === true
}

export function totalPocketCount(deck: LobbyDeck): number {
  return Object.values(deck.pocket).reduce((sum, count) => sum + count, 0)
}

export function presetLayoutForBoard(preset: DeckPreset, boardSize: number): DeckPresetLayout | null {
  return preset.layouts[boardSize] ?? null
}

export function createPresetStarting(boardSize: number, layout: DeckPresetLayout): LobbyPlacement[] {
  const offset = Math.max(0, Math.floor((boardSize - layout.backline.length) / 2))
  const frontRank = frontmostBaseRank(boardSize)

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
        return pieceType && file < boardSize ? { pieceType, square: { file, rank: frontRank } } : null
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
    + Object.entries(deck.pocket).reduce(
      (sum, [pieceType, count]) => sum + (Number.isInteger(count) && count >= 0 ? pieceScore(pieceType) * count : 0),
      0,
    )
}

function isInBaseZone(piece: LobbyPlacement, boardSize: number): boolean {
  return piece.square.file >= 0
    && piece.square.file < boardSize
    && piece.square.rank >= 0
    && piece.square.rank < baseZoneDepth(boardSize)
}

export function placementRestriction(
  pieceType: DeckPieceType,
  rank: number,
  boardSize: number,
  side: SetupSide = 'white',
): string | null {
  const piece = findPieceCatalogItem(pieceType)
  if (!piece) return '기물의 초기 배치 정보를 찾을 수 없습니다.'
  const isFrontRank = rank === frontmostBaseRank(boardSize, side)
  if (piece.deploymentZone === 'front' && !isFrontRank) {
    return '이 기물은 가장 앞쪽 시작 배치 줄에만 배치할 수 있습니다.'
  }
  if (piece.deploymentZone === 'back' && isFrontRank) {
    return '이 기물은 가장 앞쪽 시작 배치 줄에 배치할 수 없습니다.'
  }
  return null
}

export function canPieceBePlacedAtStart(
  pieceType: DeckPieceType,
  rank: number,
  boardSize: number,
  side: SetupSide = 'white',
): boolean {
  return baseZoneRanks(boardSize, side).includes(rank)
    && placementRestriction(pieceType, rank, boardSize, side) === null
}

export function validateLobbyDeck(deck: LobbyDeck, boardSize: number, name = '덱'): DeckSummary {
  const totalScore = calculateDeckScore(deck)
  const limit = scoreLimit(boardSize)
  const errors: string[] = []
  const normalizedName = name.trim()

  if (!normalizedName) {
    errors.push('덱 이름은 비어 있을 수 없습니다.')
  }

  if (!(boardSizes as readonly number[]).includes(boardSize)) {
    errors.push('지원하지 않는 보드 크기입니다.')
  }

  const occupiedSquares = new Set<string>()
  for (const piece of deck.starting) {
    if (!Number.isInteger(piece.square.file) || !Number.isInteger(piece.square.rank)) {
      errors.push('시작 기물 좌표는 정수여야 합니다.')
      continue
    }
    const squareKey = `${piece.square.file}:${piece.square.rank}`
    if (occupiedSquares.has(squareKey)) errors.push('같은 칸에 여러 시작 기물을 배치할 수 없습니다.')
    occupiedSquares.add(squareKey)
  }

  const kingCount = deck.starting.filter(piece => piece.pieceType === 'king').length
  if (kingCount !== 1) {
    errors.push('King은 시작 기물에 정확히 1개 있어야 합니다.')
  }

  if ((deck.pocket.king ?? 0) > 0) {
    errors.push('King은 포켓에 들어갈 수 없습니다.')
  }

  for (const [pieceType, count] of Object.entries(deck.pocket)) {
    if (!Number.isInteger(count) || count < 0) {
      errors.push(`${pieceLabel(pieceType)}의 포켓 수량이 올바르지 않습니다.`)
    }
    if (count > 0 && !canUseInPocket(pieceType)) {
      errors.push(`${pieceLabel(pieceType)}은 포켓에 넣을 수 없습니다.`)
    }
  }

  if (totalScore > limit) {
    errors.push(`덱 점수가 제한 점수보다 ${totalScore - limit}점 높습니다.`)
  }

  if (deck.starting.some(piece => !isInBaseZone(piece, boardSize))) {
    errors.push('시작 기물은 해당 보드 크기의 기본 진영 안에만 배치할 수 있습니다.')
  }
  for (const piece of deck.starting) {
    const restriction = placementRestriction(piece.pieceType, piece.square.rank, boardSize)
    if (restriction) {
      errors.push(`${pieceLabel(piece.pieceType)} (${piece.square.file + 1}, ${piece.square.rank + 1}): ${restriction}`)
    }
  }
  const frontRank = frontmostBaseRank(boardSize)
  const occupiedFrontFiles = new Set(
    deck.starting
      .filter(piece => piece.square.rank === frontRank && piece.square.file >= 0 && piece.square.file < boardSize)
      .map(piece => piece.square.file),
  )
  if (occupiedFrontFiles.size !== boardSize) {
    errors.push(`덱의 앞줄은 모든 칸에 기물이 배치되어야 합니다. (${occupiedFrontFiles.size}/${boardSize})`)
  }
  for (const pieceType of new Set(deck.starting.map(piece => piece.pieceType))) {
    if (isUniqueStartingPiece(pieceType) && deck.starting.filter(piece => piece.pieceType === pieceType).length > 1) {
      errors.push(`${pieceLabel(pieceType)}은 시작 기물에 1개만 배치할 수 있습니다.`)
    }
  }
  const usedTypes = [
    ...deck.starting.map(piece => piece.pieceType),
    ...Object.entries(deck.pocket).filter(([, count]) => count > 0).map(([pieceType]) => pieceType),
  ]
  for (const pieceType of new Set(usedTypes)) {
    const catalogPiece = findPieceCatalogItem(pieceType)
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
  const normalizedMapId = normalizeBoardMapId(deck.mapId, deck.boardSize)
  const map = normalizedMapId ? findBoardMap(normalizedMapId) : null
  const summary = validateLobbyDeck(deck, deck.boardSize, deck.name)
  if (!map || map.boardSize !== deck.boardSize) {
    return {
      ...summary,
      valid: false,
      errors: [...summary.errors, '덱의 전용 맵 정보가 올바르지 않습니다.'],
    }
  }
  return summary
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
