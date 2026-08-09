use serde::{Deserialize, Serialize};

use crate::types::{AbilityAction, DropAction, MoveAction};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiAction {
    Move(MoveAction),
    Drop(DropAction),
    Ability(AbilityAction),
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
}

impl BotDifficulty {
    pub const fn limits(self) -> SearchLimits {
        match self {
            Self::Easy => SearchLimits {
                max_depth_actions: 1,
                max_nodes: 500,
                soft_time_ms: 50,
                hard_time_ms: 100,
            },
            Self::Normal => SearchLimits {
                max_depth_actions: 2,
                max_nodes: 3_000,
                soft_time_ms: 150,
                hard_time_ms: 300,
            },
            Self::Hard => SearchLimits {
                max_depth_actions: 3,
                max_nodes: 10_000,
                soft_time_ms: 400,
                hard_time_ms: 800,
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
    #[serde(default)]
    pub completed_depth: u8,
    #[serde(default)]
    pub stats: SearchStats,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchStats {
    pub searched_nodes: u64,
    pub depth_reached: u8,
    pub completed_depth: u8,
    pub beta_cutoffs: u64,
    pub qnodes: u64,
    pub tt_hits: u64,
    pub tt_cutoffs: u64,
    pub aspiration_researches: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotTurnResult {
    pub state: crate::types::GameState,
    pub actions: Vec<AiAction>,
    /// Authoritative post-action snapshots. Replaying these frames yields the
    /// exact server state, including turn metadata and non-visual rule state.
    pub timeline: Vec<ActionTimelineFrame>,
    pub searched_nodes: u64,
    pub depth_reached: u8,
    #[serde(default)]
    pub completed_depth: u8,
    #[serde(default)]
    pub stats: SearchStats,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTimelineFrame {
    pub action: AiAction,
    pub state: crate::types::GameState,
}
