use crate::stores::{CustomPieceStore, GameStore, RoomStore};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) games: GameStore,
    pub(crate) rooms: RoomStore,
    pub(crate) custom_pieces: CustomPieceStore,
}

impl AppState {
    pub(crate) fn in_memory() -> Self {
        Self {
            games: Default::default(),
            rooms: Default::default(),
            custom_pieces: Default::default(),
        }
    }
}
