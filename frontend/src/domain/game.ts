import type { PlayerId, Square } from '../types/game'

export interface PocketGroup {
  typeId: string
  name: string
  representativeId: string
  pieceIds: string[]
  count: number
}

export interface PromotionRequest {
  pieceId: string
  to: Square
  owner: PlayerId
  options: string[]
}
