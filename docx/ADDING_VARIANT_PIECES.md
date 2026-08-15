# Brainfuck Chess 새 변형기물 추가 가이드

이 문서는 현재 레포 기준으로 새 변형기물을 추가할 때 수정해야 하는 위치와 검증 순서를 정리한다.

Brainfuck Chess의 기물 행마는 기본적으로 `PieceDefinition`의 Chessembly 코드로 정의된다. 단, 서버의 덱 입력 허용 목록, 프론트엔드의 기물 카탈로그와 심볼, 특수 룰 적용 로직은 별도로 연결해야 한다.

---

## 1. 먼저 기물 성격을 분류한다

새 기물이 아래 조건을 만족하면 "순수 Chessembly 기물"로 추가할 수 있다.

- 도착 칸으로 직접 이동하거나 도착 칸의 적을 포획한다.
- 이동 가능 칸과 공격 가능 칸만으로 행마를 설명할 수 있다.
- 기물 고유 상태, 변신, 원거리 포획 후 제자리 유지, 자리 교환, 다중 기물 이동 같은 별도 효과가 없다.

진급은 `PieceDefinition`의 `promotion`과 `promotion_pool`로 설정할 수 있다. "특정 랭크에 도착하면 정해진 후보 중 하나로 타입이 바뀐다" 정도의 진급은 순수 Chessembly 행마에 정의 기반 프로모션 설정만 추가하면 된다.

아래에 해당하면 엔진 룰 확장이 필요하다.

- Pawn처럼 진영별 방향, 첫 이동 2칸, 앙파상 같은 별도 특례가 필요하다.
- 진급 조건이 랭크 도착보다 복잡하거나, 승격 후보가 게임 상태에 따라 동적으로 바뀐다.
- King/Rook처럼 캐슬링 등 다른 기물과 연동되는 특례가 필요하다.
- `catch`처럼 적을 잡고도 현재 위치에 남아야 한다.
- `shift`처럼 다른 기물과 자리를 바꿔야 한다.
- `take` + `jump`처럼 잡는 칸과 착지 칸이 다르다.
- `transition`, `set-state`, `if-state`처럼 상태 변화가 실제 게임 상태에 반영되어야 한다.

현재 `ChessemblyResult`는 `movement_squares`와 `attack_squares`만 반환한다. 따라서 인터프리터가 후보 칸을 계산하더라도, 실제 `MoveAction` 적용은 `engine/src/endgame.rs`의 보드 이동/포획 모델을 따른다. 효과가 도착 칸 이동/포획보다 복잡하면 `MoveAction` 또는 별도 액션 모델부터 확장해야 한다.

---

## 2. 엔진에 기물 정의 추가

기본 기물 정의는 기물별 모듈과 레지스트리로 나뉜다.

```text
engine/src/pieces/default_pieces/<piece_name>.rs  # 기물 정의와 행마 코드
engine/src/pieces/default_pieces.rs               # 모듈 등록/재노출/전체 목록
```

새 기물은 반드시 자기 파일을 만든다. 다른 기물의 `*_definition()`에서
`chessembly_code`를 가져오지 말고, 합성 기물이라도 자신의 행마 문자열을
파일 안에 직접 둔다. 그래야 한 기물의 행마 변경이 다른 기물에 조용히
전파되지 않는다.

단순한 단일 행마 기물은 레지스트리의 `legacy_piece_definition!` 매크로를
사용할 수 있다. 이 매크로는 빈 상태 스키마와 기본 이동 레이어/`normal`
옵션, 기본 시각 자산 키를 정규화 단계에서 채운다.

```rust
use crate::types::*;
```

예시: Wazir

```rust
/// Wazir: one step orthogonally.
pub fn wazir_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "wazir".into(),
        name: "Wazir".into(),
        score: 2,
        deployment_zone: DeploymentZone::Back,
        chessembly_code: "\
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);"
            .into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: None,
        promotion_pool: Vec::new(),
    }
}
```

