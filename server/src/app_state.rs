use crate::stores::{GameStore, RoomStore};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) games: GameStore,
    pub(crate) rooms: RoomStore,
}

impl AppState {
    pub(crate) fn in_memory() -> Self {
        Self {
            games: Default::default(),
            rooms: Default::default(),
        }
    }
}
