use std::sync::Arc;

use dashmap::DashMap;

use crate::time_control::StoredGame;
use crate::MultiplayerRoom;
use crate::{account::AccountRepository, custom_piece::CustomPieceRepository};

pub(crate) type GameStore = Arc<DashMap<String, StoredGame>>;
pub(crate) type RoomStore = Arc<DashMap<String, MultiplayerRoom>>;
pub(crate) type CustomPieceStore = Arc<dyn CustomPieceRepository>;
pub(crate) type AccountStore = Arc<dyn AccountRepository>;
