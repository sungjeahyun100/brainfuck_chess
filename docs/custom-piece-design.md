# 체섬블리 커스텀 기물 통합 설계

상태: 1단계 설계 확정안  
기준 커밋: `296ee65`  
작성 기준일: 2026-07-27

이 문서는 커스텀 기물 시제품의 2단계(엔진과 체섬블리 런타임)부터
사용할 설계 계약이다. 대괄호 표시는 다음 의미를 갖는다.

- **[확인]** 현재 저장소의 코드나 설정에서 직접 확인한 사실
- **[추론]** 확인한 사실로부터 도출했지만 아직 코드 계약은 아닌 내용
- **[제안]** 다음 단계에서 구현할 계약 또는 정책

## 1. 조사한 프로젝트 구조

### 1.1 워크스페이스와 빌드

- **[확인]** 루트 `Cargo.toml:1-6`은 `engine`, `server` 두 Rust crate로
  구성된 resolver 2 워크스페이스다.
- **[확인]** `engine/Cargo.toml:1-12`의 엔진은 Rust 2021 crate이며 런타임
  의존성은 `serde`, `serde_json`뿐이다.
- **[확인]** `server/Cargo.toml:1-15`의 서버는 Rust 2021, Axum 0.7,
  Tokio, `DashMap`, UUID를 사용하고 엔진을 경로 의존성으로 참조한다.
- **[확인]** 서버 진입점은 `server/src/main.rs:764-793`의 `main`이며,
  `server/src/routes.rs:9-34`가 `/api` 아래 HTTP 라우팅을 구성한다.
- **[확인]** `frontend/package.json:6-19`의 프론트엔드는 Vue 3,
  TypeScript, Vite이며 `vue-tsc && vite build`로 빌드한다.
- **[확인]** Vue Router나 Pinia/Vuex는 의존성에 없다. 화면 전환과 최상위
  상태는 `frontend/src/App.vue:71-220`의 ref와 `AppView` 분기로 관리한다.
- **[확인]** Rust/TypeScript 타입 코드 생성은 없다. 프론트 타입은
  `frontend/src/types/game.ts:1-239`에서 Rust serde 형태를 수동으로
  복제한다.

현재 검증 명령은 다음과 같다.

```text
cargo test --workspace
cargo check --workspace
npm --prefix frontend test
npm --prefix frontend run build
```

### 1.2 저장소, 인증, 업로드

- **[확인]** `server/src/stores.rs:1-9`의 저장소는 프로세스 메모리
  `DashMap<String, GameState>`와 `DashMap<String, MultiplayerRoom>`뿐이다.
  서버 재시작 후 데이터가 사라진다.
- **[확인]** DB crate, DB 모델, 마이그레이션 디렉터리나 마이그레이션
  도구가 없다.
- **[확인]** 계정/인증 미들웨어가 없다. 멀티플레이어 권한은
  `frontend/src/api/gameApi.ts:117-124`가 탭별로 만든 `client_id` 문자열과
  `server/src/main.rs:964-1008` 등의 문자열 비교에만 의존한다.
- **[확인]** `cloudbuild.yaml:38`은 현재 서비스를 unauthenticated로
  배포한다.
- **[확인]** multipart나 이미지 업로드 라우트, 파일 저장소도 없다.
- **[제안]** 3단계에서 `CustomPieceRepository` 계약을 먼저 두고
  `InMemoryCustomPieceRepository`를 시제품 기본 구현으로 사용한다. 실제
  계정/DB가 도입되면 구현체만 교체한다. 다만 소유권이 필요한 CRUD를
  익명 `client_id`에 영구 결합하지 않는다.

### 1.3 프론트엔드 데이터와 테스트

- **[확인]** API 경계는 `frontend/src/api/gameApi.ts:105-287`의 단일
  `request<T>`와 `api` 객체다.
- **[확인]** 덱은 서버에 저장되지 않고
  `frontend/src/composables/useSavedDecks.ts:4-21`의
  `brainfuck_chess_saved_decks_v1` localStorage에 저장된다.
- **[확인]** 덱 타입은 `frontend/src/types/deck.ts:17-33`의
  `pieceType: string`, `pocket: Record<string, number>`로 기물 타입 문자열만
  참조한다. 버전 참조는 없다.
- **[확인]** 덱의 서버 전송은
  `frontend/src/composables/useDeckSerialization.ts:1-39`에서
  `{piece_type, square}`와 `pocket: string[]`로 평탄화한다.
- **[확인]** `frontend/src/views/DeckEditor.vue:200-451`은 덱 편집 전체를
  담당하는 큰 화면이다. 커스텀 기물 제작 책임을 여기에 추가하면 안 된다.
- **[확인]** `frontend/src/views/PieceLab.vue:1-1212`는 보드 구성, 옵션
  조회, 미리보기, 로컬 행동 적용까지 가진 기존 테스트 보드다.
  `frontend/src/views/PieceLab.vue:1137-1201`이 서버의 권위 있는 합법 행동을
  불러온다.
- **[확인]** 프론트 테스트는
  `frontend/src/pieceVisual.test.ts` 하나이며 `package.json:9`의 Node
  스크립트로 실행된다.

## 2. 현재 체섬블리 실행 흐름

