# G8-B — Challenge Format / Standard Challenge Infrastructure / Final Regression

작성일: 2026-09-10 (KST). 첨부 Goal, AGENTS.md 및 G0~G8-A 결과 문서를 기준으로 구현했다. 기존 미커밋 G1~G8-A 변경은 보존했으며 `/tmp/g8b-baseline` 사본과 비교해 이번 변경을 검토했다. 새 공개 Challenge는 없다.

## 1. 최종 ChallengeDefinition

```rust
ChallengeDefinition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    ruleset: DeckRuleset,
    map_id: &'static str,
    board_size: i32,
    opponent_starting: Vec<OfficialPlacement>,
    opponent_pocket: Vec<OfficialPocket>,
    opponent_extra: Vec<&'static str>,
    bot_difficulty: BotDifficulty,
    time_control: TimeControlId,
    enabled: bool,
}
```

기존 Rust registry 스타일을 유지했다. Starting은 White 중립 좌표와 기물 참조, 항목별 `allow_nonstandard_zone`을 가진다. Pocket은 기물 참조/count, Extra는 한 참조당 한 인스턴스다. Hand는 정의에 없고 실제 초기 Draw로만 생성한다. Extra 동일 타입 중복을 보존한다.

## 2. Ruleset / map source of truth

Challenge ID로 `find`한 서버 definition이 양측 룰셋, 크기, 지형, 공식 덱, 난이도, 시계를 결정한다. `map_id`는 `board_map_definition`의 canonical ID이며 맵 존재와 map.board_size 일치를 registry에서 검사한다. map.variant로 공용 게임 factory를 호출하고 같은 ID를 원래 덱 snapshot에 저장한다. 크기에서 일반전 맵을 만들어 새 Standard Challenge의 포맷을 추론하지 않는다.

`DeckRuleset`, `BotDifficulty`, `TimeControlId`는 닫힌 enum이어서 registry Rust 값으로 지원하지 않는 variant를 만들 수 없다. HTTP의 unknown ruleset 및 overrides는 역직렬화/검증에서 거부한다. 시간 제어는 기존 enum→definition 매핑을 그대로 사용한다.

## 3. 공식 opponent validation과 배치 예외

`validate_registry`는 서버 시작과 게임 생성 helper 입구에서 실행한다. ID 고유성/비어 있지 않음, 지원 보드, map/board 일치, 기물 해석, 보드 bounds, 중복 칸, 자기 Base 내부, King 정확히 하나, Pocket King 금지, Main 점수 상한을 검사한다. 기물 ID 해석을 게임 factory의 `resolve_piece_type`과 일치시켰다.

Pocket 수량은 항목당 1~1024, 합계 4096 이하를 **전개 전에** 확인한다. `validate_spec_deck_zones`를 재사용하여 Legacy Extra 금지, Standard Extra 최대 3기/eligibility, 구행·폭격기 Starting/Pocket 금지를 공식 덱에도 적용한다. Extra는 Main 점수에 합산하지 않는다.

기존 예외의 실제 의미는 **Base 안에서 Front/Back 분류를 벗어나는 공식 배치**다. 보드 어디에나 두는 허가가 아니다. Legacy는 기존처럼 Front 완전 점유를 요구하지 않는다. Standard는 예외가 없는 공식 덱이면 Front를 채워야 하며, 예외가 명시된 공식 편성은 다른 배치/불완전 Front를 허용한다. 이 경우에도 King/점수/Extra eligibility/Base/bounds/중복 검사를 생략하지 않는다.

생성 시 기물 소유권과 ID는 서버 factory가 발급하며, Starting/Pocket/Extra를 서로 다른 목록·좌표·플래그로 구체화한다. Hand는 빈 상태로 출발한다. 별도 fake materializer나 사용자 덱 validator 우회는 없다.

## 4. 플레이어 SavedDeck 전달과 서버 검증

프론트는 기존 SavedDeck에서 `serializeNeutralDeck(deck, 'white')` 결과에 플레이어의 `map_id`, `board_size`를 함께 전달한다. Challenge 전용 요청 wrapper가 기존 PlayerDeckSpec을 flatten하여 받는다. 일반전/룸의 PlayerDeckSpec 형식은 바꾸지 않았다.

```json
{
  "player_deck": {
    "ruleset": "standard",
    "map_id": "standard-9x9",
    "board_size": 9,
    "starting": [],
    "pocket": [],
    "extra": []
  }
}
```

