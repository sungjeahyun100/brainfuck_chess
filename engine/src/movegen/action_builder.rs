use crate::types::*;

pub(crate) fn push_action_if_unique(actions: &mut Vec<MoveAction>, action: MoveAction) {
    let exists = actions.iter().any(|m| {
        m.piece_id == action.piece_id
            && m.to == action.to
            && m.promotion == action.promotion
            && m.ability_id == action.ability_id
    });
    if !exists {
        actions.push(action);
    }
}