### 2.1 입력, 파싱, AST

1. **[확인]** 원문은 `engine/src/types.rs:163-196`의
   `PieceDefinition.chessembly_code`와 각
   `MoveLayerDefinition.chessembly_code`에 `String`으로 저장된다.
2. **[확인]** 기본 기물은
   `engine/src/pieces/default_pieces/*.rs`의 기물별 모듈에서 Rust 값과 독립된
   체섬블리 행마 코드로 정의된다. `engine/src/pieces/default_pieces.rs`는
   모듈을 재노출하고 `all_default_definitions()`으로 기존 등록 순서대로
   모은다. 즉 현재 기본 기물도 체섬블리 코드를 행마의 원천으로 사용한다.
3. **[확인]** 파싱 진입점은 `engine/src/chessembly/parser.rs:11-29`의
   `parse(&str) -> Program`이다.
4. **[확인]** AST는 `engine/src/chessembly/ast.rs:12-104`의
   `Program = Vec<Chain>`, `Chain = Vec<Expr>`와 `Expr` enum이다.
5. **[확인]** `ChessemblyProgramCache::rebuild`
   (`engine/src/types.rs:626-645`)가 정의의 모든 이동 레이어를 파싱해
   `type_id::layer_id` 키로 캐시한다. 캐시는 serde에서 제외된다
   (`engine/src/types.rs:917-918`).

현재 parser/runtime 계약의 중요한 한계:

- **[확인]** parser는 `Result`를 반환하지 않는다. 알 수 없는 식별자와
  잘못된 식은 `Expr::End` 또는 토큰 건너뛰기로 처리된다
  (`parser.rs:164-226`).
- **[확인]** 따라서 결정적인 문법 오류 위치, 코드, 메시지를 표현하는
  오류 타입이 없다.
- **[확인]** 하나의 입력은 이동 명령 프로그램 하나만 만든다. 스크립트
  안에서 이름 있는 여러 `PieceDefinition`을 선언하는 문법이나 컴파일
  산출물은 없다.
- **[확인]** AST/파서에 `transition(name)`은 있지만
  (`ast.rs:58-62`, `parser.rs:218`), 인터프리터는 태그를 저장만 하고
  (`interpreter.rs:490-493`) `ChessemblyResult`에 기물 타입 변경을
  기록하지 않는다. 현재 실제 타입 전환은 구현되지 않았다.
- **[확인]** `repeat`/`do while`의 정지 조건은 anchor/bits 변화 감지뿐이며
  (`interpreter.rs:501-566`) 전체 명령 수, 결과 수, 시간, 재귀/블록 깊이
  제한은 없다.

### 2.2 실행 컨텍스트와 결과

`run_chessembly_layer_for_piece`의 현재 호출 계약은
`engine/src/chessembly/mod.rs:13-34`에 있다.

```text
GameState + Piece + PieceDefinition + MoveLayerDefinition
  -> GameState.chessembly_layer_program(type, layer)
  -> ExecutionContext {
       board,
       piece(current_square, owner, type_id, state),
       piece_definition,
       all_definitions,
       all_pieces,
       player,
       global_state,
       attack_maps
     }
  -> interpreter::run
  -> ChessemblyResult {
       movement_squares,
       attack_squares,
       effects[square].set_state
     }
```

- **[확인]** 현재 위치는 `Piece.current_square`, 색/진영은 `Piece.owner`와
  실행 인자 `player`, 대상 위치는 각 `Expr`의 상대 좌표와 board 조회로
  전달된다 (`interpreter.rs:26-52`, `148-499`).
- **[확인]** `Board`, 모든 기물, 모든 정의가 컨텍스트에 들어가므로
  `enemy`, `friendly`, `piece-on`, `danger`, `if-state`를 평가할 수 있다.
- **[확인]** 반환 값은 이동 칸, 공격 칸, 칸별 global state 효과다
  (`engine/src/types.rs:1009-1018`). 실행 오류 variant는 없다.
- **[확인]** 미리보기 실행은 `GameState`를 빌리지 않고 불변 참조만
  사용한다. 상태 효과는 후보 `MoveAction.effects`에 붙고 실제 승인된
  행동 적용 때만 반영된다 (`legal_moves.rs:171-208`,
  `endgame.rs:65-186`). 이 경계는 유지해야 한다.

## 3. 현재 게임 엔진 실행 흐름

### 3.1 게임 생성과 정의 로드

```text
POST /api/games 또는 room ready
  -> create_game / start_room_game
  -> build_game_state                          server/src/main.rs:390-461
  -> all_default_definitions
  -> build_player_deck(정의로 인스턴스 상태 초기화/점수 검증)
  -> GameState {
       pieces,
       piece_definitions: 전체 기본 정의,
       chessembly_program_cache
     }
  -> GameStore(DashMap)에 삽입
```

- **[확인]** 현재 `resolve_piece_type`는 기본 기물 이름만 allowlist로
  해석한다 (`server/src/main.rs:256-277`). 커스텀 ID는 게임 생성에 들어갈
  수 없다.
- **[확인]** 게임별 `GameState.piece_definitions`가 합법 수, 적용, 공격,
  포켓, AI가 참조하는 실질적 카탈로그다.
