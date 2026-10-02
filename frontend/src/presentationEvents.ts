import type { Board, Square } from './types/game'

export type ExplosionPreset = 'small' | 'artillery'
export type ProjectilePreset = 'bullet' | 'magic'

export type PresentationEvent =
  | { type: 'move' | 'capture'; pieceId: string; from: Square; to: Square }
  | { type: 'drop' | 'summon'; pieceId: string; to: Square }
  | { type: 'death' | 'sacrifice' | 'ability_hit'; at: Square }
  | { type: 'explosion'; at: Square; preset?: ExplosionPreset; delayMs?: number }
  | { type: 'projectile'; from: Square; to: Square; preset?: ProjectilePreset }
  | { type: 'ability_start' | 'screen_dim' | 'screen_restore' }
  | { type: 'victory' | 'defeat' }

export type Occupancy = Record<string, string>

export function boardOccupancy(board: Board): Occupancy {
  const result: Occupancy = {}
  for (const [layer, squares] of Object.entries({ ground: board.squares, air: board.air_squares ?? {} })) {
    for (const [square, pieceId] of Object.entries(squares)) {
      if (pieceId) result[`${layer}:${pieceId}`] = square
    }
  }
  return result
}

function parseSquare(id: string): Square | null {
  const parts = id.split('_').map(Number)
  return parts.length === 2 && parts.every(Number.isInteger) ? { file: parts[0], rank: parts[1] } : null
}

/** Derive visual hints from committed occupancy only; never changes game state. */
export function presentationEvents(before: Occupancy, after: Occupancy): PresentationEvent[] {
  const events: PresentationEvent[] = []
  const previousSquares = new Map(Object.entries(before).map(([key, value]) => [key.split(':')[0] + ':' + value, key]))
  for (const [key, destination] of Object.entries(after)) {
    const fromId = before[key]
    const to = parseSquare(destination)
    if (!to || fromId === destination) continue
    if (!fromId) { events.push({ type: 'drop', pieceId: key.split(':').slice(1).join(':'), to }); continue }
    const from = parseSquare(fromId)
    if (!from) continue
    const occupant = previousSquares.get(key.split(':')[0] + ':' + destination)
    events.push({ type: occupant && occupant !== key && !after[occupant] ? 'capture' : 'move', pieceId: key.split(':').slice(1).join(':'), from, to })
  }
  for (const [key, location] of Object.entries(before)) {
    if (!after[key] && !events.some(event => event.type === 'capture' && `${key.split(':')[0]}:${location}` === `${key.split(':')[0]}:${event.to.file}_${event.to.rank}`)) {
      const at = parseSquare(location)
      if (at) events.push({ type: 'death', at })
    }
  }
  return events
}
