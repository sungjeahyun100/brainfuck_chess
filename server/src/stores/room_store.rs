use std::sync::Arc;

use dashmap::DashMap;

use crate::dto::room::MultiplayerRoom;

pub type RoomStore = Arc<DashMap<String, MultiplayerRoom>>;

pub fn new_room_store() -> RoomStore {
    Arc::new(DashMap::new())
}