- **[확인]** `engine/src/context.rs:6-74`에는 기본 정의와 게임 정의를 합치는
  `PieceCatalog`, 파생 캐시인 `RuntimeResources`, `GameContext`가 이미
  있으나 현재 제품 호출 경로에서는 사용되지 않고 테스트 한 곳에서만
  사용된다.
- **[위험]** `PieceCatalog::for_state`가 매번 기본 카탈로그를 새로 만들고
  `expect`로 검증한다. 이를 그대로 요청마다 쓰면 불필요한 재생성과 panic
  경계가 생긴다. 반대로 현재 직접 참조 경로와 혼용하면 서로 다른 정의
  집합이 될 수 있다.

### 3.2 선택, 합법 수, 검증과 적용

```text
기물 선택
  -> GET /games/:id/pieces/:piece_id/options
  -> generate_piece_legal_move_actions_with_options
     -> 정의/option/layer 선택
     -> run_chessembly_layer_for_piece
     -> ChessemblyResult를 MoveAction으로 구성

행동 제출
  -> POST /games/:id/actions (piece_id, to, promotion?, option_id?)
  -> 서버가 권위 상태에서 후보를 다시 생성
  -> 정확히 한 후보와 일치하는지 확인
  -> actions::submit_action                    engine/src/actions.rs:8-38
     -> 엔진이 canonical legal action을 다시 확인
     -> endgame::apply_and_advance_turn
        -> apply_move_action / apply_drop_action
        -> 포획, 상태 효과, 승리 판정
        -> history 기록과 turn/current_player 전환
```

- **[확인]** 클라이언트가 보낸 capture/effect는 받지 않고 서버가 재생성한다
  (`server/src/main.rs:1239-1307`).
- **[확인]** 적용은 포획과 보드 이동, 승급, 레이어 `on_commit`, global
  state, cooldown을 처리한다 (`engine/src/endgame.rs:65-266,326-415`).
- **[확인]** 캐슬링, 앙파상, 폰 방향과 시작 랭크는 타입 ID 문자열을
  하드코딩한다 (`legal_moves.rs:9-34,360-504`,
  `endgame.rs:3-44,326-415`). 커스텀 기물이 해당 특별 규칙을 이름으로
  획득해서는 안 된다.

### 3.3 공격 범위, 포켓, AI

- **[확인]** 공격 범위는 `engine/src/attack_map.rs:8-72`가 같은
  `GameState.piece_definitions`와 체섬블리 레이어를 순회한다.
- **[확인]** 포켓 후보는 `engine/src/placement.rs:13-77`에서 기본 진영과
  같은 공격 맵을 합치며, 착수 가능 여부와 king 여부도 같은 정의에서
  읽는다.
- **[확인]** AI는 `engine/src/ai/search.rs:13-53`에서 일반 legal move/drop
  생성 및 `submit_action`을 그대로 사용하고, 상태를 clone해 탐색한다.
- **[확인]** 평가는 `engine/src/ai/evaluate.rs:39-99`에서 알 수 없는 정의를
  조용히 건너뛰고, 알려진 정의의 `score`를 material 값으로 사용한다.
- **[추론]** 커스텀 정의가 게임 상태에 완전하게 들어가면 AI의 생성/복사는
  별도 분기 없이 동작한다. 다만 누락 정의를 0점처럼 취급하는 현재
  fail-open은 복원 검증에서 차단해야 한다.

### 3.4 직렬화와 복원

- **[확인]** `GameState`는 `piece_definitions` 전체를 JSON에 포함하고,
  프로그램 캐시만 제외한다 (`types.rs:895-919`).
- **[확인]** `GameState::ensure_chessembly_cache`가 역직렬화 후 빈 캐시를
  정의로부터 다시 만든다 (`types.rs:921-966`).
- **[확인]** 현재 서버는 DB나 디스크 복원을 하지 않지만 `GET /games/:id`
  응답과 프론트 `GameState`에는 정의가 포함된다.
- **[제안]** 이 현재 형식을 “게임 정의 스냅샷”의 엔진 표현으로 유지한다.
  복원 시 저장소 최신 버전이나 기본 카탈로그로 대체하지 말고 스냅샷
  검증 후 캐시만 재생성한다.

## 4. 발견한 결합 지점과 위험

| 지점 | 현재 사실 | 위험 | 필요한 경계 |
|---|---|---|---|
| parser | `parse -> Program`, 오류 없음 | 잘못된 코드가 빈/부분 행마로 보임 | `compile -> Result<CompiledPieceSet, Diagnostics>` |
| runtime | 명령/결과 예산 없음 | CPU·메모리 고갈 | 명시적 `ExecutionLimits`와 `LimitExceeded` |
| transition | 태그가 결과에 반영되지 않음 | 다중 내부 상태 기물 불가 | 검증된 `PieceTypeTransition` 효과 |
| catalog | `GameState` 직접 참조와 미사용 `GameContext` 공존 | 정의 집합 분기 | 게임 소유 `RuntimePieceCatalog` 하나 |
| game build | 서버 기본 정의 allowlist | 커스텀 참조 불가 | 버전 참조 resolve 후 엔진 생성 |
| special rules | pawn/rook ID 문자열 하드코딩 | ID 충돌/특권 획득 | custom namespace + capability 비부여 |
| persistence | 메모리만 존재 | 재시작/재접속 손실 | repository + game snapshot 저장 경계 |
| auth | self-issued `client_id` | 소유권 위조 | `AuthenticatedUserId` extractor |
| deck | 타입 문자열만 저장 | 수정 후 의미 변동 | `{package_id, version, exposed_piece_key}` |
| image | 정적 import만 존재 | 업로드 안전성 없음 | image service와 sanitized asset key |
| frontend | 수동 타입 복제 | DTO drift | 계약 테스트 또는 향후 생성 도입 |

