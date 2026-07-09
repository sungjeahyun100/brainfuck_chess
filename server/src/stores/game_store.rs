use std::sync::Arc;

use brainfuck_chess_engine::types::GameState;
use dashmap::DashMap;

pub type GameStore = Arc<DashMap<String, GameState>>;

pub fn new_game_store() -> GameStore {
    Arc::new(DashMap::new())
}