그리고 `default_pieces.rs`에 모듈과 기존 공개 경로를 등록한다.

```rust
mod wazir;
pub use wazir::wazir_definition;
```

마지막으로 기존 순서를 바꾸지 않고 전체 목록에 추가한다.

```rust
pub fn all_default_definitions() -> Vec<PieceDefinition> {
    vec![
        king_definition(),
        queen_definition(),
        rook_definition(),
        bishop_definition(),
        knight_definition(),
        pawn_white_definition(),
        pawn_black_definition(),
        wazir_definition(),
    ]
}
```

필드 기준:

- `id`: 서버/프론트/덱에서 쓰는 타입 ID다. 소문자 kebab-case를 권장한다.
- `name`: UI 표시명과 테스트 가독성에 쓰인다.
- `score`: 덱 점수. King이 아니면 점수 합산 대상이다.
- `deployment_zone`: 게임 시작 시 `Front`(상대와 가까운 폰 시작 줄) 또는
  `Back`(나머지 시작 줄)에 놓이는지를 정한다. 서버 덱 검증과 덱 빌더 UI가
  이 정의를 함께 사용하므로 기물 이름이나 점수 기반 예외 목록을 추가하지 않는다.
- `chessembly_code`: 행마 정의.
- `chessembly_version`: 현재 기본값은 `"1.0"`이다.
- `dialect`: 기본 Chessembly만 쓰면 `None`, Brainfuck Chess 확장 문법 의존 시 `Some(ChessemblyDialect::BrainfuckChess)`.
- `extensions`: 확장 플래그가 필요할 때만 사용한다.
- `is_king`: 이 기물을 잡으면 즉시 게임이 끝나는 왕족 기물 여부다. 일반 변형기물은 `false`.
- `promotion`: 이 기물이 언제 진급할 수 있는지 정한다. 진급하지 않는 기물은 `None`.
- `promotion_pool`: 진급 가능한 대상 타입 ID 목록이다. 진급하지 않는 기물은 `Vec::new()`.
- `state_schema`: 기물 인스턴스별 상태의 키와 초기값이다.
- `move_layers`: 독립적으로 실행할 Chessembly 프로그램과 활성 조건이다.
- `move_options`: 사용자가 선택할 일반/특수 이동과 참조 레이어다.
- `visual`: 기본 자산 키와 상태별 시각 변형이다.
- `can_capture_on_drop`: 포켓 착수로 상대 기물을 잡을 수 있는지 여부다.

주의: `is_king: true`는 단순히 "중요한 기물" 표시가 아니라 승리 조건에 직접 연결된다. 새 왕족 기물을 추가하는 경우 덱 검증의 "King 1개" 정책도 함께 재검토해야 한다.

### 프로모션 설정

프로모션은 조건과 후보 풀을 분리해서 정의한다.

```rust
promotion: Some(PromotionRule {
    condition: PromotionCondition::LastRank,
}),
promotion_pool: vec!["queen".into(), "rook".into(), "bishop".into(), "knight".into()],
```

- `PromotionCondition::LastRank`: 보드의 마지막 랭크(`board_size - 1`)에 도착하면 진급한다. White Pawn 기본값이다.
- `PromotionCondition::FirstRank`: 0번 랭크에 도착하면 진급한다. Black Pawn 기본값이다.
- `PromotionCondition::Rank { rank }`: 특정 랭크 번호에 도착하면 진급한다.
- `promotion_pool`: 실제 `MoveAction.promotion`으로 선택 가능한 타입 ID 목록이다. 여기에 없는 타입으로는 진급할 수 없다.

예시: 마지막 랭크에 도착하면 `queen` 또는 `knight`로만 진급 가능한 커스텀 기물

