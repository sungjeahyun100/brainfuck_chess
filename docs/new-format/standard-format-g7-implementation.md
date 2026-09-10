# G7 — Standard Deck Code / Persistence Compatibility / Record Versioning

작성일: 2026-09-10 (KST). 첨부된 G7 목표, AGENTS.md 및 G0~G6 문서를 기준으로 구현했다. 시작 시 존재한 미커밋 변경을 보존하고 별도 시작 사본과 비교하여 G7 변경만 검토했다. G1~G6 게임 규칙, Chessembly, Bot 전략, Challenge, DB migration은 변경하지 않았다.

## 1. Deck Code 버전과 실제 DC4 schema

새 prefix는 `DC4.`다. 기존처럼 **UTF-8 JSON → padding 없는 base64url**이며 압축하지 않는다. 실제 encoder의 키 순서와 예시는 다음과 같다. 이 예시는 Front가 미완성인 저장 초안이다.

```json
{
  "v": 4,
  "name": "Standard 초안",
  "mapId": "standard-8x8",
  "boardSize": 8,
  "starting": [{ "pieceId": "king", "file": 4, "rank": 0 }],
  "pocket": [{ "pieceId": "knight", "count": 2 }],
  "customPieces": [],
  "ruleset": "standard",
  "extra": ["bomber", "guhang", "guhang"]
}
```

위 9개 키는 모두 필수다. `ruleset`은 `legacy` 또는 `standard`만 허용한다. 누락으로 Standard를 추론하지 않는다. Starting은 `{pieceId,file,rank}`, Pocket은 `{pieceId,count}`, customPieces는 아래 고정 참조 객체의 배열이다. 알 수 없는 키, 잘못된 타입, 누락 필드, DC4 payload의 `v != 4`는 거부한다. Hand는 runtime 소속이므로 넣지 않는다.

내부 `DecodedDeckCode.v = 1`은 기존 codec의 정규화된 기하 표현이다. wire version을 재해석하는 값이 아니다. 새 입력은 명시적 `ruleset`과 `extra`를 함께 반환하며 import 경계에서 SavedDeck을 구성한다.

## 2. DC1/DC2/DC3 compatibility와 export 정책

기존 reader의 exact-key 스키마·필드 처리 로직은 `parseLegacyPayload`로 옮겨 그대로 재사용한다. DC4는 이 함수의 DC3 필드 계약을 재사용하고 새 필드와 추가 구조 검증을 적용한다. DC1~3에 ruleset/extra를 덧붙여 허용하지 않는다. 기존 버전의 입력 의미·유효한 출력은 바꾸지 않았다.

| 입력/출력 | 최종 동작 |
| --- | --- |
| DC1 import | 해당 크기 일반전 map 보완, Legacy, 빈 Extra |
| DC2 import | 기존 map/board, Legacy, 빈 Extra |
| DC3 import | 기존 name/custom refs/map/board, Legacy, 빈 Extra |
| DC4 legacy import | 명시적 Legacy 및 Extra 보존 |
| DC4 standard import | 명시적 Standard 및 Extra 보존 |
| Legacy + 빈 Extra export | **기존 DC3 문자열 그대로** |
| Standard export | DC4, Extra가 비어도 ruleset 명시 |
| Legacy + non-empty Extra 초안 export | DC4, Extra를 버리지 않음 |

Legacy DC3 encoder의 필드 순서·정렬·직렬화 코드는 유지했다. 독립된 pre-G7 wire JSON golden과 정확한 문자열 일치를 검사한다. 기존 DC1~3 editor import의 게임용 validation은 하위호환성을 위해 유지한다. DC4만 저장 초안의 구조 검증을 사용한다.

## 3. Extra encoding과 canonicalization

Extra는 SavedDeck과 같은 **기물 참조 문자열 배열**이다. 현재 정상 게임 덱은 최대 3기이므로 count 객체보다 간단하고 짧다. `guhang` 두 항목은 두 인스턴스로 복원하며 Set으로 축약하지 않는다. 좌표나 영구 PieceId는 없다.

