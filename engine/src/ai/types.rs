use serde::{Deserialize, Serialize};

use crate::types::{DropAction, MoveAction};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiAction {
    Move(MoveAction),
    Drop(DropAction),
    EndTurn,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotDifficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchLimits {
    pub max_depth_actions: u8,
    pub max_nodes: u64,
    pub soft_time_ms: u64,
    pub hard_time_ms: u64,
    pub max_actions_per_turn: u8,
}

impl BotDifficulty {
    pub const fn limits(self) -> SearchLimits {
        match self {
            Self::Easy => SearchLimits {
                max_depth_actions: 1,
                max_nodes: 500,
                soft_time_ms: 50,
                hard_time_ms: 100,
                max_actions_per_turn: 4,
            },
            Self::Normal => SearchLimits {
                max_depth_actions: 2,
                max_nodes: 3_000,
                soft_time_ms: 150,
                hard_time_ms: 300,
                max_actions_per_turn: 6,
            },
            Self::Hard => SearchLimits {
                max_depth_actions: 3,
                max_nodes: 10_000,
                soft_time_ms: 400,
                hard_time_ms: 800,
                max_actions_per_turn: 8,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotDecision {
    pub action: AiAction,
    pub score: i32,
    pub searched_nodes: u64,
    pub depth_reached: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchStats {
    pub searched_nodes: u64,
    pub depth_reached: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotTurnResult {
    pub state: crate::types::GameState,
    pub actions: Vec<AiAction>,
    pub searched_nodes: u64,
    pub depth_reached: u8,
    pub elapsed_ms: u64,
}