그 밖의 즉시 처리 위험:

- custom 내부 키가 `king`, `rook`, `pawn-white` 등 기본/특수 ID와 충돌하면
  안 된다.
- `PieceDefinition::normalize_and_validate`는 참조 무결성을 일부 보지만
  체섬블리 구문, 외부 정의 참조, 전체 패키지 중복 키를 검증하지 않는다.
- `ExecutionContext`가 모든 정의와 기물을 받으므로 실행기는 읽기 전용이어도
  정보 범위가 넓다. 공개 오류에 내부 식별자 전체를 노출하지 않는다.
- `danger()` 계산은 기존 attack map 주입에 의존하며 일부 호출은 빈 map을
  넘긴다. 2단계에서 의미를 바꾸지 말고 회귀 테스트로 현재 동작을 고정한다.

## 5. 제안 도메인 모델

계층별 타입을 하나로 합치지 않는다. 이름은 현재 Rust snake_case JSON,
TypeScript camelCase 관례를 각각 따른다.

### 5.1 서버 저장 모델

```rust
pub struct CustomPiecePackageRecord {
    pub id: CustomPiecePackageId,       // UUID, 불변
    pub owner_id: UserId,
    pub name: String,
    pub description: String,
    pub score: u32,
    pub image: CustomPieceImageRecord,
    pub latest_version: u32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub struct CustomPieceVersionRecord {
    pub package_id: CustomPiecePackageId,
    pub version: u32,                   // package 안에서 단조 증가, 불변
    pub content_hash: ContentHash,      // canonical compile input의 SHA-256
    pub raw_script: String,             // 입력 그대로 보존
    pub exposed_piece_key: LocalPieceKey,
    pub internal_piece_keys: Vec<LocalPieceKey>,
    pub compiled_definitions: Vec<PieceDefinition>,
    pub created_at: Timestamp,
}

pub enum CustomPieceImageRecord {
    BuiltIn { asset_key: String },
    Uploaded { asset_id: ImageAssetId, media_type: SafeImageMediaType },
}
```

- **[제안]** `name`, `description`, `score`, 이미지, 스크립트 또는 exposed key
  중 의미에 영향을 주는 필드가 바뀌면 새 불변 버전을 만든다. 이전 버전을
  수정하지 않는다.
- **[제안]** `raw_script`는 줄바꿈과 공백을 포함해 원문 그대로 저장한다.
  hash 계산은 별도 canonical envelope에 대해 수행하며 원문을 덮어쓰지
  않는다.
- **[제안]** compiled definitions는 파생 데이터다. 저장할 수는 있지만
  compiler/runtime version과 hash를 함께 검사하고 불일치하면 원문에서
  재컴파일한다.

### 5.2 엔진 컴파일 모델

```rust
pub struct CustomPieceSource<'a> {
    pub package_id: &'a str,
    pub version: u32,
    pub raw_script: &'a str,
    pub exposed_piece_key: &'a str,
    pub declared_score: u32,
}

pub struct CompiledPieceSet {
    pub exposed_piece_key: LocalPieceKey,
    pub definitions: Vec<PieceDefinition>,
    pub internal_piece_keys: Vec<LocalPieceKey>,
}

pub struct CompileReport {
    pub compiled: CompiledPieceSet,
    pub warnings: Vec<ChessemblyDiagnostic>,
}

pub enum CompilePieceSetError {
    Syntax(Vec<ChessemblyDiagnostic>),
    Semantic(Vec<ChessemblyDiagnostic>),
    Limit(CompileLimitKind),
}
```

2단계 시작 시 현재 스크립트 형식은 기물 하나만 생성하므로 첫 구현은
`definitions.len() == 1`, `exposed_piece_key == 그 정의의 local key`를
지원해도 된다. 단, 반환 타입은 처음부터 `CompiledPieceSet`으로 두어
다중 정의 패키지를 추가할 때 DB/API/게임 계약을 깨지 않는다.

### 5.3 게임/덱 참조 모델

```rust
pub enum DeckPieceRef {
    BuiltIn { piece_type_id: PieceTypeId },
    Custom {
        package_id: CustomPiecePackageId,
        version: u32,
        exposed_piece_key: LocalPieceKey,
    },
}

pub struct ResolvedCustomPieceVersion {
    pub package_id: CustomPiecePackageId,
    pub version: u32,
    pub content_hash: ContentHash,
    pub exposed_type_id: PieceTypeId,
    pub definitions: Vec<PieceDefinition>,
}
```

런타임 전역 ID는 충돌을 막기 위해 서버가 다음처럼 만든다.

```text
custom:{package_uuid}:v{version}:{local_piece_key}
```

