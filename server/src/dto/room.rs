use brainfuck_chess_engine::types::PlayerId;
use serde::{Deserialize, Serialize};

use super::game::PlayerDeckSpec;

#[derive(Clone, Serialize, Deserialize)]
pub struct MultiplayerRoom {
    pub id: String,
    pub board_size: i32,
    pub host_side: PlayerId,
    pub guest_side: PlayerId,
    #[serde(skip_serializing)]
    pub host_client_id: String,
    #[serde(skip_serializing)]
    pub guest_client_id: Option<String>,
    pub host_deck: Option<PlayerDeckSpec>,
    pub guest_deck: Option<PlayerDeckSpec>,
    pub host_ready: bool,
    pub guest_ready: bool,
    pub game_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRoomRequest {
    pub board_size: i32,
    pub host_side: PlayerId,
    pub client_id: String,
    pub deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
pub struct JoinRoomRequest {
    pub client_id: String,
    pub deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
pub struct SelectDeckRequest {
    pub client_id: String,
    pub deck: PlayerDeckSpec,
}

#[derive(Deserialize)]
pub struct RoomReadyRequest {
    pub client_id: String,
}

#[derive(Deserialize)]
pub struct ResignRoomRequest {
    pub client_id: String,
    pub player_id: PlayerId,
}
