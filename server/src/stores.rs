use std::sync::Arc;

use brainfuck_chess_engine::types::GameState;
use dashmap::DashMap;

use crate::custom_piece::InMemoryCustomPieceRepository;
use crate::MultiplayerRoom;

pub(crate) type GameStore = Arc<DashMap<String, GameState>>;
pub(crate) type RoomStore = Arc<DashMap<String, MultiplayerRoom>>;
pub(crate) type CustomPieceStore = Arc<InMemoryCustomPieceRepository>;