스크립트 내부 참조는 local key로 검증한 뒤 컴파일/resolve 단계에서 이
전역 ID로 바꾼다. 사용자가 전역 ID, owner ID 또는 다른 package namespace를
직접 선언할 수 없다.

## 6. 런타임 기물 카탈로그 설계

### 6.1 소유권과 불변조건

**[제안]** `GameState`가 게임에 필요한 모든 `PieceDefinition` 스냅샷을
계속 소유한다. 이를 감싼 `RuntimePieceCatalog`를 엔진의 유일한 정의 조회
경계로 정한다.

```rust
pub struct RuntimePieceCatalog {
    definitions: HashMap<PieceTypeId, PieceDefinition>,
    programs: ChessemblyProgramCache,
    source_manifest: Vec<PieceDefinitionSource>,
}

pub enum PieceDefinitionSource {
    BuiltIn { engine_catalog_version: String },
    Custom {
        package_id: String,
        version: u32,
        content_hash: String,
    },
}

impl RuntimePieceCatalog {
    pub fn build(
        built_ins: impl IntoIterator<Item = PieceDefinition>,
        custom: impl IntoIterator<Item = ResolvedCustomPieceVersion>,
    ) -> Result<Self, CatalogBuildError>;
    pub fn definition(&self, type_id: &str) -> Option<&PieceDefinition>;
    pub fn program(&self, type_id: &str, layer_id: &str)
        -> Result<Arc<Program>, CatalogLookupError>;
    pub fn snapshot(&self) -> RuntimeCatalogSnapshot;
    pub fn restore(snapshot: RuntimeCatalogSnapshot)
        -> Result<Self, CatalogRestoreError>;
}
```

불변조건:

1. 모든 `Piece.type_id`, promotion target, transition target와 내부 참조가
   catalog에 존재한다.
2. built-in ID와 custom namespace는 충돌하지 않는다.
3. `(package_id, version)`별 content hash가 manifest와 일치한다.
4. 모든 정의는 normalize/semantic validation을 통과한다.
5. 프로그램 캐시는 정확히 해당 정의 snapshot에서 파생된다.
6. lookup 실패는 빈 합법 수가 아니라 명시적 오류다.

### 6.2 실행 흐름

2단계의 목표 호출 흐름:

```text
RuntimePieceCatalog::build/restore
  -> GameContext { state, catalog }
     -> legal move
     -> move validation
     -> move apply / transition
     -> attack map
     -> pocket drop
     -> AI clone/search
  -> RuntimeCatalogSnapshot과 GameState를 함께 저장
```

- `legal_moves.rs`, `attack_map.rs`, `placement.rs`, `rules.rs`,
  `endgame.rs`, `ai/*`가 모두 같은 `&GameContext` 또는 같은 catalog 참조를
  받게 한다.
- 2단계에서 공개 API를 한 번에 깨지 않도록 기존 `&GameState` 함수는
  context를 만드는 호환 wrapper로 남길 수 있다. wrapper와 내부 함수가
  서로 다른 catalog를 만들지는 않아야 한다.
- preview는 `&GameContext`와 immutable state를 사용해
  `Vec<MoveAction>`만 반환한다. apply는 그 action을 다시 canonical
  validation한 뒤 새 state를 반환한다.
- AI state clone에는 catalog snapshot/manifest가 보존돼야 한다. 컴파일
  프로그램 캐시는 clone 또는 재구축 가능한 파생 자원이다.

### 6.3 전환 효과

```rust
pub struct PieceTypeTransition {
    pub piece_id: PieceId,
    pub target_type_id: PieceTypeId,
}
```

- transition target은 같은 package version의 internal key 또는 명시적으로
  허용된 built-in만 가능하다. 기본값은 같은 package 내부만 허용한다.
- 후보 생성 시 target 존재 여부와 target state schema를 검증한다.
- 실제 action commit에서만 `Piece.type_id`를 바꾸고 새 정의의 state schema를
  초기화한다. 미리보기/선택/취소는 state를 바꾸지 않는다.
- 기존 상태 보존이 필요해질 때는 암시적 key 복사를 하지 않고 별도
  migration 계약을 추가한다.

## 7. 저장, 버전과 재현성 정책

채택 정책은 **불변 버전 레코드 + 콘텐츠 해시 + 게임 시작 시 전체 정의
스냅샷**의 조합이다.

| 방식 | 장점 | 단점 | 결정 |
|---|---|---|---|
| 불변 버전만 | 덱 참조가 안정적 | 저장소 없이 게임 복원 불가 | 사용 |
| 전체 snapshot만 | 게임 독립 복원 | 중복 저장, 소유권/버전 추적 약함 | 사용 |
| hash만 | 무결성 확인 | 원본/정의 자체 복원 불가 | 보조로 사용 |

정책:

- 덱에 커스텀 기물을 추가할 때 당시 최신 버전을 **명시적으로 고정**한다.
- 제작자가 새 버전을 만들더라도 기존 덱은 자동 변경되지 않는다.
- 덱 편집기는 “새 버전 사용 가능”을 표시하고 사용자가 승인할 때만 참조를
  갱신한다.
- 게임 생성 시 서버는 두 덱의 고정 버전을 소유권/사용 정책에 따라 resolve,
  재검증하고 built-in과 합쳐 snapshot을 만든다.