배열은 구조 예시이며 빈 Starting은 게임 불가다. 서버에서 player.ruleset/map/size가 definition과 일치해야 한다. Standard에서는 map/size 누락도 거부한다. 기존 Legacy 요청은 map/size가 없었으므로 누락 시 해당 Legacy definition의 기존 포맷으로 해석한다. 명시적으로 다른 map/size는 Legacy에서도 거부한다. 구형 ruleset 누락은 기존 serde Legacy 기본값이다. 저장 덱 자체를 마이그레이션하거나 삭제하지 않는다.

이전과 같이 제출된 덱 내용을 재검증하며 클라이언트 로컬 저장 이력을 인증하는 것은 아니다. 전체 Front/King/Main score/배치/Extra/커스텀 참조 권한 검증은 공용 사용자 덱 factory (`validate_white_as_user_deck=true`)가 수행한다. 요청의 최상위 `deny_unknown_fields`를 유지하며 플레이어 wrapper도 unknown 필드를 거부한다. 클라이언트가 definition의 map, 상대 덱, Extra, 난이도, winner/result를 설정하는 필드는 없다.

## 5. 기존 공개 콘텐츠 보존

| ID | map / size | 공식 Starting | Pocket | 난이도 / 시계 |
| --- | --- | --- | --- | --- |
| tempest_horde | standard-12x12 / 12 | King (5,0), Tempest Pawn 전체 file rank 2 | Tempest Pawn 47 | Normal / Unlimited |
| raining_men | standard-12x12 / 12 | King (5,0), **구행 (6,0)** | Paratrooper 31 | Normal / Unlimited |
| tempest_set | standard-10x10 / 10 | 기존 Tempest 후열 8기, rank 1 Tempest Pawn 10기 | 빈 Pocket | Hard / Unlimited |

모두 명시적 Legacy, Extra empty다. 좌표는 중립 White 기준이며 Black으로 rank만 반전한다. ID/이름/설명/활성 상태/플레이 내용/clear 저장 키를 변경하지 않았다. golden 테스트는 전체 Starting 좌표·기물·예외 플래그와 Pocket, 이름/map/size/difficulty/time을 독립 expected 값으로 고정한다. `raining_men`의 구행은 Extra로 옮기지 않았다.

## 6. Standard Challenge 생성과 Draw

실제 endpoint는 ID를 찾은 뒤 `create_challenge_game_from_definition`을 호출한다. 테스트 fixture도 이 production helper를 사용한다.

1. registry 및 player format 검증, 기존 custom package 권한 확인.
2. 공식 Black 덱과 사용자 White 덱을 공용 `build_game_state_with_variant`로 구체화.
3. `StoredGame::new_with_players_and_deck_names`에서 원래 Starting/Pocket/Extra snapshot 동결.
4. 기존 `initialize_draws`: White initial 3 → Black initial 3 → White first-turn 1.
5. 기존 Local Bot capability를 인간 White로 고정.
6. `set_challenge`, 수신자별 `view_for`, store 등록.

인간 White/Bot Black 계약을 유지한다. 충분한 Pocket이면 Hand 4/3이며 실제 턴 전환의 +1은 기존 submit/Bot commit 경로다. 짧은 Pocket은 min(count), 강제 착륙 중간/종료/실패 시 Draw 없음, RNG 실패 원자성도 기존 G3 계약이다. Legacy의 initialize는 Draw/RNG 없는 no-op이다.

## 7. G8-A Bot와 ExtraSummon

Challenge는 기존 `run_bot_turn`을 사용한다. definition의 난이도와 Bot 진영을 고정하며 요청된 다른 난이도로 바뀌지 않는다. Standard에서는 기존 Local Bot capability 검사를 거친다. 탐색은 G8-A `BotObservation`을 통해 상대 Hand identity와 Pocket을 제거한 상태를 사용한다. 새 Bot heuristic, RNG, 검색 엔진을 추가하지 않았다.

테스트 전용 9×9 공식 배치에서 실제 Bot ExtraSummon을 선택하고 commit했다. 공용 bounded sacrifice selector(Extra당 최대 4개, frontier 최대 128), canonical 제물 목록/target, Extra 제거, 후속 Draw, 기록 및 exact 재생을 그대로 사용한다. 별도 유효 Front fixture에서는 실제 Hand Drop도 확인했다. 인간 hidden Hand/Pocket 종류를 바꿔도 root 후보 집합과 같은 제한의 decision/score가 같다. G8-A의 모든 난이도·TT·대량 subset·production Draw RNG 미호출 테스트도 전체 suite에서 다시 통과했다.

