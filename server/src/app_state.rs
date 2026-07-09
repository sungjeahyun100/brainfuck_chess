use crate::stores::game_store::{new_game_store, GameStore};
use crate::stores::room_store::{new_room_store, RoomStore};

#[derive(Clone)]
pub struct AppState {
    pub games: GameStore,
    pub rooms: RoomStore,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            games: new_game_store(),
            rooms: new_room_store(),
        }
    }
}
