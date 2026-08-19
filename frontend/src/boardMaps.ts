import type { BoardMapId, BoardVariant } from './types/game'

export interface BoardMapDefinition {
  id: BoardMapId
  name: string
  description: string
  boardSize: number
  variant: BoardVariant
}

export const boardMaps: readonly BoardMapDefinition[] = [
  { id: 'standard-8x8', name: '8×8 일반전', description: '지형이 없는 8×8 보드입니다.', boardSize: 8, variant: 'plain' },
  { id: 'standard-9x9', name: '9×9 일반전', description: '지형이 없는 9×9 보드입니다.', boardSize: 9, variant: 'plain' },
  { id: 'standard-10x10', name: '10×10 일반전', description: '지형이 없는 10×10 보드입니다.', boardSize: 10, variant: 'plain' },
  { id: 'standard-11x11', name: '11×11 일반전', description: '지형이 없는 11×11 보드입니다.', boardSize: 11, variant: 'plain' },
  { id: 'standard-12x12', name: '12×12 일반전', description: '지형이 없는 12×12 보드입니다.', boardSize: 12, variant: 'plain' },
  {
    id: 'central-high-ground-12x12',
    name: '12×12 고지전',
    description: '중앙 네 칸이 고지인 12×12 전용 맵입니다.',
    boardSize: 12,
    variant: 'central-high-ground',
  },
]

export function findBoardMap(mapId: string | undefined): BoardMapDefinition | undefined {
  return boardMaps.find(map => map.id === mapId)
}

export function standardMapId(boardSize: number): BoardMapId | null {
  return findBoardMap(`standard-${boardSize}x${boardSize}`)?.id ?? null
}

export function normalizeBoardMapId(mapId: unknown, boardSize: number): BoardMapId | null {
  if (typeof mapId === 'string') {
    const map = findBoardMap(mapId)
    if (map?.boardSize === boardSize) return map.id
  }
  return standardMapId(boardSize)
}

export function boardMapLabel(mapId: string): string {
  return findBoardMap(mapId)?.name ?? mapId
}