## 8. Privacy와 HUD

생성 응답부터 `view_for(now, audience)`를 호출한다. 인간은 자기 Hand/Pocket identity, Bot Hand count만 보며 Bot Pocket identity는 없다. 양쪽 Extra는 공개다. Bot 응답/timeline의 stats는 기존 Standard 정책대로 생략한다. 게임 종료 live view도 이 경계를 유지하고 완료 record 조회에서 양쪽 Hand/Draw를 공개한다.

G6의 GameScreen Standard HUD와 Legacy 화면 분기를 그대로 사용한다. 별도 Challenge Hand/Extra 패널은 없다. 기존 Challenge banner와 metadata를 유지한다. 목록에는 기존에도 상대 덱 미리보기가 없었으므로 새 opponent deck preview를 추가하지 않았다.

## 9. Record / Replay / Analysis

`GameMode::Challenge`를 유지한다. ruleset은 GameState, semantic version은 G7 중앙 dispatch다: Legacy `deck-chess-1`, Standard `deck-chess-standard-1`. 새 mode/version은 없다.

기존 원래 덱 snapshot, initial_draws, canonical ExtraSummon, action별 DrawResolution, state_delta와 challenge_id를 보존한다. Standard fixture의 실제 Drop→Bot ExtraSummon→기권 완료 기록에서 저장된 action/Draw로 복원한 hash를 마지막 action의 authoritative state와 비교한다. Analysis도 같은 root version/`state_at_ply`를 사용한다. 기권·timeout은 기존 별도 종료 metadata이므로 마지막 action frame 자체는 playing일 수 있다. 이를 임의로 delta/action으로 만들지 않는다. 새 replay 엔진이나 RNG 재실행 경로는 없다.

G3-C/G5 전체 회귀가 analysis preview/멱등 create/append/Draw 저장·reload와 exact 검증을 담당한다. G8-B fixture에서도 완료 record의 Analysis root/최종 action 위치 재구성과 양쪽 Hand 공개를 확인했다.

## 10. Clear 저장과 위조 방지

기존 `persist_completed_record`와 `challenge_clears(user_id, challenge_id)`를 그대로 재사용한다. 등록 사용자, 서버에 고정된 challenge context, 실제 record winner가 인간 side일 때만 저장한다. 첫 clear timestamp와 idempotent 저장을 유지한다. ID에 ruleset 접두사 등을 붙이지 않는다.

추가로 발견한 경로를 수정했다: Legacy의 SharedLocal 접근 정책만 검사하면 사용자가 Black Bot을 직접 기권시킬 수 있었다. 공용 `require_control`에서 Challenge 인간 side 이외의 직접 조작을 거부한다. 따라서 Legacy/Standard 모두 상대 기권 위조로 clear를 만들 수 없다. Bot은 기존 별도 검증·commit 경로로 실행한다. 정상 인간 기권/봇 응수/기존 clear 의미는 유지한다.

양 ruleset에서 상대 기권 거부, 인간 기권 패배/clear 없음, 서버 adjudication으로 판정한 인간 승리/clear 1회, 중복 persist와 동일 challenge_id를 검증했다. 등록 사용자 연결은 테스트에서 기존 resolver의 context 결과를 주입했고 실제 외부 로그인/DB 검증과 구분한다.

## 11. Challenges UI

카드에 Ruleset badge, canonical map label, board size 및 기존 난이도/clear/시작 버튼을 표시한다. 덱 선택 화면은 다른 ruleset/map/size 및 game-invalid draft를 숨기지 않고 사용 불가와 이유를 보여준다. 시작 버튼과 실제 handler가 현재 유효성을 다시 확인한다. Standard 선택 handler도 동일 코드이며 새 public Challenge를 UI에 노출하지 않았다.

## 12. Test-only fixture

`server/src/challenge/g8b_tests.rs`는 `#[cfg(test)]` 모듈이다. `fixture()`는 production `definitions()`/`find()`에 등록되지 않는다. 메모리 AppState와 실제 creation helper, submit, Bot, resign, 완료 record/analysis/clear 함수를 통과한다. 별도 fake builder나 runtime game state 주입으로 소환을 만드는 테스트가 아니다. clear 검증의 등록 사용자/서버 종료 판정 주입은 §10에 별도 명시했다.

9×9 fixture는 충분한 Pocket, 중복 Extra 포함 3기, 공식 배치 예외를 가진다. 추가 fixture는 정상 Front와 12×12 중앙 고지 맵의 production materialization도 확인한다. `/api/challenges`는 여전히 3개만 반환하며 fixture ID의 production `find`는 None이다.

