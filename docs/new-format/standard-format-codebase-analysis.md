# Standard Format — G0 코드베이스 분석

조사일: 2026-09-09~10 (Asia/Seoul) · 기준 HEAD: `50db27d09c6ec3dfa580af5e18057a567889e65f`

이 문서는 `AGENTS.md`와 같은 디렉터리의 `STANDARD_FORMAT_IMPLEMENTATION_PLAN.md`를 먼저 읽고 수행한 **G0 분석 결과**다. 구현, 설정 변경, migration 실행, Worktree 생성은 하지 않았다. 계획 본문에는 산출물이 `docs/standard-format-codebase-analysis.md`로 적혀 있지만, 이번 사용자의 명시적 경로인 `docs/new-format/standard-format-codebase-analysis.md`를 따른다. 계획 문서는 변경하지 않았다.

**근거 표기:** 아래 현재 구조 설명은 **[확인]**으로, 이후 변경 파일·설계 방향은 **[예상/권장]**으로 구분한다. **[미확정]**은 계획 또는 조사만으로 결정할 수 없는 규칙이다. `경로:행`은 위 HEAD의 실제 코드 위치이며, 경로는 저장소 루트 기준이다. 테스트 통과는 §10의 실행 범위에 한정한다. 운영 서비스 및 실제 저장 데이터의 상태는 조사하지 않았다.

## 1. 구현 전 반드시 알아야 할 사실

1. **[확인] 현재 선택 단위는 맵이다.** `SavedDeck`은 `mapId`와 `boardSize`를 함께 저장하고, 덱 에디터는 맵을 선택하여 크기를 도출한다. `standard-8x8`~`standard-12x12`는 **기존 일반전 맵 ID**이며 새 Standard 룰셋이 아니다. 별도로 `central-high-ground-12x12`가 있다. `BoardVariant` 역시 지형 구분이다. (`frontend/src/boardMaps.ts:11`, `frontend/src/views/DeckEditor.vue:31`, `engine/src/rules.rs:13`)
2. **[확인] 현재 Deck/GameState에는 Legacy/Standard 룰셋, Hand, Extra Deck, Draw 상태가 없다.** `GameRecord.game_mode = standard`는 Challenge와 구분하는 기존 일반 게임 모드이고, `ruleset_version = deck-chess-1`은 기존 기록 버전 문자열이다. 이를 새 룰셋으로 재해석하면 안 된다. (`engine/src/types.rs:889,1067`, `server/src/game_record.rs:14,28`)
3. **[확인] 저장 가능한 덱과 게임 가능한 덱은 다르다.** 미완성 덱도 저장할 수 있지만, 게임 생성 시 서버가 기물 정의를 해석하고 초기 배치·King·점수를 재검증한다. (`frontend/src/composables/useDeckValidation.ts:444,449`, `server/src/deck.rs:381`, `server/src/main.rs:551`)
4. **[확인] 기존 초기 진영은 8·9 보드에서 2줄, 10·11·12에서 3줄이며, 가장 앞줄 전체를 채워야 한다.** README의 항상 2줄이라는 설명보다 실제 코드와 회귀 테스트를 기준으로 삼아야 한다. (`engine/src/rules.rs:126,230`, `engine/tests/rule_engine.rs:928,939,972`)
5. **[확인] 최신 Deck Code는 DC3이다.** README의 DC2 설명만 따라서는 이름 및 커스텀 기물 참조를 누락한다. DC1/DC2 읽기도 존재한다. (`frontend/src/composables/useDeckCodeCodec.ts:5,115,139`)
6. **[확인] action 1개와 턴 전환 1개는 항상 같지 않다.** 폭격기의 강제 착륙이 남아 있으면 같은 플레이어가 이어서 행동한다. Draw 위치를 결정할 때 이 예외를 보존해야 한다. (`engine/src/endgame.rs:206`, `server/src/main.rs:5517` 테스트)
7. **[확인] 본게임 Replay는 초기 상태 + 서버 상태 차분으로 복원하지만, 분석 분기는 action을 엔진에서 다시 실행하고 상태 해시를 검증한다.** 랜덤 도입은 두 경로 모두 고려해야 한다. (`server/src/game_record.rs:319,355`, `server/src/main.rs:2620`)

## 2. Board Size: 덱 빌더에서 게임 생성까지

### 2.1 선택·저장·복원

**[확인] 다음 경로가 현재 구현되어 있다.**

```text
boardMaps[] { id, boardSize, variant }
  → createNewSavedDeck(mapId = standard-8x8)
  → SavedDeck { mapId, boardSize, starting, pocket, customPieces, ... }
  → DeckEditor <select v-model="deck.mapId" @change="changeMap">
  → findBoardMap(mapId).boardSize
  → 크기가 다르면 boardSize 갱신 + resetToClassic()
  → useSavedDecks → LocalDeckRepository 또는 AccountDeckRepository
  → 재로딩한 SavedDeck → validateSavedDeck → 게임별 선택 화면
```

| 단계 | 실제 파일·심벌 | 책임과 주의점 |
| --- | --- | --- |
| 맵 목록 | `frontend/src/boardMaps.ts:11`, `engine/src/rules.rs:13` | 프론트/엔진에 각각 6개 맵 정의. 8~12 일반전, 12 중앙 고지전 |
| 신규 덱 | `frontend/src/composables/localDeckRepository.ts:60` `createNewSavedDeck` | 맵에서 크기를 얻어 해당 크기 프리셋 생성. 이름·ID·시간·빈 customPieces 초기화 |
| 맵 변경 | `frontend/src/views/DeckEditor.vue:482` `changeMap` | 같은 크기의 맵끼리 전환할 때 내용 보존. 크기가 바뀌면 시작 기물/포켓을 classic으로 교체 |
| 에디터 복사 | `frontend/src/views/DeckEditor.vue:461` `cloneSavedDeck` | 필드를 직접 열거하므로 새 ruleset/zone을 추가해도 자동 복사되지 않음 |
| 점수 상한 | `frontend/src/composables/useDeckValidation.ts:279`, `engine/src/rules.rs:102` | `n*n - 25`. 8→39, 9→56, 10→75, 11→96, 12→119 |
| 구형 맵 보완 | `frontend/src/boardMaps.ts:34` `normalizeBoardMapId` | 누락뿐 아니라 알 수 없거나 크기가 맞지 않는 맵도 같은 크기 일반전으로 fallback |
| 서버 맵 해석 | `server/src/main.rs:121` `resolve_board_map` | 명시한 맵은 존재/크기 일치 검사. 누락 시 기존 board_variant와 크기로 복원. 명시한 잘못된 맵은 오류 |

프론트의 맵 정규화와 서버의 명시값 검사는 동일하지 않다. **[권장] 새 ruleset의 누락값 호환 정책을 만들 때 기존 맵 fallback을 그대로 복제하지 말고, 누락과 알 수 없는 명시값을 구분한다.**

### 2.2 로컬 2인·일반 Bot

**[확인]** `DeckSelect.vue:128`에서 `validateSavedDeck`이 통과한 덱을 선택하며, `sameMap`은 두 덱의 `mapId`를 비교한다. `App.vue:210`의 `getValidDeck`이 선택한 덱을 다시 읽고 검증한다. `startSingleGame`/`startConfiguredBotGame`은 `ensureSameMap`을 통과한 덱으로 다음 요청을 보낸다.

```text
SavedDeck 2개
 → App.startSingleGame / startConfiguredBotGame
 → serializeNeutralDeck(deck, side)
 → api.createGame(boardSize, whiteDeck, blackDeck, mapId, timeControl, metadata)
 → POST /api/games
   { board_size, map_id, white_deck, black_deck, time_control, ... }
 → CreateGameRequest
 → resolve_custom_packages + resolve_board_map
 → build_game_state_with_variant(..., true, true)
 → create_board_with_variant(board_size, variant)
 → build_player_deck(white), build_player_deck(black)
 → engine.validate_deck
 → StoredGame::new_with_players_and_deck_names(..., map_id)
 → GameView → frontend GameState
```

근거: `frontend/src/App.vue:235,272`, `frontend/src/composables/useDeckSerialization.ts:23,43`, `frontend/src/api/gameApi.ts:291`, `server/src/main.rs:55,1668,684`, `server/src/time_control.rs:366,418`.

**[확인] 게임용 PlayerDeckRequest/PlayerDeckSpec에는 mapId/boardSize가 없다.** 크기·맵은 게임 생성 요청의 바깥 필드다. 서버는 저장 덱 ID를 조회해 생성하지 않고 제출된 starting/pocket 내용을 재검증한다. 그러므로 두 저장 덱의 원래 맵이 같았다는 사실은 현재 payload만으로 서버가 확인할 수 없다. 서버가 확인하는 것은 요청 맵/크기 일치 및 그 보드에서의 덱 합법성이다.