```rust
pub fn promoter_definition() -> PieceDefinition {
    legacy_piece_definition! {
        id: "promoter".into(),
        name: "Promoter".into(),
        score: 2,
        chessembly_code: "move(0, 1);".into(),
        chessembly_version: "1.0".into(),
        dialect: None,
        extensions: None,
        is_king: false,
        promotion: Some(PromotionRule {
            condition: PromotionCondition::LastRank,
        }),
        promotion_pool: vec!["queen".into(), "knight".into()],
    }
}
```

프로모션 가능한 이동은 합법수 생성 단계에서 후보마다 별도 `MoveAction`으로 확장된다. 예를 들어 후보 풀이 `queen`, `knight`라면 같은 도착 칸에 대해 `promotion: Some("queen")`, `promotion: Some("knight")` 액션이 각각 만들어진다.

주의할 점:

- `promotion`만 있고 `promotion_pool`이 비어 있으면 프로모션 액션이 생성되지 않는다.
- `promotion_pool`만 있고 `promotion: None`이면 프로모션 액션이 생성되지 않는다.
- 후보 타입 ID는 `piece_definitions`에 존재해야 실제 게임에서 의미가 있다.
- 후보 타입이 게임 생성 API에서 덱에 직접 넣을 수 있어야 하는지는 별개다. 진급 전용 타입이라면 서버의 `resolve_piece_type()`에는 허용하지 않고 엔진 정의에만 둘 수도 있다.

---

## 3. 이동 옵션과 독립 행동 추가

선택 가능한 행마 능력은 `move_layers`와 `move_options`로 정의한다.
각 레이어가 자기 Chessembly 코드를 가지며, 옵션은 실행할 레이어 ID를
참조한다. 예를 들어 Cannon Rook은 `rook_move`와 `cannon_move` 레이어를
같은 기물 파일 안에 직접 정의한다.

```rust
move_layers: vec![
    MoveLayerDefinition {
        id: "normal_move".into(),
        chessembly_code: normal_code,
        enabled_when: Vec::new(),
        on_commit: Vec::new(),
    },
    MoveLayerDefinition {
        id: "special_move".into(),
        chessembly_code: special_code,
        enabled_when: Vec::new(),
        on_commit: Vec::new(),
    },
],
move_options: vec![
    MoveOptionDefinition {
        id: "normal".into(),
        name: "일반 이동".into(),
        description: String::new(),
        kind: MoveOptionKind::Normal,
        layer_ids: vec!["normal_move".into()],
        execution_mode: MoveOptionExecutionMode::MoveModifier,
        contributes_to_attack_map: true,
        cooldown: None,
    },
    MoveOptionDefinition {
        id: "special".into(),
        name: "특수 이동".into(),
        description: String::new(),
        kind: MoveOptionKind::Ability,
        layer_ids: vec!["special_move".into()],
        execution_mode: MoveOptionExecutionMode::MoveModifier,
        contributes_to_attack_map: true,
        cooldown: Some(CooldownDefinition {
            turns: 3,
            clock: CooldownClock::OwnerTurns,
        }),
    },
],
```

- `MoveModifier`는 선택한 레이어로 합법 이동과 공격 범위를 계산하고,
  생성된 `MoveAction.move_option_id`로 서버가 선택을 검증한다.
- `enabled_when`과 `on_commit`은 `state_schema`에 선언된 기물별 상태만
  참조해야 한다.
- `contributes_to_attack_map: false`인 옵션은 공격 맵에 포함되지 않는다.
- `StandaloneAction`은 Chessembly 이동이 아니라 `TurnAction::Ability`로
  처리한다. 현재 일반화된 플러그인 지점이 아니며, 새 독립 행동을 추가할
  때는 `legal_moves.rs`, `actions.rs`, `endgame.rs`의 canonical 생성·검증·적용
  경계를 함께 구현하고 테스트해야 한다.

---

## 4. Chessembly 작성 기준

현재 가장 안전하게 쓸 수 있는 기본 행마식은 다음이다.

