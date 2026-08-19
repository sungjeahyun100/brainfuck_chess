use std::sync::Arc;

use brainfuck_chess_engine::types::GameState;
use dashmap::DashMap;

use crate::MultiplayerRoom;
use crate::{account::AccountRepository, custom_piece::CustomPieceRepository};

pub(crate) type GameStore = Arc<DashMap<String, GameState>>;
pub(crate) type RoomStore = Arc<DashMap<String, MultiplayerRoom>>;
pub(crate) type CustomPieceStore = Arc<dyn CustomPieceRepository>;
pub(crate) type AccountStore = Arc<dyn AccountRepository>;
