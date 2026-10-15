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
  const pieceId = (key: string) => key.slice(key.indexOf(':') + 1)
  const layer = (key: string) => key.slice(0, key.indexOf(':'))
  const previousPieces = new Map(Object.entries(before).map(([key, square]) => [pieceId(key), { key, square }]))
  const nextPieces = new Set(Object.keys(after).map(pieceId))
  const previousSquares = new Map(Object.entries(before).map(([key, square]) => [`${layer(key)}:${square}`, key]))
  const capturedIds = new Set<string>()
  for (const [key, destination] of Object.entries(after)) {
    const previous = previousPieces.get(pieceId(key))
    const to = parseSquare(destination)
    if (!to || (previous?.key === key && previous.square === destination)) continue
    if (!previous) { events.push({ type: 'drop', pieceId: pieceId(key), to }); continue }
    const from = parseSquare(previous.square)
    if (!from) continue
    const occupant = previousSquares.get(`${layer(key)}:${destination}`)
    const captured = occupant && pieceId(occupant) !== pieceId(key) && !nextPieces.has(pieceId(occupant))
    if (captured && occupant) capturedIds.add(pieceId(occupant))
    events.push({ type: captured ? 'capture' : 'move', pieceId: pieceId(key), from, to })
  }
  for (const [key, location] of Object.entries(before)) {
    if (!nextPieces.has(pieceId(key)) && !capturedIds.has(pieceId(key))) {
      const at = parseSquare(location)
      if (at) events.push({ type: 'death', at })
    }
  }
  return events
}
