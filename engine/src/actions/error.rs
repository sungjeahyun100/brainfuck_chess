#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    GameAlreadyEnded,
    WrongPlayer,
    IllegalMove,
    IllegalDrop,
    IllegalAbility,
    CannotEndTurn,
    UnsupportedAction,
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::GameAlreadyEnded => "게임이 이미 종료되었습니다.",
            Self::WrongPlayer => "현재 턴 플레이어와 행동 플레이어가 일치하지 않습니다.",
            Self::IllegalMove => "합법적인 이동이 아닙니다.",
            Self::IllegalDrop => "합법적인 착수가 아닙니다.",
            Self::IllegalAbility => "사용할 수 없는 능력입니다.",
            Self::CannotEndTurn => "행동 없이 턴을 종료할 수 없습니다.",
            Self::UnsupportedAction => "지원하지 않는 행동입니다.",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ActionError {}