- 저장된 게임은 package 저장소의 최신 레코드를 조회하지 않고 snapshot으로
  복원한다.
- snapshot에는 raw script까지 중복하지 않아도 되지만 재현에 필요한
  normalized definitions, dialect/compiler version, package/version/hash,
  이미지 asset 식별자를 포함한다. 원문은 version 저장소에서 영구 보존한다.
- snapshot hash 불일치, 참조 누락, 지원하지 않는 compiler version은
  fail-safe 복원 오류이며 기본 기물로 대체하거나 빈 행마로 진행하지 않는다.

## 8. 서버 API 계약

현재 API의 snake_case JSON과 `{ "error": "사용자용 메시지" }` 스타일을
유지한다. 다음 라우트는 `/api/custom-pieces` 아래 둔다.

공통 공개 DTO:

```ts
interface CustomPieceSummaryDto {
  id: string
  name: string
  description: string
  score: number
  image: CustomPieceImageDto
  latest_version: number
  exposed_piece_key: string
  content_hash: string
  created_at: string
  updated_at: string
}

interface CustomPieceDetailDto extends CustomPieceSummaryDto {
  owner_id: string
  raw_script: string
  internal_piece_keys: string[]
}

interface ValidationDiagnosticDto {
  severity: 'error' | 'warning'
  code: string
  message: string
  span?: { start: number; end: number; line: number; column: number }
}
```

### 8.1 CRUD

| 기능 | HTTP | 요청 | 성공 응답 |
|---|---|---|---|
| 목록 | `GET /custom-pieces` | 없음; 인증 사용자로 scope | `{items: CustomPieceSummaryDto[]}` |
| 상세 | `GET /custom-pieces/:id` | 없음 | `CustomPieceDetailDto` |
| 생성 | `POST /custom-pieces` | `{name, description, score, image_ref, raw_script, exposed_piece_key}` | `201 CustomPieceDetailDto` |
| 수정 | `PUT /custom-pieces/:id` | 생성과 같은 전체 입력 + `expected_version` | 새 version의 `CustomPieceDetailDto` |
| 삭제 | `DELETE /custom-pieces/:id` | `expected_version` | `204` |

- 생성/수정은 서버에서 다시 compile/validate한다.
- owner ID, content hash, compiled definitions, internal keys는 요청에서 받지
  않는다.
- 삭제는 package를 새 덱에서 숨기는 soft delete가 기본값이다. 불변 version과
  기존 game snapshot은 유지한다.
- `expected_version` 불일치는 `409`; 소유권 위반은 존재 정보 노출을 줄이기
  위해 `404` 또는 정책화된 `403`; validation은 `422`를 사용한다.

### 8.2 검증과 테스트 보드

| 기능 | HTTP | 요청 | 성공 응답 |
|---|---|---|---|
| 코드 검증 | `POST /custom-pieces/validate` | `{raw_script, exposed_piece_key, score}` | `{valid, diagnostics, exposed_piece_key, internal_piece_keys, preview_definitions}` |
| 보드 상태 검증 | `POST /custom-pieces/test/validate-board` | `{draft, board}` | `{valid, diagnostics, normalized_board?}` |
| 이동 계산 | `POST /custom-pieces/test/options` | `{draft, board, selected_piece_id, move_option_id?}` | 기존 lab 응답 형태 + diagnostics |
| 행동 적용 | `POST /custom-pieces/test/actions` | `{draft, board, action: {type, piece_id, to, promotion?, move_option_id?}}` | `{state, legal_moves, legal_drops, attacks}` |

여기서 `draft`는 저장 ID가 아니라 검증할 raw source와 metadata다. 서버는 매
요청 bounded compile을 수행하거나 사용자/내용 hash 기준의 짧은 캐시만
사용한다. 클라이언트가 보낸 `PieceDefinition`, capture, effects를 신뢰하지
않는다.

테스트 행동 적용은 기존 `/lab/piece-options`처럼 서버가 만든 canonical
action과 선택 데이터가 정확히 일치할 때만 수행한다. options 조회는 절대
state를 변경하지 않는다.

### 8.3 이미지

```text
POST /api/custom-piece-images
Content-Type: multipart/form-data
file=<binary>
-> 201 { asset_id, media_type, width, height, content_hash }
```

기본 이미지는 업로드하지 않고 allowlist `asset_key`를 package 요청에
참조한다. 업로드 응답의 `asset_id`도 현재 사용자에게 귀속되며 package
저장 시 다시 소유권을 검사한다.

## 9. 프론트엔드 구조

현재 `AppView` 분기 방식을 시제품 동안 유지하되 제작 도구를 한 컴포넌트에
몰아넣지 않는다.

```text
views/
  CustomPieceLibrary.vue       목록, 생성/편집/삭제 진입
  CustomPieceEditor.vue        화면 조합, dirty/version 충돌 처리
components/custom-piece/
  CustomPieceMetadataForm.vue  이름/설명/점수
  ChessemblyEditor.vue         원문 편집(고급 IDE 아님)
  ValidationPanel.vue          diagnostics와 대표 기물 선택
  PieceImagePicker.vue         기본 asset/업로드
  CustomPiecePreview.vue       대표 이미지/메타 미리보기
  CustomPieceTestBoard.vue     테스트 보드 조합
composables/
  useCustomPieceDraft.ts       draft, dirty, validation 상태
  useCustomPieceTestSession.ts 보드 상태와 권위 API 응답
api/
  customPieceApi.ts            CRUD/validate/image/test 계약
types/
  customPiece.ts               API DTO와 UI draft
```