**[확인] 좌표는 중립 덱에서 White 기준이다.** 로컬 2인/Bot에서는 클라이언트가 Black의 rank를 `n-1-rank`로 바꾼다. file은 뒤집지 않는다. 수량 맵인 pocket은 기물 참조 배열로 펼쳐진다. 내장 기물은 `piece_type`, 커스텀 기물은 ID/version/content_hash/exposed_piece_key만 전송하며 소스나 점수를 보내지 않는다.

### 2.3 Multiplayer

**[확인]** `MultiplayerLobby.vue:152` → `api.createRoom`은 선택 덱의 크기·맵과 **중립 좌표 덱**을 함께 보낸다. 서버 `CreateRoomRequest`/`MultiplayerRoom`이 크기·맵·variant를 보관한다. 참가/재선택 요청은 `client_id + deck`이며 별도의 크기·맵이 없다.

```text
createRoom(boardSize, neutralDeck, hostSide, mapId, timeControl)
 → room { board_size, map_id, board_variant, host_deck, guest_deck, ... }
 → joinRoom / selectRoomDeck / readyRoom
 → 양측 준비 시 start_room_game
 → 서버 materialize_neutral_deck(side, room.board_size)로 Black 미러링
 → build_game_state_with_variant(..., true, true)
```

근거: `frontend/src/views/MultiplayerLobby.vue:195,226`의 맵 비교, `frontend/src/api/gameApi.ts:426,463,473`, `server/src/main.rs:108,146,1020,1116,1744,1813,1874,1933`.

방 생성·덱 재선택만으로 `validate_deck`를 호출하지 않는다. 최종 시작에서 검사한다. **[권장] G1에서는 룸의 ruleset과 제출 덱의 ruleset 계약을 명시해야 하며, 클라이언트의 같은 포맷 필터만으로 서버 검증을 대신하지 않는다.**

### 2.4 권위 있는 상태가 만들어지는 지점

**[확인]** `build_game_state_with_variant`는 보드와 기본 정의를 만들고 `install_runtime_catalog`로 커스텀 정의/manifest를 설치한다. 이후 양측 덱을 실제 `PieceId`로 구체화한다. 시작 상태는 White, `turn_number=1`, Playing, 빈 history/global_state, result 없음이다. 생성 성공 후에만 게임 저장소에 넣는다.

`GameState.board.size`가 실행 중 크기의 원본이다. `GameState`/엔진 `Deck` 자체에는 `map_id`가 없다. 지형은 `board.terrain`, 맵 이름은 룸 및 `GameRecord.decks[side].map_id` 등에 남는다. `GameStore`는 `Arc<DashMap<String, StoredGame>>`인 메모리 저장소다. 대국 기록 영속 저장과 실행 중 게임 저장소는 별개다. (`server/src/stores.rs:9`, `server/src/game_record.rs:432`)

## 3. Deck / GameState / Pocket / Drop의 데이터 계약

### 3.1 덱의 세 가지 표현

**[확인]** 같은 이름의 덱이라도 편집·요청·실행 데이터가 다르다.

| 표현 | 실제 구조 | 책임 |
| --- | --- | --- |
| `LobbyDeck` | `starting: {pieceType, square}[]`, `pocket: Record<DeckPieceType, number>`, optional `customPieces` | 중립 좌표·종류별 수량으로 편집 |
| `SavedDeck extends LobbyDeck` | `id`, `name`, `mapId`, `boardSize`, `createdAt`, `updatedAt`, `customPieces`, optional `version` | 로컬/계정 저장. version은 계정의 낙관적 동시성 버전 |
| 서버 `DeckData` | camelCase JSON: `mapId`, `boardSize`, `starting`, `pocket: BTreeMap<String,u32>`, `customPieces` | 저장 입력의 엄격한 스키마. `deny_unknown_fields` |
| `PlayerDeckSpec` | optional name, `starting: Vec<StartingPieceSpec>`, `pocket: Vec<DeckPieceRef>` | 게임 생성에 쓰는 내장/커스텀 참조 |
| 엔진 `Deck` | `player_id`, `starting_pieces: Vec<PieceId>`, `pocket_pieces: Vec<PieceId>`, `score_limit: u32`, `total_score: u32` | 게임에 구체화된 기물 ID 목록과 시작 점수 |
| 엔진 `Player` | `id`, `deck`, `captured_pieces: Vec<PieceId>` | 플레이어별 상태 |

근거: `frontend/src/types/deck.ts:34`, `frontend/src/types/game.ts:142`, `server/src/deck.rs:25`, `server/src/main.rs:184`, `engine/src/types.rs:889`.

### 3.2 실행 중 보드와 기물

**[확인]** `Board`는 `size`, 지상 `squares: HashMap<SquareId,Option<PieceId>>`, 공중 `air_squares`, `terrain`을 가진다. 공중과 지상 기물이 같은 좌표에 있을 수 있다. `Square`는 정수 file/rank이며 JSON의 보드 키는 SquareId의 문자열 표현이다. (`engine/src/types.rs:83,132,150`)

**[확인]** `Piece`는 `id`, `owner`, `type_id`, `current_square: Option<Square>`, `in_pocket`, `captured`, `has_moved`, `current_ammo`, `layer`, `remaining_flight_turns`, 기물별 `state`, `move_option_cooldowns`를 가진다. `is_on_board()`는 square가 있고 Pocket/포획 상태가 아닌지 검사한다. (`engine/src/types.rs:845`)

**[확인] Pocket 전용 객체는 없다.** `player.deck.pocket_pieces` 목록과 `pieces[id].in_pocket=true`, `current_square=None`을 함께 유지한다. 같은 종류 여러 기물에도 서로 다른 ID가 있으며 기물별 상태를 보존할 수 있다. Drop은 Pocket 목록에서 ID를 제거하지만 `starting_pieces`에 넣지 않는다. 이 목록들은 현재 보드 전체를 나타내지 않는다. 시작 때 계산한 `total_score`도 보드 material의 실시간 점수가 아니다. (`server/src/main.rs:541,615`, `engine/src/endgame.rs:808`)

**[확인] GameState**는 id/board/pieces/piece_definitions/custom_piece_manifest/players/current_player/turn_number/phase/en_passant_target/en_passant_available_to/global_state/history/result 및 serialize하지 않는 Chessembly cache를 가진다. 현재 Hand/Extra에 해당하는 목록이나 zone 식별자는 없다. `GameView`는 이를 펼쳐 clock, presence, player_info, record_notation, challenge, revision을 덧붙인다. (`engine/src/types.rs:1067`, `server/src/time_control.rs:251`)

**[권장]** G3/G4는 기물 인스턴스를 별도 복제하기보다 현재 ID 참조 구조를 확장할 수 있다. 단, `in_pocket=false && square=None`만으로 Hand와 Extra를 구분하지 말고 소속을 명시하며 기존 ID 목록·플래그와의 불변조건을 정해야 한다. 최종 필드 위치/zone enum 여부는 G1 이후 설계 선택이며 G0에서 확정하지 않는다.

### 3.3 Drop과 canonical action

**[확인]** 클라이언트 `SubmitDropAction`/서버 `SubmitDropRequest`는 `piece_id`, `to` 선택값을 제출한다. 서버 요청 enum의 tag는 `type: drop`이고, 요청 구조는 알 수 없는 필드를 거부한다. 서버가 현재 플레이어와 합법 후보에서 canonical Drop을 만든다.

```text
SubmitDrop { piece_id, to }
 → server submit_action
 → generate_piece_legal_drop_actions(authoritative state, piece_id)
 → DropAction { player_id, piece_id, to, captured_piece_id? }
 → engine actions::submit_action (canonical 일치 재검사)
 → apply_and_advance_turn → apply_drop_action
 → history / server record / GameView
```

근거: `frontend/src/types/game.ts:192,233`, `server/src/main.rs:225,261,3055`, `engine/src/actions.rs:9`, `engine/src/legal_moves.rs:709`, `engine/src/endgame.rs:808`.

**[확인]** 엔진 `TurnAction`은 Move/Drop/Ability의 tagged enum이다. `ActionRecord`는 `turn_number`, `player_id`, `action`을 보관한다. canonical Move에는 capture/effects/option/layer/승격 결과가 있고, Ability에는 actor·ability ID, 단일/복수 대상, pocket_piece_id, 목적지, deployments가 있다. **기존 Ability의 sacrifice 또는 deployment는 새 ExtraSummon action이 아니다.** (`engine/src/types.rs:912,921,994,1020`)

## 4. 검증 경계와 기존 배치 규칙