encoder는 Extra 복사본을 `localeCompare`로 정렬한다. Starting은 rank/file/pieceId, Pocket은 pieceId, customPieces는 id/version/exposedPieceKey의 기존 DC3 정렬을 사용한다. 배열 순서만 다른 같은 편성은 동일 DC4 코드가 된다. source 배열은 변경하지 않는다. decoder는 입력 배열의 인스턴스들을 그대로 복원한다.

## 4. 구조 한도와 게임 한도

- 코드 길이: 공백 제거 전 최대 65,536자. DC4 export도 최종 코드 길이를 검사한다.
- Starting: 최대 144기. DC4에서는 정수·보드 범위·중복 좌표를 검사한다.
- Pocket: 최대 256종, 각 1~1,024, 총 4,096. 같은 종류의 중복 count entry는 거부한다.
- customPieces: 최대 256개. ID/key 길이는 1~256, 양의 safe integer version 및 기존 hash 문자 계약을 사용한다.
- map: 등록 맵이며 boardSize와 일치해야 한다. 기존 지원 크기 8~12만 허용한다.
- Extra: 기존 G4 저장 초안과 같은 **구조 상한 4,096기**, ID 1~256자 및 제어문자 금지. count 객체를 받지 않으므로 수량 전개로 메모리를 무한 할당하지 않는다.
- 게임: 기존 G4 validator의 **총 3기** 및 ruleset별 eligibility를 그대로 적용한다.

따라서 Extra 4기의 초안과 Legacy+Extra 초안도 DC4로 저장·공유·import할 수 있지만 게임 시작은 거부한다. Front 미완성·점수 초과 등 기존 draft 의미도 유지한다. 4,096 경계 성공 및 4,097 거부를 테스트했다.

## 5. Custom reference/package 계약

```json
{ "id": "airship", "version": 7, "contentHash": "sha256_frozen", "exposedPieceKey": "captain" }
```

`custom:airship:v7:captain`은 위 package의 고정 참조다. SavedDeck과 기존 DC3는 package 실행 소스/정의를 내장하지 않고 이 참조를 저장한다. DC4도 같은 계약이다. code만으로 받는 사람이 다른 계정의 package를 설치하거나 권한을 얻지 않는다.