- 기존 `PieceLab.vue`의 보드 상호작용은 재사용 후보지만 1,590줄 화면을
  통째로 중첩하지 않는다. 4단계에서 공통 보드 조작 부분만
  `CustomPieceTestBoard` 또는 composable로 국소 추출한다.
- 코드 입력 원문과 마지막 검증 성공 hash를 별도로 유지한다. 입력 변경 시
  이전 검증 결과는 stale로 표시한다.
- 대표 기물 선택지는 서버 compile 결과의 internal keys만 사용한다.
- 이미지 preview는 브라우저 검사 결과를 “안전함”으로 표시하지 않는다.
  서버 업로드 성공 asset만 저장 가능 상태로 승격한다.
- 덱 편집기는 built-in catalog와 사용자 package summaries를 별도 section으로
  보여주고 `DeckPieceRef`를 저장한다. 표시용 exposed key만으로 version을
  잃지 않는다.
- 게임 보드는 이미 `GameState.piece_definitions`와 `pieceAssets.ts`를 통해
  시각을 해석한다. custom asset은 `asset_key -> 안전한 서버 URL` resolver를
  추가하되 정의의 임의 URL을 직접 렌더링하지 않는다.
- 인증 상태 계층은 현재 없다. 계정 도입 전에는 제작/저장 UI를 기능 플래그
  뒤에 두거나 명시적 임시 사용자 provider를 주입한다.

## 10. 권한과 보안 경계

### 10.1 시제품에서 즉시 필요한 제한

정확한 수치는 2단계 benchmark로 조정할 수 있으나 정책 객체와 오류는
처음부터 둔다. 권장 안전 기본값:

```rust
pub struct ChessemblyLimits {
    pub max_source_bytes: usize,       // 64 KiB
    pub max_tokens: usize,             // 16_384
    pub max_ast_nodes: usize,          // 16_384
    pub max_block_depth: usize,        // 32
    pub max_execution_steps: u64,      // 100_000 / piece evaluation
    pub max_generated_squares: usize,  // board_size², hard cap 256
    pub max_internal_definitions: usize, // 16
}
```

- 파싱/의미 오류, limit 초과, 존재하지 않는 참조는 재시도하지 않고
  diagnostics를 반환한다.
- 실행 step은 모든 expression, jump, repeat, while, nested block 평가마다
  차감한다. anchor 변화 감지는 보조 안전장치일 뿐 예산을 대체하지 않는다.
- board size는 기존 lab의 `8..=12`를 시제품 테스트 기본 범위로 유지한다.
- custom 정의는 `is_king`, built-in special rule capability,
  `can_capture_on_drop` 같은 권한성 필드를 raw script만으로 획득하지 못한다.
- package/버전/이미지 CRUD마다 인증 user와 owner를 비교한다.
- 서버 내부 parser/runtime 오류는 correlation ID와 redacted log에 남기고
  사용자에게 안정적인 error code와 span만 보낸다.

이미지 즉시 제한:

- 파일명/확장자가 아니라 decode된 실제 MIME 확인
- 허용 MIME: `image/svg+xml`, `image/png`, `image/jpeg`
- 압축 전송 크기, decode 후 width/height/pixel count 각각 제한
- 손상/다중 형식(polyglot) 거부
- SVG의 script, event handler, `foreignObject`, 외부 URL/resource,
  entity/DOCTYPE, 위험한 HTML/XML 제거 또는 fail-closed 거부
- 원본 파일명을 저장 경로로 사용하지 않고 서버 생성 asset ID 사용
- SVG는 정제본만 제공하고 `Content-Type`, CSP, nosniff를 설정

### 10.2 후속 강화

- malware scanning 및 이미지 재인코딩 격리 worker
- 사용자별 저장량/요청 rate limit과 abuse audit
- compiler/runtime 버전별 sandbox process 또는 WASM 격리
- 공유/마켓이 추가될 때 moderation과 공개 범위 정책

## 11. 단계별 변경 예정 파일

실제 구현 시 책임이 커지는 경우에만 파일을 나눈다.

### 2단계: 엔진과 체섬블리 런타임

- `engine/src/chessembly/parser.rs`: 진단 가능한 parse 결과와 parse limits
- `engine/src/chessembly/ast.rs`: 필요한 package/transition IR
- `engine/src/chessembly/interpreter.rs`: execution budget와 runtime error
- `engine/src/chessembly/mod.rs`: compile/run 공개 계약
- `engine/src/types.rs`: transition effect, snapshot 직렬화 타입
- `engine/src/context.rs`: 실제 `RuntimePieceCatalog`/`GameContext` 경계로 정리
- `engine/src/legal_moves.rs`, `attack_map.rs`, `placement.rs`,
  `endgame.rs`, `rules.rs`, `ai/*`: 동일 catalog 전달
- `engine/tests/chessembly_compat.rs`: 기존 문법 회귀와 오류/limit
- 새 `engine/tests/custom_piece_runtime.rs`: namespace, 다중 정의,
  transition, snapshot/restore