### 4.1 앞줄/뒷줄 분류

**[확인]** 분류는 점수가 아닌 `PieceDefinition.deployment_zone: Front|Back`이다. 누락된 기존 정의 JSON의 기본값은 Back이다. 내장 Front는 pawn/tempest-pawn/bouncing-pawn/dozer/surface-to-air-missile의 양 진영 정의, fanatic, wall이며 그 외 내장 정의는 Back이다. 전체 분류를 검사하는 테스트가 있다. (`engine/src/types.rs:206,256`, `engine/tests/rule_engine.rs:1035,1151`)

**[확인]** `guhang`은 score 25/Back, `bomber`는 score 13/Back이다. 현재 프론트 카탈로그에서는 둘 다 `canPocket: true`이고 Extra-only 속성은 없다. (`engine/src/pieces/default_pieces/guhang.rs:9`, `engine/src/pieces/default_pieces/bomber.rs:36`, `frontend/src/composables/useDeckValidation.ts:44,68`)

프론트 카탈로그는 최초 score 0/Back으로 만들어졌다가 `/api/piece-catalog`의 엔진 정의 메타데이터로 갱신된다. `applyPieceMetadata`는 메타데이터 누락·잘못된 점수·분류를 거부한다. 커스텀 기물 분류는 저장된 package definition에서 읽는다. (`server/src/main.rs:1456,1462,1493`, `frontend/src/composables/useDeckValidation.ts:114,198`)

### 4.2 Legacy 초기 배치

**[확인]** `get_base_zone_squares`는 전 file에 걸친 아래 랭크를 반환한다. front는 적에 가장 가까운 한 줄, back은 나머지 기본 진영 줄이다.

| 크기 | White 기본 진영 / front rank | Black 기본 진영 / front rank |
| --- | --- | --- |
| 8 | 0,1 / 1 | 6,7 / 6 |
| 9 | 0,1 / 1 | 7,8 / 7 |
| 10 | 0,1,2 / 2 | 7,8,9 / 7 |
| 11 | 0,1,2 / 2 | 8,9,10 / 8 |
| 12 | 0,1,2 / 2 | 9,10,11 / 9 |

근거: `engine/src/rules.rs:230,249,258`, 프론트 대응 `useDeckValidation.ts:19,25,33,363`. 표의 rank는 0부터 시작한다. `DeckEditor.vue:552`는 같은 프론트 함수를 이용해 미리보기 줄을 만든다.

**[확인]** 엔진 `validate_deck`는 시작 King 정확히 1기, Pocket King 금지, front 모든 file 점유, 점수 상한, 각 기물 분류별 초기 배치를 검사한다. 점수는 starting+pocket의 non-King 정의 score 합이다. `build_player_deck`는 이보다 먼저 보드 범위, 기본 진영, 중복 점유, 기물 참조 해석을 검사한다. 엔진 validator의 ID 조회는 `filter_map`을 쓰므로 이것 하나를 외부 입력 전체 검증기라고 간주해서는 안 된다. 생성 경로의 앞단 검사가 계약의 일부다.

### 4.3 검증 진입점 비교

| 단계 | 실제 진입점 | [확인] 적용하는 검사 |
| --- | --- | --- |
| 편집 저장 | `validateDeckForStorage` → `validateDeckStructure` | 이름, 지원 크기, 정수/범위 좌표, 중복 칸, 기물 존재, 수량/크기 한도, 맵 정규화 검사 |
| 게임용 프론트 | `validateLobbyDeck`, `validateSavedDeck`, `validateDeckForGame` | 위 구조 + King/점수/front 채움/배치 분류/활성 커스텀 버전. `validateDeckForGame`은 지정 크기도 비교 |
| Deck Code | `decodeDeckCode` → `importDeckCode` | envelope/스키마 검사 후 현재 카탈로그와 게임용 덱 validation 적용 |
| 계정 저장 서버 | `DeckInput::spec`, `DeckInput::validate` | 엄격한 typed JSON, 이름, 8~12, 좌표/중복, 수량, 맵 일치, 커스텀 참조 소유·고정 버전/hash 일치 |
| 게임 생성 서버 | `resolve_custom_packages`, `build_player_deck` → `validate_deck` | 서버 정의로 기물 구체화, 활성 runtime package 해석, 보드/기본 진영/점유 및 게임 규칙 재검증 |
| 일반 Drop | `generate_piece_legal_drop_actions` → `actions::submit_action` | 현재 player, 소유·Pocket ID 소속·플래그·포획 여부, King 금지, 강제 착륙 대기, 합법 목적지/capture 일치 |

프론트 구조 한도는 starting 144, Pocket 종류 256, 종류별 최대 1024/총 4096이다. 계정 저장에는 customPieces 최대 256, 요청 128 KiB 한도도 있다. **저장 검증은 King 없음·점수 초과·배치 미완성·비활성 고정 버전이라는 이유만으로 초안을 거절하지 않는다.** (`useDeckValidation.ts:391`, `server/src/deck.rs:21,381,489`)

### 4.4 초기 배치와 게임 중 착수의 결합점

**[확인]** `placement.rs`의 Drop 후보 영역은 `get_base_zone_squares ∪ attack_map`이다. 일반 기물은 빈 지상 칸, `can_capture_on_drop` 기물은 적 점유 칸도 후보이며 canonical 생성에서 지형·포획 가능성을 추가 검사한다. `placement::validate_drop_action`도 있지만 실제 public submit 경로는 더 많은 조건을 검사하는 canonical generator를 사용한다. (`engine/src/placement.rs:13,58,81`, `engine/src/legal_moves.rs:709`)

**[확인]** `get_base_zone_squares`는 Drop뿐 아니라 박격포 관련 영역 제한, 폭격기 귀환/탄약, 귀환 탄약 보충 등에 쓰인다. (`engine/src/legal_moves.rs:810,1086`, `engine/src/endgame.rs:373,797,996`)

**[권장]** G2가 초기 배치 변경을 위해 이 공용 함수를 전역 변경하면 게임 중 이동·착수·특수 능력까지 바뀔 수 있다. 초기 배치 영역 함수와 기존 home/base 판정의 적용 범위를 먼저 나누고, 계획에 없는 Standard Drop 영역 변경은 추론하지 않는다. 현재 UI의 배치 predicate는 주로 rank만 받으므로 중앙 file 영역 도입 시 Square 또는 file 입력이 필요할 가능성이 높다.

## 5. 덱 저장 및 Deck Code

### 5.1 Local / Account

**[확인]** `useSavedDecks`는 인증 상태 `undefined`(미해결), `null`(게스트), 문자열(계정)에 따라 저장소를 선택한다. 인증 실패를 게스트 fallback으로 숨기지 않는다. 로컬→계정 이관은 별도 import 동작이며 원본을 남긴다. (`frontend/src/composables/useSavedDecks.ts:9,26,101`, `deckRepository.ts`)

| 경로 | 책임 |
| --- | --- |
| `frontend/src/composables/localDeckRepository.ts:8` | localStorage 키 `brainfuck_chess_saved_decks_v1`의 JSON 배열 읽기/쓰기. 구형 맵·customPieces 보완. 원시 레코드 검사는 얕으며 손상 JSON은 빈 목록으로 처리 |
| `frontend/src/composables/deckRepository.ts:11,25` | Local/Account 공통 repository 인터페이스와 CRUD 전달. 저장용 프론트 구조 검증은 에디터에서 수행하며 repository 자체의 공통 검증은 아님 |
| `frontend/src/api/deckApi.ts:14` | `deckInput`이 name 및 deckData의 허용 필드를 직접 열거. 새 필드는 명시적으로 추가하지 않으면 전송에서 사라짐 |
| `server/src/deck.rs:25,161,217` | strict DeckData deserialize, format_version 검사, owner별 DB 조회/CRUD, expectedVersion 동시성 검사, requestId 및 import fingerprint로 멱등성 |
| `server/src/routes.rs:19` | `/api/decks`, `/:id`, `/import`, body limit/no-store |
| `server/db/prod/20260908000000_account_decks.sql`, `server/db/test/20260908000000_account_decks.sql` | 환경별 decks 테이블. `deck_data JSONB`, `format_version=1` 제약, owner, version, request_key/create_hash |

**[확인]** 저장 덱의 DB format_version(1), account version(동시성 증가값), Deck Code v(1~3), GameRecord format_version(2)는 서로 다른 개념이다. `DeckInput::fingerprint`는 이름을 포함한 정규화 입력을 SHA-256으로 해시하며 starting/custom 참조를 정렬하고 0개 pocket 항목을 제외한다. (`server/src/deck.rs:185`)