- codec은 전달받은 고정 참조를 보존하며 Starting/Pocket/**Extra** 전체의 custom ID가 해당 참조에 연결되는지 검사한다.
- editor의 기존 저장용 reference collector를 copy에서도 재사용한다. 아직 저장하지 않은 Extra-only custom 참조도 수집한다. DC4 copy에서는 이미 고정된 참조의 hash를 우선 보존하며 편집 덱을 변경하지 않는다.
- import는 현재 카탈로그에서 해당 고정 버전을 찾고 contentHash까지 일치하는지 검사한다. DC4의 누락/불일치 참조를 현재 버전으로 자동 대체하지 않는다.
- 참조는 구조 decode만으로 보존 가능하다. 실제 editor import에는 해당 카탈로그가 필요하며 없는 package는 오류로 반환한다. inactive version 및 custom Extra 게임 eligibility는 기존 저장/게임 검증 경계에 따른다.
- 완료 Replay의 원래 덱 snapshot에서도 Extra를 참조 수집에 포함한다. explicit snapshot custom identity 또는 기존 manifest를 사용하며 현재 catalog로 과거 package를 추정하지 않는다.

## 6. Import 원자성과 UI

`importDeckCode`는 새 candidate만 만든다. decode·카탈로그/고정 참조 검사·해당 버전의 validation이 모두 성공한 후 미리보기를 제공한다. `applyImportedDeck`이 clone한 SavedDeck 전체를 한 번에 교체한다. 실패 시 ruleset/map/size/Starting/Pocket/Extra/customPieces 및 전역 custom catalog가 그대로다. 새 prepare가 실패하면 이전 성공 candidate도 제거하여 오래된 candidate를 잘못 적용할 수 없게 했다.

Standard 복사 차단 상수/버튼 조건을 제거했다. DC4 사용 덱은 구조적으로 저장 가능한 경우 복사할 수 있고 일반 Legacy는 기존 게임 유효성 조건을 유지한다. import 입력 안내는 DC1~DC4를 표시하며 미리보기에서 룰·맵·보드·Extra·Starting·Pocket·점수를 확인한다. 적용 후 룰 select도 code의 값을 표시한다. game-valid와 구조 import 성공을 구분하는 안내를 추가했다.

## 7. Local/account persistence와 fingerprint

G1/G4의 저장 구현을 재설계하지 않았다. local SavedDeck → JSON → 새로운 LocalDeckRepository, accountDeckApi/DeckInput allowlist → typed DeckData → SavedDeck fetch 경로를 다시 검사했다. 8~12 일반전 및 12 고지전에서 name/ruleset/mapId/boardSize/Starting/Pocket/Extra/custom refs를 비교한다. Extra 중복·구형 ruleset/Extra 누락 기본값도 보존한다.

SQL adapter는 기존 `sqlx::types::Json(DeckData)`를 JSONB에 쓰고 typed DeckData로 읽는다. DB 형식은 이미 이 필드를 표현한다. 실제 외부 PostgreSQL 왕복은 실행하지 않았으며 계정 검증은 실제 서버 handler/메모리 repository와 프론트 fetch adapter 테스트다.

fingerprint 구현도 변경하지 않았다. 기존 Starting/custom 정렬 및 0 Pocket 제외에 더해 G4의 Extra 복사본 정렬을 유지한다. 동일 내용의 Legacy/Standard는 다르고, 구행 1기/2기는 다르며, Extra 순서만 바뀌면 같다. 기존 Legacy 고정 fingerprint `0df9dceb7f145306a9dea2f4da33d5311a7e0e2144168dd3107082065845b635` 회귀가 통과한다. 원본 저장 배열의 순서는 fingerprint 때문에 변경하지 않는다.

## 8. GameRecord semantic version과 중앙 dispatch

서버 `game_record.rs`:

- `LEGACY_RULES_VERSION = "deck-chess-1"`
- `STANDARD_RULES_VERSION = "deck-chess-standard-1"`
- `current_rules_version_for(DeckRuleset)` — 신규 기록 생성 시 선택.
- `supported_record_rules_version` → `GameRulesVersion::{LegacyV1,StandardV1}`.
- `GameRecord::validate_rules_version` — root initial_state.ruleset과 ruleset_version 쌍 검증.

프론트는 `gameRulesVersions.ts` 한 곳의 대응 상수/검증을 replay codec과 frame builder가 공유한다. endpoint나 화면마다 version 문자열을 복제하지 않는다. 서로 다른 Rust/TypeScript runtime에 대응 계약이 존재하며 future version 구현 시 양쪽 지원 목록을 함께 갱신해야 한다.

| Record 조합 | 처리 |
| --- | --- |
| Legacy(구형 누락 포함) + deck-chess-1 | supported, 기존 delta Replay 및 Analysis |
| Standard + deck-chess-standard-1 | supported, 현재 exact Replay/Analysis |
| Standard + deck-chess-1 | `unsupported_development_standard_record` |
| Legacy + deck-chess-standard-1 | `unsupported_rules_version` |
| 알 수 없는/future semantic version | `unsupported_rules_version` |

기존 `deck-chess-1`을 `legacy`/`standard`로 치환하지 않는다. `GameMode::Standard`도 별도 기존 일반전 구분이며 unchanged다. 개발 단계 Standard 규칙은 실제로 달랐으므로 자동 migration/현재 버전 덮어쓰기 없이 명시적으로 거부한다. 미래 Standard v2는 새 dispatch와 실제 대응 엔진이 준비되기 전까지 현재 엔진으로 실행하지 않는다.

## 9. Replay / Analysis enforcement

서버 record 공개/owner 경계, `state_at_ply`, `analysis_state`, `validate_analysis_trees`에서 중앙 version 검증을 먼저 호출한다. unsupported는 HTTP 422와 위 명시적 error로 반환한다. initial Draw 검증, canonical action 적용, hash 비교, preview/create/append, 빈 tree 검증보다 앞선다. 기존 접근/소유권 검사는 유지한다.

supported Standard는 기존 G3-C/G5의 initial Draw 검증 → Move/Drop/Ability/ExtraSummon → 저장된 exact DrawResolution → delta/canonical hash 일치를 사용한다. Replay에서 RNG를 재실행하지 않는다. Analysis는 기존 preview draw_pending, 잠금/멱등성 검사 뒤 Draw 확정, node 저장·reload/hash 검증을 유지한다. 각 node에 semantic version을 중복 저장하지 않고 **root record**를 source of truth로 쓴다.

`DC-G2-` Replay Code는 ruleset_version 및 sparse snapshot Extra를 그대로 보존한다. decode에서 unsupported 조합을 명시적으로 거부하고 자동 upgrade하지 않는다. 기존 `encodeReplayCode`는 기록을 손실 없이 담는 역할을 유지하므로 unsupported 기록을 담아도 import에서는 거부된다. 직접 frame builder를 호출해도 version 오류가 delta 적용 전에 발생한다. ReplayPage는 오류를 표시하고 unsupported root의 analysis list 요청도 시작하지 않는다.

브라우저 local Replay의 검증은 버전·구조·안전한 delta 적용이다. Rust 엔진의 canonical exact 검증을 브라우저에서 수행한다고 주장하지 않는다. 서버에서 가져오는 supported Standard 기록은 기존 서버 exact 검증을 거친다.

## 10. 구조/DB 버전과 migration

| 별도 개념 | G7 값/결정 |
| --- | --- |
| Deck Code | 기존 DC1~3 유지, DC4 추가 |
| account SavedDeck DB format_version | **1 유지**: typed JSONB가 ruleset/Extra 표현 가능 |
| account row version | 기존 optimistic concurrency 증가값 유지 |
| GameRecord format_version / DB record_version | **2 유지**: 기존 Draw/ExtraSummon/sparse 구조 표현 가능 |
| DeckSnapshot snapshot_version | **1 유지**: 원래 Starting/Pocket/Extra snapshot 표현 가능 |
| GameRecord ruleset_version | Legacy 유지, Standard 첫 stable semantic version 추가 |
| DeckRuleset | 기존 legacy/standard 그대로 |

G7 신규 migration/열은 없다. semantic version은 기존 record JSONB 내부 필드다. **G3-C의 `game_analysis_nodes.draws` forward migration은 새 서버 배포 전에 적용해야 한다.** 두 환경의 기존 migration 파일을 수정하거나 실행하지 않았다.

## 11. 변경 파일과 책임

- `frontend/src/composables/useDeckCodeCodec.ts`: DC4 exact schema/encoding/범위/참조 검사, 기존 reader 재사용.
- `frontend/src/composables/useDeckCode.ts`: DC4 source ruleset/Extra, 구조 draft import, hash 검사.
- `frontend/src/views/DeckEditor.vue`: 복사 허용/참조 수집/원자적 미리보기/룰 반영·안내.
- `frontend/src/gameRulesVersions.ts` (신규): 프론트 중앙 semantic version 검증.
- `frontend/src/replayCodec.ts`, `replayState.ts`: shared record 버전 검증, snapshot Extra schema.
- `frontend/src/replayDeckCode.ts`, `types/gameRecord.ts`: 원래 snapshot Extra/custom refs 및 Standard DC4 출력.
- `frontend/src/views/ReplayImport.vue`, `ReplayPage.vue`: unsupported 오류 표시 및 초기 analysis 요청 차단.
- `server/src/game_record.rs`: 중앙 semantic version 선택/dispatch와 Replay 입구.
- `server/src/main.rs`: 공개/Analysis 오류 변환과 version gate; 신규 게임 버전 회귀 기대값.
- `server/src/deck/tests.rs`, `draw/g3c_tests.rs`: 저장 전체 왕복/fingerprint 및 실행 이전 버전 거부·HTTP 경계 증거.
- 프론트 기존 테스트 `composables/useDeckCode.test.ts`, `deckRepository.test.ts`, `views/DeckEditor.test.ts`, `replayCodec.test.ts`, `replayState.test.ts`, `replayDeckCode.test.ts`, `replayAnalysis.test.ts`: codec/실제 Vue handler/저장/기록 회귀 및 stable fixture.
- 이 문서와 `STANDARD_FORMAT_IMPLEMENTATION_PLAN.md`: 형식 동결 계약과 완료 증거.

여러 파일이 필요한 이유는 codec/편집기, 서버 record/Analysis 입구, 프론트 local Replay, 저장 adapter 테스트가 각각 다른 경계이기 때문이다. 엔진·저장 시스템·의존성·DB migration 변경은 없다.

## 12. 자동 검증 결과

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | runner 기준 22파일 통과, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 vue-tsc script |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 222 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 143 passed, 9 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server g7` | 신규 G7 2개 통과 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| `git diff --check` / 시작 사본과 G7 diff 검토 | 통과 |

기존 frontend 500 kB 번들 경고 및 서버 미사용 생성자 2개 경고는 남아 있다. engine ignored 7개는 벤치마크, server ignored 9개는 기존 외부 DB/진단 테스트다. 테스트를 삭제하거나 약화하지 않았다. G1/G4의 Standard export 미지원 단언과 G1~G6 개발 record fixture만 새 G7 지원 계약으로 갱신했다.

## 13. 실제 브라우저 검증

외부 DB 없이 로컬 서버 18087 / Vite 15187에서 **실제 App → DeckLibrary → DeckEditor → DeckSelect → 서버 게임 생성 → GameScreen**을 사용했다. 게임/카탈로그 API를 mock하거나 runtime state를 주입하지 않았다. 서버 proxy를 위한 임시 Vite 설정만 사용했다.

1. Legacy 새 덱에서 Standard 선택, 최소 프리셋 적용.
2. 구행 2기 + 폭격기 1기, Knight Pocket 2기 구성. Main 12/39, Starting 7 확인.
3. 저장 → 코드 복사 → 실제 Ctrl+V로 DC4 확인.
4. 새 Legacy editor에서 붙여넣기 → 미리보기의 Standard/map/8×8/Extra3/Starting7/Pocket2 확인 → 적용.
5. 복원 덱을 다시 복사·붙여넣기하여 **원본 코드와 byte-for-byte 일치** 확인.
6. 별도 이름으로 저장 → 페이지 reload → 원본/복원 두 Standard 덱 선택 → 실제 게임 생성.
7. Standard 새 Base 양측 배치, Extra 각 3기(구행 중복), 초기 Draw로 양측 Hand Knight2/Pocket0 확인. 짧은 Pocket은 기존 min(count) 규칙이다.
8. 새 Legacy 덱 복사 결과 **DC3** 확인. Standard를 선택한 editor에 import한 뒤 Legacy select/빈 Extra/39점 기존 배치 복원 확인.
9. Legacy 복원 화면 screenshot으로 룰 선택·성공 안내·기존 배치 UI 확인.

브라우저 clipboard read helper는 빈 값을 반환했지만 실제 native paste는 정상 동작하여 textarea DOM의 실제 코드를 기준으로 검증했다. 일부 read-only locator 평가 timeout은 최신 DOM snapshot으로 상태를 확인했다. 임시 브라우저 탭/서버/설정은 검증 후 정리했다.

미실행: 실제 외부 PostgreSQL JSONB/동시성 통합, 브라우저 custom package 실기·9~12 각 크기 실기·Replay/Analysis 실기·모바일 전체 검증. 이 부분의 serialization/HTTP/엔진/기존 Vue 회귀는 자동 suite로 검사했으며 실기 검증과 구분한다.

## 14. 요구 테스트·완료 조건 대조

| 목표 테스트 번호/완료 조건 | 증거 |
| --- | --- |
| 1~7 DC1~3 / 기존 Legacy DC3 | 기존 fixture import, exact 추가 키 거부, 독립 golden bytes, DC3/4 prefix-version 거부 테스트 |
| 8~21 DC4 8~12/맵/name/Starting/Pocket/Extra/custom | `G7 DC4 preserves…`, Extra-only custom test, frozen snapshot test, 실제 UI byte 왕복 |
| 22~30 malformed/bounds/draft | `G7 DC4 strict bounds…`, `G7 DC4 draft import…`: 누락/unknown/map/count/Extra4096/4097/참조/코드길이 |
| 31~34 import 원자성/룰 | 실제 DeckEditor setup 테스트의 candidate 실패·전체 상태/catalog 불변·DC3/DC4 적용, 브라우저 양방향 룰 변경 |
| 35~40 local/account persistence | 프론트 새 repository reload/fetch와 서버 전체 맵 handler+serialization 테스트, 구형 default 회귀 |
| 41~44 fingerprint | 기존 고정 Legacy hash·ruleset 차이, 신규 수량/순서 검사 |
| 45~50 신규 버전 및 exact Replay/Analysis | 신규 factory 두 ruleset 버전, `g7_semantic_versions…`, 전체 G3-C/G5 exact action/Draw/node/hash/reload suite |
| 51~57 unsupported | 중앙 version gate 테스트 및 실제 preview/create/append 오류/0 RNG/빈 저장소, frontend Replay Code와 delta 실행 전 오류 |
| 58~70 G1~G6 / Legacy | 전체 engine/server/frontend suite: geometry, Hand/Draw/privacy, ExtraSummon, 완료 Replay/Analysis, HUD, Pocket/Ability, Challenge, forced landing/ammo/air, pre-G1 hash |

Standard Deck Code export/import, ruleset/Extra/custom reference 보존, Legacy bytes/의미, 코드가 룰의 source of truth라는 계약, 저장/fingerprint, stable semantic version, unknown/development 조기 거부, supported exact Replay/Analysis 및 전체 로컬 test/build/check를 충족했다. **G7 구현 완료**이며 운영 DB 통합/배포 완료를 뜻하지 않는다.

## 15. G8에 넘기는 계약과 남은 배포 위험

G8은 현재 Standard를 `deck-chess-standard-1`으로 취급하고 Hand/Pocket/Extra 및 canonical ExtraSummon/DrawResolution을 재사용한다. Bot 전략이나 Challenge 규칙을 이 버전 아래 조용히 변경하면 안 된다. 게임 규칙의 호환성을 깨는 변경에는 새 semantic version 및 명시적 지원 dispatch가 필요하다. unknown version의 best-effort Replay는 금지한다.

배포 전 G3-C Draw migration을 적용한 disposable PostgreSQL에서 기존 opt-in integration tests를 실행하고 서버/프론트 버전 지원을 함께 배포해야 한다. 구형 클라이언트는 DC4를 읽지 못한다. 개발 단계 Standard 기록은 의도적으로 미지원이며 이름만 바꿔 upgrade하지 않는다. 커스텀 package 공유 권한/설치 시스템은 새로 만들지 않았으므로 수신자에게 해당 고정 package가 있어야 editor import할 수 있다.

G8의 Standard Bot 전략/불완전 정보/소환 subset 선택/Standard Challenge는 이번 구현에 포함하지 않았다.
