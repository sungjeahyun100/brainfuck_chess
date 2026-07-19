use std::sync::Arc;

use brainfuck_chess_engine::types::GameState;
use dashmap::DashMap;

use crate::MultiplayerRoom;

pub(crate) type GameStore = Arc<DashMap<String, GameState>>;
pub(crate) type RoomStore = Arc<DashMap<String, MultiplayerRoom>>;