**[확인]** `DeckEditor.vue:825`의 `save`는 starting과 양수 pocket에서 사용한 타입만 모아 customPieces를 다시 만든다. 서버 `resolve_custom_packages` 역시 starting+pocket 참조만 순회한다. **[예상]** G4에서 Extra에만 있는 커스텀 참조를 지원한다면 이 두 수집 경로 및 저장 validator의 미사용 참조 검사를 함께 확장해야 한다. 저장 UI 필드만 추가해서는 참조가 보존되지 않는다. (`server/src/main.rs:1046`, `server/src/deck.rs:477`)

**[예상]** ruleset/Extra를 typed DeckData에 추가하면 SQL JSONB에 담을 수 있지만, 이것만으로 migration 불필요가 확정되지는 않는다. format_version를 바꿀 경우 현재 DB CHECK와 `from_row`를 함께 검토해야 한다. format_version를 유지해 필드를 보완하는 전략에서도 구형 레코드 기본값 및 create/import fingerprint 안정성 테스트가 필요하다. 기존 배포 SQL을 재작성할지 여부는 G0에서 결정하지 않는다.

### 5.2 Deck Code

**[확인]** `frontend/src/composables/useDeckCodeCodec.ts`는 UTF-8 JSON을 base64url로 변환하며, 최신 출력은 `DC3.`이다. 압축은 하지 않는다.

| 입력 | payload | 읽기 동작 |
| --- | --- | --- |
| DC1 | `v:1, boardSize, starting[{pieceId,file,rank}], pocket[{pieceId,count}]` | 해당 크기의 일반전 mapId 보완 |
| DC2 | DC1 내용 + `v:2, mapId` | 등록 맵과 boardSize 일치 검사 |
| DC3 | DC2 내용 + `v:3, name, customPieces` | 이름·고정 커스텀 참조 포함 |

스키마는 버전별 **정확한 키 집합**을 검사한다. 구형 버전에 ruleset/Extra 키만 추가하면 기존 reader가 거부한다. 코드 최대 길이 65,536, starting 144, Pocket 종류 256/종류별 1024/총 4096 제한이 있다. encode는 starting/pocket/custom 참조를 정렬하며, decode 성공 이후에도 `importDeckCode`가 기물 존재와 덱 게임 규칙을 검사한다. 유효하지 않으면 현재 편집 덱을 교체하지 않는다. (`useDeckCodeCodec.ts:51,61,115,139`, `useDeckCode.ts:24`)

**[권장]** G1은 새 룰셋이 저장/복사 과정에서 조용히 사라지지 않도록 최소 직렬화 계약을 마련해야 한다. 최종 신규 코드 버전과 전체 기록 통합은 G7에서 정리하되, DC1/DC2/DC3의 기존 의미와 읽기 경로는 보존한다. 현재 버전에 새 키를 몰래 붙이는 전략은 사용할 수 없다.

## 6. 턴 전환, RNG, Pocket에 영향을 주는 기존 능력

### 6.1 실제 턴 순서

**[확인]** `engine/src/endgame.rs:206`의 순서는 다음과 같다.

1. 현재 turn 번호와 actor, 강제 착륙 여부, 기존 공중 기물/새 cooldown을 기억한다.
2. Move/Drop/Ability를 적용한다.
3. canonical action을 history에 추가한다.
4. 강제 착륙 action이 아니면 cooldown, 비행 잔여 턴, 착륙 불가능 폭격기를 처리한다.
5. 종료되지 않았고 강제 착륙 대기가 없어야 current_player를 교체하고 turn_number를 1 올린다.

서버는 검증용 복사본에 행동을 적용하고, 착수 확정 직전 시간 만료를 다시 확인한 후 `StoredGame.state`를 교체한다. 이후 clock/record를 갱신한다. Bot은 별도 endpoint에서 같은 엔진을 쓰고 timeline의 여러 action을 각각 기록한다. (`server/src/main.rs:3055,3229`, `server/src/time_control.rs`의 `finish_turn`)

**[권장]** G3 Draw는 실제 새 턴이 시작되는 경계에서 한 번만 실행되고, 거절/시간 초과 요청이나 강제 착륙 중간 action에서는 발생하지 않아야 한다. 생성 초기 Draw와 턴 시작 Draw를 구분하여 기록해야 한다. 단순히 서버 HTTP 응답마다 Draw하는 방식은 Bot/분석/Lab 경로와 맞지 않는다.

### 6.2 기존 난수 사용

**[확인]** 조사 범위 `engine/src`, `server/src`, `frontend/src`에서 찾은 관련 비결정성은 다음과 같다.

| 위치 | 실제 사용 | Standard Draw와의 관계 |
| --- | --- | --- |
| `engine/src/ai/search.rs:712` `easy_choice_index` | 현재 시각 subsec_nanos로 상위 최대 3개 후보 중 선택 | 시간 기반 선택이며 seed/균등 draw/재현 RNG 계약이 아님 |
| `server/src/main.rs:1093,1688` 등 | UUID v4로 방/게임 ID 생성 | 게임 기물 draw 서비스가 아님 |
| `frontend/src/singlePlayerSetup.ts:13` | `crypto.getRandomValues`로 로컬 진영 선택 | 클라이언트 진영 선택 |
| `frontend/src/views/MultiplayerLobby.vue:119` | `Math.random`으로 방장 진영 선택 | 클라이언트 진영 선택 |
| `frontend/src/composables/localDeckRepository.ts:46`, `api/gameApi.ts:262` 등 | UUID/시간/Math.random으로 로컬 식별자 생성 | 식별자 용도 |

엔진의 직접 의존성은 serde/serde_json, 서버에는 uuid(v4)가 있으나 직접 rand 의존성이 없다. **Draw용 seed/state/event 및 Pocket 무작위 추출 함수는 현재 모델과 전이 경로에서 확인되지 않았다.** (`engine/Cargo.toml`, `server/Cargo.toml`, `engine/src/types.rs`, `engine/src/endgame.rs`)

**[권장]** G3에서는 서버가 선택한 구체적 PieceId 결과와 검증 가능한 상태 전이를 연결한다. 검색 중 가상 전이가 실제 RNG를 소비하지 않도록 경계를 마련한다. 기존 Easy 봇의 시간 modulo나 프론트 난수 선택을 권위 Draw 구현으로 재사용하지 않는다. RNG 라이브러리/seed 저장 방식은 미선정이다.

### 6.3 Pocket은 Drop만으로 변하지 않는다

**[확인]** `legal_moves.rs:971,995` 및 `endgame.rs:490` 이후에는 교대병의 Board↔Pocket 교체, 공수부대의 복수 Pocket deployment, 그린캠프의 적 기물을 원소유자 Pocket으로 되돌리는 행동이 있다. 포획형 Drop은 공수부대 대원/포탄 같은 기물의 기존 동작이며 포탄은 착수 직후 폭발한다. 희생의 성소의 기존 sacrifice action은 별도 능력이다. (`engine/tests/rule_engine.rs:387,429,472,515`, `engine/tests/v2_supplement.rs:184`)

**[미확정]** 계획은 Standard의 일반 Drop 출처를 Hand로 지정하지만, 이런 기존 특수 능력이 Standard에서 Pocket을 직접 사용해도 되는지까지 명시하지 않는다. G3/G5에서 영향을 보고하되 임의로 모든 Pocket 참조를 Hand로 치환하지 않는다.

## 7. GameRecord / Replay / 분석 분기 / 동기화

### 7.1 GameRecord 생성·저장

**[확인]** `server/src/game_record.rs:217`의 GameRecord에는 format_version(2), ruleset_version(`deck-chess-1`), chessembly_version, game_id/display_name, 시작·종료 시각/result, players/time_control, initial_state/initial_clock, decks, actions, final_clock, game_mode/challenge_id, retention이 있다. 내부 ownership은 공개 serialize에서 제외된다.

`new_with_deck_names`는 history를 비운 초기 상태와 **그 시점의** DeckSnapshot을 동결한다. snapshot_version=1의 덱 snapshot은 side/deck_name/map_id/board_size/deployments/pocket(종류별 count 및 custom identity)을 가진다. 기본 생성자는 GameMode::Standard로 시작하고 Challenge 생성에서 별도로 모드를 바꾼다.

**[확인]** `RecordedAction`은 ply, player_id, canonical TurnAction, notation, state_delta, elapsed_ms/clock 전후값을 가진다. `push_action`은 서버의 before/after로 차분을 만든다. `replay_state_value`는 piece_definitions/custom_piece_manifest/history/id 및 board.size/terrain을 차분 대상에서 제외하고 나머지 JSON을 재귀 비교한다. 배열은 원소 patch가 아닌 값 전체 set으로 기록된다. (`server/src/game_record.rs:135,319,635`)

