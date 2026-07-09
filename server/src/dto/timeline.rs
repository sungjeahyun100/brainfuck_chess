use brainfuck_chess_engine::{actions::ActionEffect, ai::AiAction};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ActionTimelineFrame {
    pub action: AiAction,
    pub effects: Vec<ActionEffect>,
}
