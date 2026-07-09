import type {
  ActionEffect,
  ActionTimelineFrame,
  GameState,
  PieceId,
  Square,
} from '../types/game'

function cloneGameState(state: GameState): GameState {
  return JSON.parse(JSON.stringify(state)) as GameState
}

function squareId(square: Square): string {
  return `${square.file}_${square.rank}`
}

function removePieceFromBoard(state: GameState, pieceId: PieceId) {
  for (const [id, occupant] of Object.entries(state.board.squares)) {
    if (occupant === pieceId) state.board.squares[id] = null
  }
}

function applyEffect(state: GameState, effect: ActionEffect) {
  switch (effect.type) {
    case 'capture_piece': {
      const piece = state.pieces[effect.piece_id]
      if (state.board.squares[squareId(effect.at)] === effect.piece_id) {
        state.board.squares[squareId(effect.at)] = null
      } else {
        removePieceFromBoard(state, effect.piece_id)
      }
      if (piece) {
        piece.captured = true
        piece.current_square = undefined
        const capturedPieces = state.players[piece.owner]?.captured_pieces
        if (capturedPieces && !capturedPieces.includes(effect.piece_id)) {
          capturedPieces.push(effect.piece_id)
        }
      }
      return
    }
    case 'move_piece': {
      removePieceFromBoard(state, effect.piece_id)
      state.board.squares[squareId(effect.to)] = effect.piece_id
      const piece = state.pieces[effect.piece_id]
      if (piece) {
        piece.current_square = effect.to
        piece.has_moved = true
      }
      return
    }
    case 'drop_piece': {
      removePieceFromBoard(state, effect.piece_id)
      state.board.squares[squareId(effect.to)] = effect.piece_id
      const piece = state.pieces[effect.piece_id]
      if (piece) {
        piece.in_pocket = false
        piece.current_square = effect.to
        const player = state.players[piece.owner]
        if (player) {
          player.deck.pocket_pieces = player.deck.pocket_pieces
            .filter(pieceId => pieceId !== effect.piece_id)
        }
      }
      return
    }
    case 'promote_piece': {
      const piece = state.pieces[effect.piece_id]
      if (piece) piece.type_id = effect.to_type
      return
    }
    case 'swap_pieces': {
      const first = state.pieces[effect.first_piece_id]
      const second = state.pieces[effect.second_piece_id]
      removePieceFromBoard(state, effect.first_piece_id)
      removePieceFromBoard(state, effect.second_piece_id)
      state.board.squares[squareId(effect.first_to)] = effect.first_piece_id
      state.board.squares[squareId(effect.second_to)] = effect.second_piece_id
      if (first) {
        first.current_square = effect.first_to
        first.has_moved = true
      }
      if (second) {
        second.current_square = effect.second_to
        second.has_moved = true
      }
      return
    }
    case 'set_piece_ability': {
      const piece = state.pieces[effect.piece_id]
      if (piece) {
        const definition = state.piece_definitions[piece.type_id]
        const ability = definition?.abilities?.find(candidate => candidate.id === effect.ability_id)
        piece.active_ability = {
          ability_id: effect.ability_id,
          activated_turn_number: state.turn_number,
          activated_player: state.current_player,
          duration: ability?.duration ?? 'permanent',
        }
      }
      return
    }
    case 'clear_piece_ability': {
      const piece = state.pieces[effect.piece_id]
      if (piece?.active_ability?.ability_id === effect.ability_id) {
        piece.active_ability = null
      }
      return
    }
    case 'set_ability_cooldown': {
      const piece = state.pieces[effect.piece_id]
      if (piece) {
        piece.ability_cooldowns ??= {}
        piece.ability_cooldowns[effect.ability_id] = effect.usable_turn
      }
      return
    }
    case 'set_en_passant':
      state.en_passant_target = effect.target
      state.en_passant_available_to = effect.available_to
      return
    case 'advance_turn':
      state.current_player = effect.to_player
      state.turn_number = effect.turn_number
      state.turn_state = { mode: 'undecided', actions: [] }
      return
    case 'end_game':
      state.phase = 'ended'
      state.result = effect.result
  }
}

export function useActionTimeline() {
  function applyTimelineFrame(state: GameState, frame: ActionTimelineFrame): GameState {
    const next = cloneGameState(state)
    if (frame.action.type === 'move' || frame.action.type === 'drop') {
      next.turn_state.mode = frame.action.type
      next.turn_state.actions.push(frame.action)
    }
    for (const effect of frame.effects) applyEffect(next, effect)
    return next
  }

  return { applyTimelineFrame }
}