`persist_completed_record`와 repository는 종료 기록을 저장한다. PostgreSQL은 환경별 game_records의 `record JSONB` 및 record_version 등에 보관하며, in-memory 구현도 있다. 게스트 전용 기록은 repository 저장에서 제외하는 기존 테스트가 있다. (`server/src/main.rs:1353,4032,4080`, `server/src/game_record.rs:747,864`, `server/db/prod/20260826000500_create_game_records.sql:56`, 대응 `server/db/test/20260826000500_create_game_records.sql`)

**[예상/위험]** 초기 Draw를 실행한 뒤 현재 snapshot 함수를 그대로 호출하면 원래 Pocket 중 Hand로 옮긴 기물이 덱 snapshot에서 빠진다. G3는 initial_state 시점과 원본 덱 snapshot 시점을 구분할 수 있어야 하며, G7의 Replay→Deck Code까지 이를 보존해야 한다.

### 7.2 Replay와 Deck Code 재추출

**[확인]** `frontend/src/replayCodec.ts`는 GameRecord를 gzip + base64url로 만든 `DC-G2-` 코드로 내보내며, decoder는 현재 format_version=2만 받는다. 길이/압축 해제 크기/action 수/기물 수/차분 수, action 종류 및 차분 루트 allowlist를 검사한다. 허용 action 종류는 move/drop/ability다. `draw`라는 기존 GameEndReason 문자열은 무승부를 뜻하며 카드 Draw가 아니다.

`frontend/src/replayState.ts:80,99`와 서버 `GameRecord::state_at_ply`는 초기 상태에 차분을 차례대로 적용하여 복원한다. 프론트는 위험한 prototype 경로를 차단하고 이전 frame을 변형하지 않는다. 이 복원에는 난수 재추출이나 합법 수 재계산이 없다. `ReplayPage.vue`, `ReplayImport.vue`, `GameHistory.vue`, `replayNotation.ts`가 화면/입출력/표기를 담당한다.

`frontend/src/replayDeckCode.ts:39`의 `frozenDeckCodeSource`는 GameRecord의 덱 snapshot과 manifest에서 내장/커스텀 타입 및 Black 좌표를 중립화해 Deck Code 입력을 만든다. 최신 카탈로그에서 과거 덱을 새로 추정하는 방식이 아니다. 식별 정보가 부족한 snapshot은 null로 거부한다.

**[권장]** G7은 Standard의 초기 draw/턴 draw/소환·제물 결과가 서버에서 실제 확정된 상태와 일치하는지 검증한다. 기존 delta 메커니즘을 활용할 수 있지만, 새 최상위 state 필드가 생기면 `DELTA_ROOTS`, 새 action이 생기면 codec/notation/type 분기를 함께 확장해야 한다. 기존 DC-G2와 구형 optional snapshot 필드 호환을 따로 테스트한다. 현재 DC-G1 지원은 확인되지 않았으므로 기존 지원이라고 주장하지 않는다.

### 7.3 state hash가 있는 경로는 분석 분기다

**[확인]** `server/src/analysis.rs:14,50`의 AnalysisNode에는 action/state_after/state_hash가 있다. hash는 history를 비운 GameState JSON의 객체 키를 정렬한 SHA-256(`sha256-canonical:`)이다. 일반 `RecordedAction`에는 별도 state_hash 필드가 없다. AI의 PositionKey도 별개다.

**[확인]** `server/src/main.rs:2620`의 `analysis_state`는 부모 상태에서 저장 action을 `submit_engine_action`으로 다시 실행하고 expected/actual/stored hash를 비교한다. pending_actions도 재실행한다. `frontend/src/replayAnalysis.ts`, `types/gameRecord.ts`, `api/gameApi.ts`에 분기/요청 타입과 action identity 경로가 있다.

**[예상/위험]** 비결정적 Draw를 엔진 전이에 직접 넣기만 하면 본게임 차분 Replay는 보이더라도 분석 분기 재검증이 달라질 수 있다. 또한 기존 상태에 새 기본 필드가 serialize되면 과거 canonical hash가 달라질 수 있다. G1의 기본값/생략 규칙부터 검토하고, G7에서는 확정 Draw 결과 재사용과 구형 hash 호환을 검증해야 한다.

### 7.4 전체 응답과 heartbeat 응답은 다른 구조다

**[확인]** `GameView`는 GameState 전체를 flatten하지만 `GameDynamicView`/`GameStaticData`는 필드를 직접 열거한다. `StoredGame::sync_view`는 revision/catalog/history 차분을 만든다. 프론트 `mergeGameSync`는 dynamic과 catalog를 다시 합친다. (`server/src/time_control.rs:251,265,275,443`, `frontend/src/api/gameApi.ts:106,149`)

**[예상/위험]** GameState에 ruleset을 추가해도 sync view에 반영하지 않으면 최초 응답 이후 동기화에서 잃을 수 있다. 현재 응답은 양측 pieces/players/Pocket을 포함하고, Hand의 비공개 정보를 플레이어별로 감추는 모델은 없다. Hand/Extra 공개 여부가 결정되면 UI에서만 숨기는 것으로 끝낼 수 없으며 catalog/dynamic, record, Bot timeline까지 검토해야 한다. 공개 정책은 G0에서 정하지 않는다.

## 8. Bot / Challenge 경로

### 8.1 Bot

**[확인]** 일반 Bot 게임의 생성은 §2.2의 일반 `/api/games` 경로다. `GameScreen.vue`가 봇 차례에 `/api/games/:id/bot-turn`을 호출한다. 서버는 봇 진영·난이도·현재 턴·종료 여부를 검사하고 `play_bot_turn_detailed`를 실행한다. 확정된 result.state와 timeline을 저장/기록한다. (`server/src/main.rs:3229`, `engine/src/ai/search.rs:872`)

| 파일 | [확인] 현재 책임 | [예상] Standard 영향 |
| --- | --- | --- |
| `engine/src/ai/evaluate.rs:20,104` | Board/Pocket material, 이동/Drop mobility, King capture 위협. 정의의 ai_board_value/ai_pocket_value, 미지정 시 score 사용 | Hand/Extra/남은 Pocket을 구분하는 평가, Draw 기대값 |
| `engine/src/ai/search.rs:32,61,111` | Move/Drop/Ability 생성, Pocket 동등 기물 대표 ID로 축약, canonical 전이, 검색, Easy 시간 기반 선택 | Hand 후보 생성, ExtraSummon 및 확률적 전이 처리 |
| `engine/src/ai/beam.rs` | PocketPieceKey, 동등 행동 축약, tactical/quiet Drop 및 Board quota | zone/기물 상태 차이를 지우지 않는 축약 |
| `engine/src/ai/move_ordering.rs` | 이동/Drop/Ability 순서, 전술 우선순위 | 소환 및 제물 비용의 순서 평가 |
| `engine/src/ai/transposition_table.rs:9` | Board/공중/지형/플레이어/기물 상태/Pocket 소속 등을 포함하는 구조적 PositionKey | ruleset/Hand/Extra/추가 전이 상태의 key 구분 |
| `engine/src/ai/types.rs` | AiAction, 결정/탐색 제한/결과/timeline 타입 | 새 action 전달 및 exhaustive match |
| `frontend/src/views/BotDebugger.vue`, `frontend/src/botDebugMetrics.ts` | 봇 설정·진단 표시 | 새 행동 분류를 표시할 경우 대응 |

**[권장]** G8 전까지 기존 Bot을 Standard 지원으로 간주하지 않는다. 다만 공용 타입 추가에 필요한 컴파일 대응과 Legacy key 회귀는 해당 Goal에서 수행하고, Standard 평가/탐색 정책은 G8로 남긴다. 실제 검색은 `apply_canonical_action`을 직접 사용하므로 public submit만 고쳐서는 모델 불일치를 막을 수 없다.

### 8.2 Challenge

**[확인]** `ChallengeDefinition`은 서버 registry의 고정 board_size, opponent_starting, opponent_pocket, bot_difficulty, time_control, enabled를 가진다. 별도 ruleset/map 필드는 없고, Summary에서 일반전 map_id를 생성한다. 현재 3개는 tempest_horde(12), raining_men(12), tempest_set(10)이다. raining_men은 구행을 처음부터 Board에 둔다. (`server/src/challenge.rs:33,88`, `server/src/main.rs:1497`)

```text
Challenges.vue: validateSavedDeck + challenge.map_id 일치
 → POST /api/challenges/:id/games { player_deck, local_nickname? }
 → 서버 find(id)로 크기/상대 덱/난이도 결정
 → opponent_deck(definition, black): 중립 좌표 미러링, pocket 수량 전개
 → build_game_state_with_variant(size, Plain, player, official, ..., true, false)
 → StoredGame.set_challenge → GameMode::Challenge / metadata
```