```text
take-move(dx, dy)  // 빈 칸 이동 + 적 포획
move(dx, dy)       // 빈 칸 이동만
take(dx, dy)       // 공격/포획 후보
repeat(n)          // 직전 n개 식 반복
observe(dx, dy)    // 특정 칸이 비었는지 검사
anchor(dx, dy)     // 기준 위치 이동
{ ... }            // 블록 스코프
```

슬라이더 기물은 `repeat(1)`을 붙인다.

```chessembly
take-move(1, 1) repeat(1);
take-move(-1, 1) repeat(1);
take-move(1, -1) repeat(1);
take-move(-1, -1) repeat(1);
```

점프형 기물은 각 방향을 별도 체인으로 적는다.

```chessembly
take-move(1, 2);
take-move(2, 1);
take-move(2, -1);
take-move(1, -2);
take-move(-1, -2);
take-move(-2, -1);
take-move(-2, 1);
take-move(-1, 2);
```

폰처럼 이동과 공격이 분리된 기물은 `move`와 `take`를 분리한다.

```chessembly
move(0, 1);
take(1, 1);
take(-1, 1);
```

현재 룰 엔진은 `attack_squares`를 포켓 착수 가능 칸 계산에도 사용한다. 공격 범위가 넓은 기물을 추가하면 해당 플레이어의 착수 가능 범위도 넓어진다.

---

## 5. 서버 입력 허용 목록 추가

서버는 `PlayerDeckSpec`의 문자열을 곧바로 신뢰하지 않고 `resolve_piece_type()`에서 허용된 타입만 받는다.

수정 파일:

```text
server/src/main.rs
```

현재 기본 구조:

```rust
fn resolve_piece_type(player_id: &str, raw_piece_type: &str) -> Option<String> {
    match raw_piece_type {
        "king" | "queen" | "rook" | "bishop" | "knight" => Some(raw_piece_type.into()),
        "pawn" | "pawn-white" | "pawn-black" => Some(if player_id == "white" {
            "pawn-white".into()
        } else {
            "pawn-black".into()
        }),
        _ => None,
    }
}
```

순수 변형기물은 match 목록에 ID를 추가한다.

```rust
"king" | "queen" | "rook" | "bishop" | "knight" | "wazir" => Some(raw_piece_type.into()),
```

진영별 정의가 필요한 기물은 Pawn처럼 중립 입력 ID를 받아 화이트/블랙 타입으로 변환한다.

```rust
"soldier" | "soldier-white" | "soldier-black" => Some(if player_id == "white" {
    "soldier-white".into()
} else {
    "soldier-black".into()
}),
```

이 함수를 수정하지 않으면 엔진에 `PieceDefinition`을 추가해도 게임 생성 API에서 "알 수 없는 기물 타입"으로 거절된다.

---

## 6. 프론트엔드 카탈로그와 심볼 추가

덱 빌더와 기물 실험실이 공유하는 기본 기물 목록을 갱신한다.

수정 파일:

```text
frontend/src/composables/useDeckValidation.ts
```

`pieceCatalog`에 항목을 추가한다.

```ts
{ id: 'wazir', name: 'Wazir', score: 2, category: 'minor', canPocket: true },
```

카탈로그 타입이나 카테고리 필터가 새 ID를 전제로 한다면 관련 타입과 맵도
같이 갱신한다.

게임 화면과 보드의 실제 표시 심볼도 각각 추가한다.

```text
frontend/src/components/GameScreen.vue
frontend/src/components/Board.vue
```

두 파일 모두 `PIECE_SYMBOLS`에 같은 타입 ID를 넣는다.

```ts
wazir: 'W',
```

유니코드 체스 기호가 없는 변형기물은 짧은 알파벳 또는 커스텀 아이콘을 사용한다. 현재 보드는 문자열 하나를 크게 그리는 구조라 긴 이름은 칸 안에서 깨질 수 있다.

---

## 7. 타입 정의 확인