## 13. 변경 파일

- `server/src/challenge.rs`: definition/summary 필드, registry validation, ruleset-aware 공식 materialization.
- `server/src/main.rs`: player format wrapper, authoritative creation helper, initial Draw/access/view, summary, Challenge 상대 직접 조작 차단.
- `server/src/challenge/g8b_tests.rs` 신규: golden/validation/생성/Bot/privacy/완료 replay/analysis/clear 회귀 7개.
- `frontend/src/api/gameApi.ts`: summary ruleset 및 제출 플레이어 map/size 타입.
- `frontend/src/views/Challenges.vue`: badge/map/compatibility/reason/start guard.
- `frontend/src/views/Challenges.test.ts` 신규: 실제 Vue setup/handler 및 wire 전달 검증.
- `frontend/package.json`: 기존 runner에 새 테스트 등록.
- 계획 문서와 이 문서: 동결 계약/검증/완료 대조.

파일이 여러 개인 이유는 서버 definition/HTTP, 프론트 API/UI, 각 경계의 테스트가 서로 다른 책임이기 때문이다. 기존 엔진·Bot·기물·SQL·저장 시스템을 재작성하지 않았다. 새 의존성은 없다.

## 14. 전체 자동 검증

| 명령 | 최종 결과 |
| --- | --- |
| `npm test --prefix frontend` | 23 test 파일 통과, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 vue-tsc script |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 228 passed, 8 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 154 passed, 9 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| `git diff --check` 및 G8-B 시작 사본 비교 | 통과 |
| `cargo test --offline -p brainfuck-chess-engine --test ai g8a::standard_search_benchmark -- --ignored --nocapture` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine --test ai_benchmark ai_search_difficulty_budgets -- --ignored --nocapture` | 통과, Legacy 3난이도 |

Standard benchmark: root actions 415, subsets 11, generation 32ms. depth 1/2/3 완주, 24/74/444 nodes, 216/776/3152ms. 기존 명시 benchmark 예산이며 기본 Hard 응답 시간 보장은 아니다. Legacy benchmark 전체 8.45초. 기존 frontend 500kB 번들 및 서버 미사용 생성자 2개 경고가 남는다. ignored DB/진단 및 나머지 성능 테스트는 실행하지 않았다.

신규 테스트 작성 중 타입 참조와 HashMap 유래 후보 순서 비교 오류를 수정했다. 후보는 순서가 아닌 정렬된 canonical 집합으로 비교하고 decision은 별도로 검사했다. 기존 테스트를 삭제하거나 약화하지 않았다. 로그는 `/tmp/g8b-*.log`에 남겼다.

## 15. 실제 브라우저 검증

외부 DB 없는 임시 서버 18089, Vite 15189에서 실제 App/DeckEditor/Challenges/GameScreen과 서버 API를 사용했다. production UI에 Standard fixture를 등록하지 않았다.

- 공개 3개 카드의 Legacy/map/board/Normal·Hard/미클리어 표시 확인, screenshot 시각 검사.
- 실제 덱 편집기에서 12×12 Legacy 기본 덱 저장 → Challenges에서 사용 가능/자동 선택 → `raining_men` 시작.
- Black King f12, 구행 g12, Pocket Paratrooper 31, 무제한 시계, 기존 Legacy 양측 Pocket 및 Challenge metadata 확인.
- White Pawn a3→a4 → 실제 Normal Bot 구행 g12→k3 → White Turn 3 및 기보 확인.
- 인간 기권 종료 및 resignation 표시 확인.

첫 로컬 포트 바인딩은 sandbox에서 거부되어 승인된 임시 개발 서버 실행으로 재시도했다. 처음 실패한 탭의 오류 페이지 대신 정상 새 탭으로 연결했다. 다른 브라우저/보안 설정을 바꾸지 않았다. 테스트용 proxy 설정·서버·탭은 정리했다. Standard 전체 경로는 위 서버 통합 테스트이며 공개 브라우저 콘텐츠를 추가하지 않았다. 모바일/외부 계정/실제 DB/모든 Challenge의 전체 대국 실기는 미실행이다.

## 16. G1~G8 최종 회귀와 요구 테스트 대조

| Goal / 요구 번호 | 이번 증거 |
| --- | --- |
| G8-B 1~15 definition/기존 Legacy | 독립 golden, registry invalid cases, summary 및 3개 구형 Legacy 요청 생성/빈 Hand·Draw·Extra |
| G8-B 16~23 player validation | Standard 성공, 양방향 ruleset 불일치/맵/크기/빈 draft/누락 및 override 거부, 기존 실제 router 회귀 |
| G8-B 24~32 Standard creation | initial 3/3/+1, privacy/public Extra/snapshot, Standard Base 예외/정상 Front, 실제 turn Draw, 고지 map |
| G8-B 33~39 Bot | fixture hidden 타입 후보/decision 동등성, 실제 Hand Drop/ExtraSummon, G8-A 전체 관측·subset·RNG 회귀 |
| G8-B 40~48 record/replay/analysis | Challenge mode/ID/version, 원래 Extra snapshot, actual action Draw, 완료 exact action hash와 Analysis root version |
| G8-B 49~53 clear | 양 format 상대 기권 위조 거부/인간 패배/서버 승리/idempotency; 기존 registry ID 유지 |
| G8-B 54~60 UI | 실제 Vue setup: Legacy/Standard 선택, 불일치·draft 거부, 이유/표시/clear source 연결; 실제 Legacy 브라우저 시작 |
| G8-B 61~65 privacy | 생성/Bot 전체 JSON hidden reserve 부재/양측 Extra/stats 생략/완료 record 양측 Hand |
| G1 / 66~76 전체 회귀 | Board/Map/Ruleset 생성·저장·sync 및 pre-G1 Legacy hash 전체 suite |
| G2 | 모든 지원 크기 Base/Home/Front, 배치, Drop/탄약/air/forced landing |
| G3-A/B/C | Hand 불변조건, initial/actual Draw, privacy, exact Replay/Analysis, 멱등성 |
| G4/G5 | Extra count/eligibility, atomic 제물/소환, canonical action/Draw |
| G6 | Standard HUD/interaction/privacy/Legacy UI 자동 회귀 |
| G7 | DC1~DC4, fingerprint/저장, semantic version gates 및 replay codec |
| G8-A | 공정한 Bot, bounded Extra subset, Legacy 3난이도 및 명시 benchmarks |

요구 번호는 위 신규 테스트와 기존 전체 회귀를 함께 대응한다. 76개의 독립 신규 test 함수를 만들었다는 뜻은 아니다. 외부 PostgreSQL 검증을 자동 통과 결과에 포함하지 않는다.

## 17. DB / 배포 전 체크리스트 / 남은 위험

G8-B 신규 DB migration은 없다. clear는 기존 challenge_id 기반 테이블, record는 기존 JSONB를 사용한다. 기존 migration을 수정하거나 실행하지 않았다. **G3-C `game_analysis_nodes.draws` forward migration은 새 서버 배포보다 먼저 적용해야 한다.**

배포 전 남은 작업:

1. 해당 migration을 적용한 disposable PostgreSQL에서 기존 opt-in 분석/저장/동시성 통합 테스트 실행.
2. 서버와 프론트를 함께 배포하여 Challenge summary ruleset 및 player map/size 계약 일치 확인.
3. 운영 계정의 기존 clear/저장 덱을 삭제·변환하지 않고 실제 로그인 조회/플레이 smoke 확인.
4. 실제 Standard Challenge 콘텐츠는 별도 사용자 정의 Goal에서 결정. 이번 test fixture는 배포 목록에 없음.

G8-A의 bounded 검색은 전수 최적성/엄격한 시간 보장이 아니며 큰 Hand/custom/analysis tree 성능 위험은 기존 문서대로 남는다. 구형 Legacy 클라이언트의 map/size 누락 호환 경로는 명시값 검증과 구분된다. 클라이언트 저장 이력을 인증하는 새 서버 덱 저장 시스템은 만들지 않았다. 기존 개발 단계 Standard 기록을 새 version으로 강제 변환하지 않는다.

## 18. 완료 판정

G8-B 구현 완료: 명시 ruleset/map/Extra, authoritative player format과 공식 validation, 세 Legacy 콘텐츠/구행 Board/clear 보존, initial Draw, fair Bot/ExtraSummon/privacy, 기존 HUD, Challenge mode/semantic record version 및 exact Replay/Analysis를 연결하고 요구된 로컬 회귀·build/check를 통과했다.

G1~G8의 Standard Format **구현과 로컬 최종 회귀는 완료**다. 실제 외부 DB 통합 검증·migration 적용·운영 배포 완료를 뜻하지 않는다. 공개 Standard Challenge 콘텐츠 추가는 별도 범위다.