근거: `frontend/src/views/Challenges.vue:101,119`, `server/src/main.rs:75,1541`, `server/src/challenge.rs:247`, `server/src/time_control.rs:389`.

**[확인] 공식 덱만 별도 검증 계약을 쓴다.** `validate_registry`는 ID/크기/기물/좌표/중복/King/점수/일부 배치 분류를 검사하지만 일반 덱 front 전체 점유는 요구하지 않는다. `allow_nonstandard_zone`은 registry의 분류 검사를 생략하는 공식 배치 플래그다. 게임 생성의 `build_player_deck`는 validation=false여도 기본 진영/보드 범위/점유 검사는 유지하므로 이 플래그가 임의 보드 위치까지 허용하는 것은 아니다. tempest_set의 rank 1 폰은 10보드의 기본 진영 안이지만 정상 Front rank 2는 아닌 예다.

**[확인]** 클라이언트가 상대 덱/크기를 덮어쓰는 Challenge 생성 필드는 `deny_unknown_fields`로 거부된다. Bot endpoint는 Challenge의 봇 진영/난이도를 고정하며, 서버가 확인한 플레이어 승리에 대해서만 등록 계정 clear를 저장한다. (`server/src/main.rs:75,3229,1353`, 테스트 `:4317,4376`)

**[권장]** G1은 기존 Challenge의 일반전 맵/기존 동작을 Legacy로 보존한다. G4의 구행 Extra-only를 전역 정의로 강제하여 raining_men을 깨뜨리지 않는다. Standard Challenge의 공식 덱/진영/예외는 G8에서 따로 설계한다.

## 9. G1~G8 예상 변경 파일과 Worktree 충돌

### 9.1 Goal별 예상 파일

**이 절 전체는 [예상/권장]이다.** 아래는 현재 책임을 근거로 한 변경 후보이며 구현 확정 목록이 아니다. 타입 추가로 필요한 기존 fixture 생성자 갱신은 기능 확장과 구분한다. 아직 존재하지 않는 모듈을 기존 파일인 것처럼 기재하지 않는다. 표에서 같은 그룹의 후속 파일은 앞에서 명시한 소스 디렉터리를 기준으로 줄여 적었다(예: `server/src/deck.rs`, `main.rs`는 모두 `server/src/` 아래).

| Goal | 변경 가능성이 높은 파일 | 변경 이유 / Goal 경계 |
| --- | --- | --- |
| G1 Ruleset 분리 | `engine/src/types.rs`, `engine/src/rules.rs`; `frontend/src/types/deck.ts`, `types/game.ts`, `views/DeckEditor.vue`, `composables/useDeckValidation.ts`, `composables/localDeckRepository.ts`, `composables/useDeckSerialization.ts`, `composables/useDeckCodeCodec.ts`, `composables/useDeckCode.ts`, `api/deckApi.ts`, `api/gameApi.ts`; `server/src/deck.rs`, `main.rs`, `time_control.rs` | ruleset 타입/기본값/validation 문맥, 저장·요청·실행·동기화 전달, 에디터 명시 필드 복사. 보드/맵 의미 보존. 새 배치·Hand·Extra 구현 금지 |
| G1 전달 보조 | `frontend/src/App.vue`, `views/DeckSelect.vue`, `views/MultiplayerLobby.vue`, `views/DeckLibrary.vue`, `views/Challenges.vue`; `server/src/game_record.rs`, `analysis.rs`, `challenge.rs`; `engine/src/ai/transposition_table.rs` | 포맷 매칭, 기존 생성 경로 Legacy 처리, 기록/해시에 새 기본 필드가 미치는 영향. record 전면 통합·Standard Bot 정책은 후속 Goal |
| G2 초기 배치 | `engine/src/rules.rs`, `server/src/main.rs`; `frontend/src/composables/useDeckValidation.ts`, `views/DeckEditor.vue`; `engine/tests/rule_engine.rs`, `server/src/main.rs` tests, `frontend/src/composables/customDeckIntegration.test.ts`, `views/DeckEditor.test.ts` | 중앙 file/rank 영역 계산, 서버 사전 기본 진영 검사와 최종 validator 연결, UI 좌표 predicate/미리보기/프리셋 점검. 공용 home 판정 보존 |
| G3 Hand/Draw | `engine/src/types.rs`, `actions.rs`, `legal_moves.rs`, `placement.rs`, `endgame.rs`; `server/src/main.rs`, `time_control.rs`; `frontend/src/types/game.ts`, `api/gameApi.ts`; `server/src/game_record.rs` | Hand 소속, 권위 Draw, 생성/진짜 턴 전환, 일반 Drop 출처. initial_state와 덱 snapshot의 시점 정리. 전체 게임 UI/기록 포맷 통합은 G6/G7 |
| G4 Extra 모델/빌더 | `engine/src/types.rs`, `rules.rs`; `frontend/src/types/deck.ts`, `types/game.ts`, `views/DeckEditor.vue`, `composables/useDeckValidation.ts`, `composables/localDeckRepository.ts`, `composables/useDeckSerialization.ts`, `composables/useDeckCodeCodec.ts`, `composables/useDeckCode.ts`, `api/deckApi.ts`, `api/gameApi.ts`; `server/src/deck.rs`, `main.rs`; `engine/src/pieces/default_pieces/guhang.rs`, `bomber.rs` (메타데이터를 기물 정의에 둘 경우만) | 3**기** 제한, Main 점수와 Extra 소환 점수 분리, ruleset별 Extra-only, 저장/로드/전송. 메타데이터 위치는 별도 설계 선택. 제물·소환 전이는 구현하지 않음 |
| G5 ExtraSummon | `engine/src/types.rs`, `actions.rs`, `legal_moves.rs`, `endgame.rs`; `server/src/main.rs`; `frontend/src/types/game.ts`, `api/gameApi.ts`; 필요 시 `engine/src/ai/types.rs` 등 enum 사용처 | 요청 선택값/canonical 결과, zone별 제물/중복/소유/점수/target 검증과 원자 적용. exhaustive match 컴파일 대응이 Standard 봇 전략 구현을 뜻하지 않음 |
| G6 게임 UI | `frontend/src/components/GameScreen.vue`, `components/Board.vue`, `moveOptionUi.ts`, `composables/useActionTimeline.ts`, `gameControlPolicy.ts`, `types/game.ts`, `api/gameApi.ts`; 필요 시 `server/src/time_control.rs`, `main.rs` | Hand/Pocket/Extra 표시, 제물 선택·취소·가치 표시, 합법 후보·서버 오류. 공개 정책에 따른 서버 응답 투영 필요 여부는 미확정 |
| G7 저장/API/복기 | `server/src/deck.rs`, `main.rs`, `game_record.rs`, `analysis.rs`, `time_control.rs`; `frontend/src/types/gameRecord.ts`, `types/game.ts`, `api/gameApi.ts`, `api/deckApi.ts`, `composables/useDeckCodeCodec.ts`, `composables/useDeckCode.ts`, `replayCodec.ts`, `replayState.ts`, `replayNotation.ts`, `replayDeckCode.ts`, `replayAnalysis.ts`, `views/ReplayPage.vue`, `views/ReplayImport.vue`, `views/GameHistory.vue` | 최종 버전·구형 읽기, draw/summon/sacrifice 차분과 notation, frozen 덱, 분기 재실행/hash. DB 버전 정책에 따라 `server/db/prod/`, `server/db/test/`의 새 migration 및 검증 SQL이 필요할 수 있음 |
| G8 Bot/Challenge | `engine/src/ai/evaluate.rs`, `search.rs`, `beam.rs`, `move_ordering.rs`, `transposition_table.rs`, `types.rs`; `server/src/challenge.rs`, `main.rs`, `time_control.rs`; `frontend/src/views/Challenges.vue`, `DeckSelect.vue`, `BotDebugger.vue`, `components/GameScreen.vue`, `App.vue`, `api/gameApi.ts`, `botDebugMetrics.ts` | zone 가치·기대값·소환 비용·새 후보/key, 공식 Standard 덱과 서버 포맷 고정, 진단/선택 대응 |

공통 테스트 영향: `engine/tests/{rule_engine,ai,ammo_air_layer,v2_supplement,custom_piece_runtime}.rs`, 엔진 모듈 내부 tests, `server/src/main.rs`/`game_record.rs`/`analysis.rs`/`deck/tests.rs` 및 프론트의 대응 `.test.ts`. `GameState`/`Deck`/`Piece`를 struct literal로 만드는 fixture는 serde default가 있어도 Rust 컴파일상 새 필드 지정이 필요할 수 있다. 이 기계적 갱신을 별도 Worktree가 중복 수행하면 충돌 범위가 커진다.

