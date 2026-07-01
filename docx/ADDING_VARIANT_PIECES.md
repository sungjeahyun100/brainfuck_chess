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

기본 기물 정의 파일:

```text
engine/src/pieces/default_pieces.rs
```

새 함수 하나를 추가하고 `all_default_definitions()`에 넣는다.

프로모션이나 능력을 쓰는 기물이 있으면 import도 함께 확인한다.

```rust
use crate::types::{
    AbilityDuration, PieceAbilityDefinition, PieceDefinition, PromotionCondition, PromotionRule,
};
```

예시: Wazir

```rust
/// Wazir: one step orthogonally.
pub fn wazir_definition() -> PieceDefinition {
    PieceDefinition {
        id: "wazir".into(),
        name: "Wazir".into(),
        score: 2,
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
        abilities: Vec::new(),
    }
}
```

그리고 목록에 추가한다.

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
- `chessembly_code`: 행마 정의.
- `chessembly_version`: 현재 기본값은 `"1.0"`이다.
- `dialect`: 기본 Chessembly만 쓰면 `None`, Brainfuck Chess 확장 문법 의존 시 `Some(ChessemblyDialect::BrainfuckChess)`.
- `extensions`: 확장 플래그가 필요할 때만 사용한다.
- `is_king`: 이 기물을 잡으면 즉시 게임이 끝나는 왕족 기물 여부다. 일반 변형기물은 `false`.
- `promotion`: 이 기물이 언제 진급할 수 있는지 정한다. 진급하지 않는 기물은 `None`.
- `promotion_pool`: 진급 가능한 대상 타입 ID 목록이다. 진급하지 않는 기물은 `Vec::new()`.
- `abilities`: 이 기물이 발동할 수 있는 능력 목록이다. 없으면 `Vec::new()`.

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
    PieceDefinition {
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
        abilities: Vec::new(),
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

## 3. 능력 추가

기물 능력은 기본 행마와 별도의 Chessembly 코드로 정의한다. Rust 룰 엔진은 기본 코드와 능력 코드를 합치지 않는다.

```text
Piece.active_ability == None
→ PieceDefinition.chessembly_code 실행

Piece.active_ability == Some(...)
→ PieceDefinition.abilities 안의 해당 ability.chessembly_code 실행
```

기본 예시는 Bishop의 `bounce_mode`다. 평소에는 Bishop 코드가 실행되고, 능력이 활성화된 동안에는 Bouncing Bishop 코드가 실행된다.

```rust
abilities: vec![PieceAbilityDefinition {
    id: "bounce_mode".into(),
    name: "Reflective Movement".into(),
    description: "Moves like a Bouncing Bishop until this turn ends.".into(),
    chessembly_code: bouncing_bishop_definition().chessembly_code,
    duration: AbilityDuration::UntilTurnEnd,
    once_per_turn: true,
}],
```

능력 필드:

- `id`: 기물 타입 안에서 능력을 식별하는 문자열이다.
- `name`: UI 표시용 이름이다.
- `description`: UI 설명용 텍스트다.
- `chessembly_code`: 능력이 켜져 있을 때 사용할 이동/공격 범위 코드다.
- `duration`: `UntilTurnEnd`, `UntilPieceMoves`, `Permanent`, `Turns(u32)` 중 하나다. 현재 `Turns(u32)`는 구조만 준비되어 있고 만료 정책은 TODO다.
- `once_per_turn`: `true`면 같은 턴에 같은 기물이 같은 능력을 두 번 발동할 수 없다.

능력 발동은 `TurnAction::ActivateAbility`로 제출한다.

```json
{
  "action": {
    "type": "activate_ability",
    "player_id": "white",
    "piece_id": "white_bishop_1",
    "ability_id": "bounce_mode"
  }
}
```

주의할 점:

- 능력 발동은 Move 모드 행동이다. Drop 모드가 선택된 턴에는 발동할 수 없다.
- 한 기물은 동시에 하나의 `active_ability`만 가질 수 있다.
- 능력 발동은 `turn_state.actions`에 기록되며, 이후 이동 또는 착수를 완료하면 턴이 자동으로 넘어간다.
- 능력이 활성화되면 legal move와 attack map 모두 같은 능력 Chessembly 코드를 기준으로 계산된다.
- 기물의 `type_id`는 능력 발동으로 바뀌지 않는다.
- 기본 행마에 능력 행마를 더하는 기능은 아직 없다. 그런 조합은 Chessembly 코드 자체로 표현하거나 별도 설계가 필요하다.

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

로비의 덱 빌더는 기물 목록을 하드코딩한다.

수정 파일:

```text
frontend/src/App.vue
```

`pieceCatalog`에 항목을 추가한다.

```ts
{ id: 'wazir', name: 'Wazir', score: 2, category: 'minor', canPocket: true },
```

`DeckPieceType` 유니언 타입에 새 ID가 필요하면 함께 추가한다. `pieceLabels`나 카테고리 필터가 새 ID를 전제로 한다면 해당 맵도 같이 갱신한다.

로비 심볼은 `displayPieceSymbol()`에서 추가한다.

```ts
wazir: 'W',
```

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

별도 필드가 필요한 상태성 기물이 아니라면 `types.rs` 수정은 보통 필요 없다. 프로모션은 `promotion`과 `promotion_pool`로 설정하고, 단순한 "발동 중에는 다른 행마 코드를 쓴다" 능력은 `abilities`와 `Piece.active_ability` 모델로 설정할 수 있다.

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
cargo test
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
- `engine/src/pieces/default_pieces.rs`에 `PieceDefinition` 함수를 추가했다.
- `all_default_definitions()`에 새 정의를 넣었다.
- 진급 기물이라면 `promotion` 조건과 `promotion_pool` 후보를 설정했다.
- 능력 기물이라면 `abilities`에 별도 Chessembly 코드와 지속 시간을 설정했다.
- `server/src/main.rs`의 `resolve_piece_type()`에 새 타입을 허용했다.
- `frontend/src/App.vue`의 카탈로그, 라벨, 심볼을 갱신했다.
- `frontend/src/components/GameScreen.vue`와 `frontend/src/components/Board.vue`의 `PIECE_SYMBOLS`를 갱신했다.
- 순수 Chessembly로 표현되지 않는 효과가 있으면 `legal_moves.rs`와 `endgame.rs`의 액션 적용 모델을 확장했다.
- Chessembly 단위 테스트를 추가했다.
- 룰 엔진 통합 테스트를 추가했다.
- `cargo test -p brainfuck-chess-engine`를 통과시켰다.
- 프론트 빌드 또는 수동 UI 확인을 완료했다.

---

## 12. 작업 예시 요약: Wazir

1. `default_pieces.rs`에 `wazir_definition()` 추가
2. `all_default_definitions()`에 `wazir_definition()` 추가
3. `server/src/main.rs`의 `resolve_piece_type()` match에 `"wazir"` 추가
4. `frontend/src/App.vue`의 `pieceCatalog`에 `{ id: 'wazir', name: 'Wazir', score: 2, category: 'minor', canPocket: true }` 추가
5. `App.vue`, `GameScreen.vue`, `Board.vue`의 심볼 맵에 `wazir: 'W'` 추가
6. `engine/tests/chessembly_compat.rs`에 중심/가장자리 이동 테스트 추가
7. `engine/tests/rule_engine.rs`에 실제 합법수와 포켓 착수 테스트 추가
8. `cargo test -p brainfuck-chess-engine` 실행