프론트엔드의 서버 응답 타입은 이미 확장 가능한 문자열 타입을 쓴다.

```text
frontend/src/types/game.ts
```

`PieceTypeId = string`이므로 서버에서 내려오는 새 타입 ID 자체는 타입 오류 없이 받을 수 있다. 다만 로비의 `DeckPieceType`처럼 UI 내부에서 좁힌 타입을 쓰는 곳은 별도 갱신이 필요하다.

Rust 쪽 `PieceDefinition`도 이미 커스텀 기물을 담을 수 있다.

```text
engine/src/types.rs
```

별도 필드가 필요한 상태성 기물이 아니라면 `types.rs` 수정은 보통 필요
없다. 프로모션은 `promotion`과 `promotion_pool`로 설정하고, 선택형 행마는
`move_layers`와 `move_options`로 설정한다. 기물별 지속 상태가 필요하면
`state_schema`, 레이어의 `enabled_when`과 `on_commit`을 사용한다.

---

## 8. 특수 룰이 필요한 경우

다음 파일들이 특수 룰의 주요 연결점이다.

```text
engine/src/legal_moves.rs
engine/src/endgame.rs
engine/src/attack_map.rs
engine/src/placement.rs
engine/src/rules.rs
```

현재 특례 예시:

- Pawn 2칸 이동 제한: `engine/src/legal_moves.rs`
- 앙파상 가능/만료/적용: `engine/src/legal_moves.rs`, `engine/src/endgame.rs`
- 정의 기반 프로모션 생성/적용: `engine/src/legal_moves.rs`, `engine/src/endgame.rs`
- 캐슬링 후보 생성과 룩 이동: `engine/src/legal_moves.rs`, `engine/src/endgame.rs`
- King 포획 시 게임 종료: `engine/src/endgame.rs`
- 포켓 착수 가능 칸: `engine/src/placement.rs`
- 덱 점수와 King 검증: `engine/src/rules.rs`

특수 룰을 추가할 때는 먼저 액션 모델이 그 효과를 표현할 수 있는지 확인한다. 예를 들어 원거리 포획은 현재 `MoveAction`만으로는 "공격자는 제자리에 있고 대상만 제거"를 표현하지 못한다. 이런 경우 `TurnAction`에 별도 액션을 추가하거나 `MoveAction`에 효과 필드를 추가한 뒤 서버 검증, 적용, 프론트 클릭 처리까지 함께 바꿔야 한다.

---

## 9. 테스트 추가

최소 테스트는 두 종류를 권장한다.

1. Chessembly 단위 테스트

```text
engine/tests/chessembly_compat.rs
```

인터프리터가 새 기물의 `movement_squares`와 `attack_squares`를 의도대로 계산하는지 확인한다.

```rust
#[test]
fn test_wazir_center() {
    let board = create_board(8);
    let def = wazir_definition();
    let piece = make_piece("w1", "white", "wazir", 3, 3);
    let mut pieces = HashMap::new();
    pieces.insert("w1".into(), piece.clone());

    let result = run_code(&def.chessembly_code, &piece, &board, &pieces, &def);
    assert!(result.movement_squares.contains(&Square::new(4, 3)));
    assert!(result.movement_squares.contains(&Square::new(2, 3)));
    assert!(result.movement_squares.contains(&Square::new(3, 4)));
    assert!(result.movement_squares.contains(&Square::new(3, 2)));
}
```

2. 룰 엔진 통합 테스트

```text
engine/tests/rule_engine.rs
```

실제 `GameState`에서 합법수 생성, 아군 충돌, 적 포획, 포켓 착수, 점수 계산까지 확인한다.

프로모션 기물이라면 다음도 확인한다.

- 프로모션 랭크에 도착하는 이동이 `promotion_pool` 후보 수만큼 생성되는지
- 프로모션 랭크가 아닌 이동은 `promotion: None` 하나만 생성되는지
- `apply_move_action()` 뒤 기물의 `type_id`가 선택한 후보 타입으로 바뀌는지
- `promotion_pool` 밖의 타입은 서버 검증에서 합법수로 인정되지 않는지