### 9.2 G2/G3/G4 충돌 파일

**[예상] 현재 구조에서 세 Goal을 아무 조정 없이 완전히 독립 병렬 작업으로 취급하기 어렵다.** Worktree는 작업 디렉터리만 분리하며 아래 공통 계약과 동일 파일 충돌은 제거하지 않는다.

| 공통 파일 | G2 | G3 | G4 | 충돌 예상 및 조정 |
| --- | --- | --- | --- | --- |
| `engine/src/types.rs` | 배치 문맥/정의 확장 가능 | GameState/Deck/Piece Hand | Deck/Extra·기물 속성 | G3↔G4 높음. 필드 위치·소속 불변조건을 먼저 맞추고 한 담당자가 공용 타입 반영 |
| `engine/src/rules.rs` | 배치 영역·validator | zone 점수 처리와 연결 가능 | Extra 제한·score 제외·Extra-only | G2↔G4 매우 높음. 동일 `validate_deck`와 점수/배치 계약을 건드림 |
| `server/src/main.rs` | `build_player_deck` 사전/최종 배치 검사 | `build_game_state_with_variant`, submit/턴 처리 | `PlayerDeckSpec`, `build_player_deck`, package 수집 | 세 Goal 모두 높음. struct·생성자·긴 inline test 모듈까지 공유 |
| `frontend/src/composables/useDeckValidation.ts` | 좌표 predicate/front 검증/프리셋 | Hand는 런타임에만 두면 접점 적음 | Extra-only·수량·점수·카탈로그 | G2↔G4 매우 높음. 같은 validation 단계 의미 충돌 |
| `frontend/src/views/DeckEditor.vue` | 배치 미리보기와 클릭 제한 | G3에 UI를 넣지 않으면 낮음 | Extra 영역·복사·점수·입출력 | G2↔G4 높음. script/template 같은 상태와 computed 사용 |
| `frontend/src/types/game.ts` | 배치 타입 추가 시 | Hand/GameState | Extra/Deck | G3↔G4 높음. 타입 편집 소유권 조정 |
| `frontend/src/api/gameApi.ts` | 별도 배치 API가 없으면 낮음 | sync/상태 타입 | PlayerDeckRequest/전송 | G3↔G4 중간. 파일 내 구간은 달라도 최종 모델 합의 필요 |
| `server/src/time_control.rs`, `game_record.rs` | 직접 변경 적음 | 생성 draw/sync/snapshot 시점 | Extra가 runtime에 있으면 sync/snapshot 영향 | G3↔G4 중간~높음. 전이/기록 시점은 의미상 충돌 |
| `engine/tests/rule_engine.rs`, `server/src/main.rs` tests | 배치 회귀 | Hand/Draw fixture | Extra validation fixture | 높음. 공용 helper/struct 초기화가 중복됨 |
| `frontend/src/composables/customDeckIntegration.test.ts`, `views/DeckEditor.test.ts` | 배치/UI 검증 | G3에서는 변경 적음 | Extra 카탈로그/UI 검증 | G2↔G4 높음 |

**[권장 작업 순서]** G1 완료 커밋을 공통 base로 삼고, 공용 타입/validator 인터페이스/생성자 변경의 담당 범위를 먼저 적는다. 현재 결합도를 기준으로는 G2와 G4를 순차 통합하는 편이 안전하다. G3의 독립 Draw 계산/전이 테스트 부분은 병렬 진행할 수 있으나 `types.rs`/`main.rs` 공용 통합은 순서를 정한다. G4에서 Extra-only 배치 금지를 완성하되 G2의 배치 영역 계산을 덮어쓰지 않는다. G2·G3·G4의 공통 생성 경로가 합쳐진 상태를 검증한 후 다음 단계로 진행한다. 이번 G0에서는 작업 분할을 분석했을 뿐 실제 Worktree나 하위 에이전트를 만들지 않았다.

### 9.3 G1을 시작할 때의 구체적 체크포인트

**[권장]** G1에서 다음 연결을 먼저 완성하면 G2~G4가 사용할 문맥을 잃지 않는다.

1. 보드 크기, map/terrain, ruleset, GameMode, record version을 별도 개념으로 유지한다. 현재 맵 ID의 `standard-` 및 GameMode::Standard는 변경/재해석하지 않는다.
2. ruleset의 안정 ID 및 누락된 기존 데이터→Legacy 규칙을 정하고, 알 수 없는 명시값을 정상 Legacy로 위장하지 않는다. serde default와 serialize 생략이 기존 분석 해시에 주는 영향을 확인한다.
3. `SavedDeck → cloneSavedDeck → local/account 저장 → deckInput/DeckData` 전 구간에서 왕복시킨다. G1은 새 Standard 게임 규칙을 적용하지 않는다.
4. `serializeNeutralDeck/savedDeckToPlayerDeckRequest → CreateGame/Room/PlayerDeckSpec → game factory → GameState → full view/sync merge` 전달과 두 덱/룸의 호환 검사를 연결한다. Black 미러링이 경로마다 한 번만 일어나도록 보존한다.
5. validation의 ruleset 입력 경계를 추가하되 Legacy 배치·점수·Pocket 행동을 유지한다. Standard 선택 표시만으로 G2~G5가 완료된 것처럼 취급하지 않는다.
6. 구형 저장/코드/요청, 같은 크기 맵 전환, 크기와 ruleset 상호 독립, Legacy 생성·Drop·강제 착륙·Challenge를 회귀 확인한다. G7 최종 통합 전에도 ruleset이 저장·요청에서 유실되지 않아야 한다.

## 10. 기존 테스트, 부족한 영역 및 실제 검증

### 10.1 기존 테스트 위치와 책임

**[확인]** 다음 테스트들이 현재 경계를 검증한다.

| 위치 | 기존 검증 내용 |
| --- | --- |
| `engine/tests/rule_engine.rs` | 크기/고지/점수, 8·9·10 기본 진영, front 채움, King/Pocket, 전체 내장 배치 분류·양 진영, 구형 정의 기본값, canonical Drop/포획/고지/앙파상, Pocket 교체·복수 deployment |
| `engine/src/actions.rs` 내부 tests | public canonical 검증과 내부 apply의 Move/Drop/Ability 결과 일치 |
| `engine/tests/ammo_air_layer.rs` | 공중·비행 지속 시간·강제 착륙·탄약, 상대 턴 비행시간 불변 |
| `engine/tests/v2_supplement.rs` | 포탄 Drop 폭발, 기존 sacrifice·광신도·성벽·수리병 등 |
| `engine/tests/ai.rs`, `engine/src/ai/*` 내부 tests | Bot 행동/턴 종료, Pocket material/대표 축약, 상태 key·탐색 회귀 |
| `engine/tests/chessembly_compat.rs`, `chessembly_piece_expression.rs`, `custom_piece_runtime.rs` | 기존 문법·표현식·커스텀 runtime 호환 회귀 위치 |
| `server/src/main.rs` tests | 맵 불일치/구형 요청, 양측 배치 검사, catalog, server canonical action, 강제 착륙의 같은 플레이어 history, record·분기·heartbeat·Bot·Challenge |
| `server/src/deck/tests.rs` | 계정 CRUD/소유/멱등성/동시성/한도, 미완성 덱 저장과 게임 거절, DB 통합(일부 ignored) |
| `server/src/challenge.rs` tests | 공식 registry 3개, tempest_set 미러링, 중복/알 수 없는 기물 거절 |
| `server/src/game_record.rs`, `analysis.rs`, `time_control.rs` tests | 보존 기간·상태 복원·hash 변조/정렬·분기·동일 플레이어 후속 action의 clock 처리 |
| `frontend/src/composables/customDeckIntegration.test.ts` | 고정 커스텀 참조 전송, 활성 버전, 8~12 진영/프리셋·front 채움·점수와 독립인 분류 |
| `frontend/src/composables/useDeckCode.test.ts`, `deckRepository.test.ts` | DC1/2/3·정확한 스키마·잘못된 import, guest/account 저장·이관·인증 변화·draft |
| `frontend/src/views/DeckEditor.test.ts` | 동일 크기 맵 전환 내용 보존, 크기 변경 classic reset, Pocket 조작, 저장 성공/실패/편집 보존 |
| `frontend/src/replay{Codec,State,Notation,DeckCode,Analysis}.test.ts`, `api/gameApi.test.ts` | 기록 코드·차분/동일 플레이어 순서·proxy/위험 경로·frozen 덱·분기·동기화 |

### 10.2 이후 추가해야 할 테스트