### 3단계: 서버, 저장소, API

- `server/src/app_state.rs`, `stores.rs`: repository trait/임시 구현 주입
- `server/src/routes.rs`: custom piece/image/test 라우트
- `server/src/main.rs`: 기존 거대 파일에서 DTO/handler를 관련 모듈로 국소 분리
- 새 `server/src/custom_pieces/{mod,dto,service,repository}.rs`
- DB 도입 시에만 migration 파일 추가. 1단계에서 특정 DB를 가정하지 않는다.

### 4단계: 프론트 제작 도구

- `frontend/src/App.vue`, `types/deck.ts`
- 새 `types/customPiece.ts`, `api/customPieceApi.ts`
- 새 library/editor view, custom-piece components/composables
- `PieceLab.vue`의 재사용 가능한 보드 책임만 국소 추출

### 5단계: 덱과 멀티플레이어 연동

- `useSavedDecks.ts`: v1 문자열 덱을 built-in ref로 읽는 호환 migration
- `useDeckSerialization.ts`, `useDeckValidation.ts`, `DeckEditor.vue`
- `MultiplayerLobby.vue`, `gameApi.ts`
- 서버의 create/select/ready/start 경로와 game snapshot 저장

## 12. 테스트 전략

### 12.1 엔진

- 기존 `chessembly_compat`, `rule_engine`, `ai` 전체 회귀
- 잘못된 토큰/인자/괄호가 빈 이동이 아니라 정확한 diagnostic을 반환
- source/token/AST/depth/step/result limit 각각의 경계값과 초과 오류
- 한 package의 exposed/internal key resolve와 namespace 충돌 거부
- transition은 preview에서 불변, commit에서만 적용
- legal move, attack map, drop, apply, AI가 같은 custom catalog를 사용
- JSON snapshot round-trip 후 같은 legal actions/hash를 생성
- 누락/변조 정의 복원 fail-safe
- 기존 `PieceDefinition` JSON과 built-in 행마가 변하지 않는 회귀 테스트

### 12.2 서버

- 인증/소유권별 list/detail/update/delete
- 저장 시 클라이언트의 hash/compiled result를 신뢰하지 않는 테스트
- optimistic version conflict
- image MIME/decode/dimension/SVG 공격 fixture
- test options가 상태를 바꾸지 않고 test action만 상태를 변경
- repository 구현 계약 테스트를 메모리/향후 DB 구현에 공통 적용
- 공개 오류에 path/stack/raw sensitive content가 없는지 확인

### 12.3 프론트

- DTO decoding/오류 표시
- raw script 보존과 stale validation 상태
- custom ref의 version round-trip 및 v1 built-in deck migration
- 최신 버전 자동 추종 금지
- 서버 승인 전 이미지/기물을 저장 가능으로 표시하지 않음
- 실제 `npm test`, `npm run build`로 타입 drift 확인

## 13. 결정되지 않은 사항과 권장 기본값

| 사항 | 현재 확인 상태 | 권장 기본값 |
|---|---|---|
| 실제 DB/provider | 없음 | repository 경계 후 3단계에서 선택 |
| 실제 계정 ID | 없음 | 영구 저장 전 auth provider 결정; `client_id`를 owner로 쓰지 않음 |
| script의 다중 정의 문법 | 없음 | 2단계는 단일 정의 compile, 결과 타입은 set |
| custom 왕/특수 능력 | 정책 없음 | custom `is_king=false`, built-in 특권 금지 |
| internal key의 built-in 참조 | 정책 없음 | 기본 deny, 필요한 경우 allowlist |
| 정확한 runtime limit | benchmark 없음 | 10.1의 보수적 값으로 시작하고 정책 객체로 조정 |
| 이미지 저장 backend | 없음 | opaque asset service 계약; 로컬 경로를 API에 노출하지 않음 |
| 삭제 후 기존 덱 | 저장소 없음 | package soft delete; 기존 snapshot 게임은 복원, 새 게임 resolve는 차단 |
| TypeScript 타입 생성 | 없음 | 시제품은 수동 DTO + 계약 테스트, 이후 생성 검토 |

### 다음 엔진 단계의 최소 완료 인터페이스

추가 질문 없이 2단계를 시작하기 위한 우선순위는 다음과 같다.

1. `parse/compile`을 diagnostics와 limits가 있는 `Result` 계약으로 만든다.
2. `CompiledPieceSet`을 도입하되 첫 지원 문법은 단일 정의로 제한한다.
3. 전역 namespace resolve와 모든 참조 무결성 검사를 구현한다.
4. `RuntimePieceCatalog::build/snapshot/restore`를 구현하고 cache를 그
   catalog의 파생 자원으로 만든다.
5. legal/attack/drop/apply/AI를 같은 catalog context에 연결한다.
6. transition을 action effect로 운반하고 commit 시에만 적용한다.
7. 기존 public wrapper와 serde를 유지해 기본 기물/기존 API 회귀를 막는다.
8. 위 흐름을 runtime/snapshot/호환성 테스트로 고정한다.

이 단계에서는 서버 CRUD, DB, 이미지 업로드와 제작 UI를 구현하지 않는다.