능력 기물이라면 다음도 확인한다.

- 능력 비활성 상태에서는 기본 `chessembly_code`를 사용하는지
- 능력 활성 상태에서는 해당 ability의 `chessembly_code`를 사용하는지
- legal move와 attack map 양쪽에 능력 코드가 반영되는지
- `UntilTurnEnd`, `UntilPieceMoves`, `Permanent` 만료가 의도대로 동작하는지
- `once_per_turn`과 이미 활성화된 능력 재발동이 서버에서 거부되는지
- 능력 발동 뒤에도 기물의 `type_id`가 바뀌지 않는지

새 기물이 서버/프론트에서 선택 가능해야 한다면 다음도 수동 확인한다.

- 로비 카탈로그에 표시되는지
- 시작 배치에 놓을 수 있는지
- 포켓에 추가할 수 있는지
- 게임 생성 API가 거절하지 않는지
- 보드에서 심볼이 `?`로 나오지 않는지
- 클릭 시 이동/공격/착수 하이라이트가 의도대로 나오는지

---

## 10. 검증 명령

엔진 테스트:

```bash
cargo test -p brainfuck-chess-engine
```

전체 Rust 워크스페이스 테스트:

```bash
cargo test --workspace
```

프론트 타입/빌드 확인:

```bash
cd frontend
npm run build
```

서버까지 포함한 로컬 실행은 프로젝트의 기존 실행 방식에 맞춰 확인한다.

---

## 11. 새 기물 추가 체크리스트

- 기물 ID를 정했다. 예: `wazir`, `knightrider`, `archbishop`
- 점수와 포켓 허용 여부를 정했다.
- `engine/src/pieces/default_pieces/<piece_name>.rs`에 독립된 정의와 행마 코드를 추가했다.
- 다른 기물의 정의 함수나 행마 코드에 의존하지 않는지 확인했다.
- `engine/src/pieces/default_pieces.rs`에 모듈 선언과 공개 재노출을 추가했다.
- `all_default_definitions()`에 새 정의를 넣었다.
- 진급 기물이라면 `promotion` 조건과 `promotion_pool` 후보를 설정했다.
- 선택형 행마라면 `move_layers`와 `move_options`를 설정했다.
- 독립 행동이라면 canonical 생성·검증·적용 경계를 구현했다.
- `server/src/main.rs`의 `resolve_piece_type()`에 새 타입을 허용했다.
- `frontend/src/composables/useDeckValidation.ts`의 카탈로그와 라벨을 갱신했다.
- `frontend/src/components/GameScreen.vue`와 `frontend/src/components/Board.vue`의 `PIECE_SYMBOLS`를 갱신했다.
- 순수 Chessembly로 표현되지 않는 효과가 있으면 `legal_moves.rs`와 `endgame.rs`의 액션 적용 모델을 확장했다.
- Chessembly 단위 테스트를 추가했다.
- 룰 엔진 통합 테스트를 추가했다.
- `cargo test -p brainfuck-chess-engine`를 통과시켰다.
- 프론트 빌드 또는 수동 UI 확인을 완료했다.

---

## 12. 작업 예시 요약: Wazir

1. `default_pieces/wazir.rs`에 독립된 `wazir_definition()`과 행마 코드 추가
2. `default_pieces.rs`에 `mod wazir`, `pub use`와 목록 등록 추가
3. `server/src/main.rs`의 `resolve_piece_type()` match에 `"wazir"` 추가
4. 프론트엔드 기물 카탈로그와 시각 자산/심볼 연결 추가
5. `engine/tests/chessembly_compat.rs`에 중심/가장자리 이동 테스트 추가
6. `engine/tests/rule_engine.rs`에 실제 합법수와 포켓 착수 테스트 추가
7. `cargo test --workspace` 실행