**[예상: 현재 Standard 모델/행동이 없으므로 관련 검증도 아직 없음]**

- G1: ruleset 누락 데이터/구형 코드가 Legacy로 왕복, 명시 unknown 거절, 보드·맵·ruleset 독립, 계정 저장 allowlist·클론·heartbeat 후 보존, 두 덱/룸 포맷 불일치, 과거 분석 hash.
- G2: 8/9/10/12의 중앙 영역 정확한 Square 집합과 Black 대칭, file/rank 경계, 중복·front 충족 기준, Legacy home/drop/탄약/능력 영역 불변. 현재 테스트는 중앙 집중형 영역의 기대값을 제공하지 않는다.
- G3: 초기 양측 3기 및 정상 턴 1기, Pocket 부족/빈 경우, 같은 종류의 서로 다른 ID, Hand 소속/플래그 불일치 거절, Pocket 직접 Drop 거절, 실패/timeout 무변경, 강제 착륙·재요청에서 추가 Draw 없음, 초기 덱 snapshot 누락 방지.
- G4: 3종류가 아닌 3기 기준, 4기 거절, Extra 점수 보존/Main 상한 제외, Legacy Extra 금지, 구행/폭격기 Standard 초기 배치 불가, local/account/code 왕복.
- G5: 제물 ID 중복/다른 소유자/허용되지 않은 zone/점수/목적지/없는 Extra 거절, 실패 전후 전체 상태 일치, 포획·King 종료·공중 레이어와 원자성.
- G6/G7: 공개 정책 확정 후 서버 응답 노출 검사, Hand/Extra 선택·취소, 서버가 정한 Draw 결과의 Replay/분기/hash/Deck Code 동일성, 구형 GameRecord 읽기. 전체 브라우저를 통한 새 포맷 E2E는 현재 조사·실행 범위에 없음.
- G8: 동일 Board지만 Hand/Pocket/Extra가 다른 상태의 평가·key 구분, 난수 결과를 소비하지 않는 탐색, 소환의 제물 기회비용, 공식 Challenge 포맷 고정과 Legacy raining_men 보존.

### 10.3 이번 G0에서 실행한 검증

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | **성공**, Node runner 기준 21개 테스트 파일 통과, 실패 0 |
| `cargo test --offline -p brainfuck-chess-engine --test rule_engine` | **성공**, 75개 통과 |
| `cargo test --offline -p brainfuck-chess-server` | **성공**, 99개 통과, 8개 ignored, 실패 0 |
| `git diff --check`, `git diff HEAD --name-only`, `git status --short --untracked-files=all` | tracked 변경 없음. 기존 계획 문서와 새 분석 문서만 untracked |
| `git diff --no-index --check /dev/null docs/new-format/standard-format-codebase-analysis.md` | 새 문서 whitespace 오류 출력 없음. no-index의 새 파일 차이 종료 코드 1은 예상된 결과 |
| 문서 참조 경로/행 범위 점검 스크립트 | 명시적 소스 경로 65개 존재 및 인용 행 범위 확인 |

테스트 명령은 workspace Cargo manifest, `frontend/package.json`, 기존 `docs/account-deck-persistence.md`의 검증 명령을 확인하고 사용했다. ignored 8개는 DB 연결이 필요한 7개와 payload 진단 1개다. 이들 및 전체 엔진 suite/벤치마크, Chessembly suite, 별도 build/typecheck/lint, 브라우저 E2E는 이번 G0에서 실행하지 않았다. 문서만 추가하는 조사이며 필요한 기존 동작 표본을 검증했다. 프론트의 `lint` script는 현재 `vue-tsc --noEmit`으로 typecheck와 동일하다. 테스트 결과를 운영 DB나 Standard 기능의 정상 동작으로 확대 해석하지 않는다.

## 11. 위험·미확정 사항과 최소 변경 방향

| 구분 | 위험 또는 미확정점 | 권장 대응 / 다음 Goal |
| --- | --- | --- |
| 확인된 명칭 충돌 | standard 맵 ID / GameMode::Standard / 새 Standard 룰셋 | 별도 ruleset ID. G1에서 기존 의미 보존 |
| 확인된 입력 경계 | PlayerDeckSpec에 덱 고유 포맷이 없고 현재 비교는 주로 UI | G1에서 요청·룸·서버 validation의 포맷 계약 결정 |
| 확인된 저장 경계 | strict DeckData와 정확한 Deck Code 키, UI clone/whitelist | 새 필드 누락/거부 방지, Legacy reader 보존 |
| 확인된 함수 재사용 | 초기 배치와 Drop/home/능력이 같은 base 함수 사용 | G2를 초기 배치 범위로 제한, 나머지 동작 회귀 |
| 확인된 상태 중복 | ID 소속 목록 + in_pocket + square + captured | G3/G4에서 zone 불변조건 명시, 한 전이에서 일관되게 변경 |
| 확인된 다중 action 턴 | 강제 착륙이 턴 전환을 보류 | Draw를 진짜 턴 전환에 결합, clock/history/Replay 보존 |
| 확인된 기록 시점 | snapshot이 initial_state의 Pocket 목록 사용 | 초기 draw 이후 원본 덱 손실 방지 |
| 확인된 재실행/해시 | 분석 분기는 action을 다시 실행; 직렬화 기본 필드가 hash에 포함 | G1의 호환 기본값부터 점검, G3/G7에서 확정 난수 재현 |
| 미확정 게임 규칙 | 첫 턴 추가 Draw, Hand 상한, 빈 Pocket UI, 상대 Hand/Extra 공개 | 계획의 미확정 상태 유지. 해당 Goal에서 필요한 최소 결정을 요청 |
| 미확정 소환 규칙 | 기본 소환 칸, 재사용, 동일 기물 제한, 비용 예외/Pocket 제물 | G4/G5에서 임의 결정하지 않음 |
| 미확정 배치 세부 | 중앙 2/3칸과 감싸는 칸의 정확한 2차원 집합·채움 의무 | 기존 코드는 그 기대값을 정의하지 않음. G2 테스트 작성 전에 계획 해석을 확정 |
| 미확정 능력 상호작용 | 기존 Pocket 교체/deployment/복귀 능력의 Standard 적용 | 일반 Hand Drop 규칙과 구분해 G3/G5에서 보고 |
| 미확정 봇 전략 | 기대값·비공개 정보·소환 평가 | G8까지 Legacy 봇 정상 동작만 보존 |

**[권장]** 기존 Rust 엔진의 `types → rules/legal_moves → actions/endgame` 경계, 프론트의 편집 모델/직렬화 분리, 서버의 덱 재구성과 canonical action 검증, 기록의 상태 차분 구조를 확장하는 방향이 가장 국소적이다. G0만으로 새 프레임워크·registry·저장 계층 또는 Chessembly 문법 변경의 필요성은 확인되지 않았다. 관련 enum/모델 확장에 필요한 경계만 추가하고, 미확정 게임 규칙을 기존 함수 이름이나 문서의 옛 설명으로 메우지 않는다.

## 12. G0 완료 조건 대조

| 계획의 조사 대상 | 분석 위치 |
| --- | --- |
| 1 Deck 모델, 2 Board size 위치, 3 보드 선택 UI, 4 점수 상한 | §2~3 |
| 5 front/back 분류, 6 초기 배치, 7 덱 validation 진입점 | §4 |
| 8 저장/로딩, 9 Deck Code | §5 |
| 10 생성 요청, 11 서버 재검증, 12 GameState 생성 | §2.2~2.4, §4.3 |
| 13 Pocket, 14 Drop | §3.2~3.3, §4.4 |
| 15 턴 시작/전환, 16 RNG | §6 |
| 17 canonical action/history, 18 GameRecord, 19 Replay | §3.3, §6.1, §7 |
| 20 Bot Board/Pocket 평가, 21 Challenge 생성 | §8 |
| 파일별 책임·데이터 흐름·예상 변경 및 G1~G8 | §2~9 |
| G2/G3/G4 병렬 충돌·테스트·부족 영역·호환 위험·권장 방향 | §9.2~11 |

- [x] 코드 수정 없이 분석 문서만 작성했다.
- [x] 기존에 untracked였던 계획 문서를 그대로 보존했다. 이번 작업의 새 산출물은 이 분석 문서 하나다.
- [x] G1을 시작할 수 있도록 선택→저장→요청→서버 재검증→실행 상태→동기화까지 심벌과 데이터 형태를 추적했다.
- [x] 현재 코드에서 확인한 사실, 이후 변경 예상/권장, 미확정 게임 규칙을 분리했다.
- [x] 조사 대상 21개와 산출물 요구사항을 모두 문서에 연결했다. G1~G8 구현은 수행하지 않았다.
