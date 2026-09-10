# G6 — Standard Game UI / HUD

작성일: 2026-09-10 (KST). 첨부된 G6 목표와 G1~G5 문서를 기준으로 구현했다. 작업 시작 시 존재한 미커밋 변경을 보존했으며 `/tmp/g6-baseline`과 비교해 아래 G6 변경만 검토했다. 엔진 규칙, Draw/RNG, 소환 정책, 저장 형식, SQL migration은 변경하지 않았다.

## 1. Standard Game HUD 구조

`GameScreen.vue`의 Standard 분기에서 보드를 중심으로 위에 상대 reserve와 시계, 아래에 자기 시계·Hand/Pocket·현재 interaction 안내를 배치한다. 데스크톱 오른쪽은 양측 Extra와 소환 조작, 드로우 안내, 기존 기보다. 플레이어 이름, 현재 턴, 시계, 연결 상태, 게임 결과 UI는 기존 서버 상태를 사용한다.

로컬 2인은 현재 플레이어의 Hand를 아래에 배치하며 기존 보드 회전과 함께 Standard 시계의 위/아래 진영도 바뀐다. Legacy의 원래 양측 Pocket 레이아웃은 그대로다.

## 2. Hand UI

공용 `StandardReservePanel.vue`가 명시적인 `deck.hand_pieces`를 읽는다. 보이는 기물마다 기존 이미지 렌더러, 이름, 정의 score, 선택 표시를 제공한다. PieceId는 내부 key와 서버 제출에만 사용하며 카드 문구나 DOM data 속성에 넣지 않는다.

Hand 선택 → 기존 `getLegalDrops` → 선택한 ID에 대한 서버 target 강조 → 보드 클릭 → 기존 canonical Drop intent 제출 → `stateUpdate` 응답 반영 순서다. 합법 위치를 클라이언트에서 계산하지 않는다. Hand 제거와 Board 추가는 응답 전에는 발생하지 않는다. 같은 기물 재선택, 취소 버튼, Escape로 선택을 취소할 수 있다.

## 3. Pocket UI

자기 Pocket은 Hand와 같은 reserve 패널의 접을 수 있는 상세 영역이다. 목록과 score를 보여주고 “드로우 및 능력용”, “일반 착수는 Hand에서 선택”을 안내한다. Pocket 항목은 일반 Drop용 버튼이나 drag source가 아니다. Standard의 기존 Pocket click/drag handler도 일반 Drop을 시작하지 않는다.

공수부대·교대 등 기존 Ability UI는 원래 Pocket 출처와 제출 경로를 유지한다. Pocket이 비면 빈 상태를 표시한다. 상대 Pocket count는 현재 G3 wire에 없으므로 0이나 추정 장수를 만들지 않고 `Pocket: 비공개`로 표시한다.

## 4. Extra UI

G5의 `ExtraSummonPanel.vue`를 확장했다. 양측의 실제 `extra_deck_pieces`와 공개 Piece object로 이미지·이름·score·남은 인스턴스 수를 표시한다. 동일 타입의 여러 인스턴스는 각각 선택할 수 있고 타입별 남은 수량을 함께 안내한다. 상대 Extra는 보기 전용이며 현재 플레이어 권한이 있는 경우만 자기 Extra를 선택한다.

새 정책이나 기물 타입 분기, custom Extra 제작 기능은 없다. 공개/자기 기물 이미지에는 기존 `renderedPieceAsset`를 사용한다.

## 5. ExtraSummon interaction

한 패널에서 ① Extra 선택 ② 제물 선택 ③ 점수 확인 ④ 위치 단계 전환 ⑤ 보드 또는 좌표 버튼 선택 ⑥ 소환 확정/취소를 수행한다.

후보와 비용, 허용 zone, 각 선택의 canonical target은 기존 summon-options API에서 받는다. Board 후보는 점선 및 “제물 후보”, 선택된 제물은 실선 및 “✓ 제물”로 구분한다. Hand 카드도 같은 서버 후보 목록으로 선택 가능 상태를 표시한다. Panel의 개별 checkbox 목록은 ground/air가 같은 칸에 겹친 경우에도 각각 선택할 수 있다. 겹친 칸의 square click은 임의로 한 기물을 고르지 않는다.

구행 Hand+Board / 폭격기 Board 구분, King·상대·Pocket·Extra 제외는 서버가 반환한 후보에 따른다. `toggle`도 후보 밖 ID를 받아들이지 않는다. 비용 미달에는 target/확정이 없고, 충분한 점수라도 서버 target이 없으면 소환할 수 없다고 안내한다. 합계/필요 점수, 선택한 기물 전체, 초과 점수와 환급 없음, 선택 위치를 표시한다. 제물을 줄이거나 조합 전체를 자동 생성하지 않는다.

요청 거부 시 현재 state에서 여전히 유효한 선택과 target을 유지하고 안전한 오류를 표시한다. 실제 state 변경은 선택을 정리한다. 취소 후 늦게 온 조회/오류 응답은 무시한다. 제출 중 취소하더라도 이미 전송한 action 자체를 취소했다고 간주하지 않으며 부모의 제출 잠금은 응답까지 유지한다.

## 6. Board interaction state 관리

기존 Move/Ability 상태를 대규모로 재작성하지 않고 `interactionMode` computed로 `None`, `HandDrop`, `Move`, `Ability`, `ExtraSummonSacrifice`, `ExtraSummonTarget`을 표현한다. 소환 시작 시 일반 선택, promotion, 공수 배치 draft, 기존 성소 희생 선택을 함께 정리한다. 소환 중 Board 이벤트는 소환 패널로만 전달한다. Hand 카드 클릭 역시 소환 중에는 제물 선택으로만 전달한다.

`selectionGeneration`은 취소/다른 선택 이후의 늦은 Hand·Move·Ability 후보 응답을 무시한다. 제출 중에는 새 action이나 소환 시작을 막는다. `gameplayKey`가 실제 보드·zone·턴·기물·종료·규칙 관련 상태 변화에서 선택과 후보 cache를 정리한다. 객체 키를 정렬하므로 Rust map의 응답 순서만 바뀐 경우에도 선택을 보존한다. clock, presence, state revision만 변하는 heartbeat는 key에 포함하지 않는다. 같은 플레이어가 유지되는 forced landing 중간의 실제 Board 변화도 정리 대상이다.

## 7. Draw와 턴·시계

최초 playable state의 White 4 / Black 3을 그대로 렌더링한다. frontend initial Draw나 RNG를 실행하지 않는다. 새 Hand 목록/장수는 서버 응답에 따라 즉시 바뀐다. 자동 드로우 안내만 제공하며 Draw 버튼이나 player action을 만들지 않는다. 별도 Draw animation/state나 과거 reserve diff 기반 identity 추정은 없다.

턴은 항상 `current_player`다. action 하나마다 교대를 가정하지 않는다. 기존 시계 계산·서버 time control을 유지하며 소환 선택 중에도 시계가 흐른다. Bot 기존 timeline 재생은 서버 frame을 사용한다. Standard 응답에 frame이 부족한 경우 Legacy용 로컬 action 적용기로 reserve 이동을 추정하지 않고 서버 최종 상태를 표시한다. Standard Bot 착수 안내는 Hand라는 표현과 공개 이름/일반 기물 표현을 사용하고 숨긴 ID를 fallback 문구로 쓰지 않는다.

## 8. Privacy 표현 및 heartbeat 보완

live G3 projection과 capability 계약을 그대로 사용한다. 상대 Hand는 `hand_counts`만, Pocket은 비공개, 양측 Extra는 공개다. hidden reserve ID를 복구하거나 이전 state와 비교해 이름을 붙이지 않는다. reserve 컴포넌트는 `reveal=false`이면 Hand/Pocket 객체 목록을 만들지 않는다. 모든 reserve 목록에서 누락된 Piece object를 안전하게 건너뛴다.

실제 Multiplayer 테스트에서 **빈 custom manifest가 wire에서 생략되는 정상 응답**을 기존 `mergeGameSync`가 오류로 처리하는 문제를 발견했다. `gameApi.ts`에서 빈 manifest를 `[]`로 받아들이도록 보완했다. 새 catalog가 왔을 때 생략된 manifest는 기존 catalog의 package를 유지하지 않고 빈 배열로 교체한다. 따라서 이전에 보였던 custom reserve package가 남는 것도 막는다. definitions/player info 필수 검사는 유지한다. 서버 projection 변경이나 privacy 정책 완화는 없다.

## 9. Local / Bot / Multiplayer

| 모드 | 자기/활성 reserve | 상대 reserve | Extra |
| --- | --- | --- | --- |
| Local 2-player | 현재 player Hand 조작, 양측 Pocket/Hand 열람 | 공유 capability의 반대쪽 Hand도 보기 가능 | 양측 공개, 현재 player만 소환 |
| Bot | 인간 Hand/Pocket | Bot Hand count, Pocket 비공개 | Bot 전략 미지원과 관계없이 양측 공개 |
| Multiplayer | 저장된 localPlayer의 Hand/Pocket | 상대 Hand count, Pocket 비공개 | 양측 공개, 자기 턴만 소환 |

권한은 서버가 최종 검증한다. 사용자 ID, 닉네임이나 기물 이름에서 capability를 추론하지 않는다.

## 10. Responsive

Standard 데스크톱은 최대 680px 보드 열 + 280~360px 소환/기보 열이다. 1000px 이하에서는 한 열로 이어져 보드가 reserve 때문에 지나치게 줄어들지 않는다. 모바일에서는 패딩을 줄이며 Hand/Extra 목록은 가로 스크롤, Pocket은 접는 상세, 제물/target 목록은 높이 제한 스크롤을 사용한다.

실제 390px viewport에서 scrollbar 제외 client 폭 375px, 보드 363px, document scrollWidth 375px로 가로 overflow가 없었다. 820px에서는 client/scrollWidth 805px, 보드 680px였다. 작은 화면에서는 보드와 아래 소환 패널 사이에 세로 스크롤이 필요하다.

## 11. 접근성 / 오류 처리

Hand/Extra에 이름과 점수, 선택 체크 표시, `aria-pressed`, disabled 이유를 제공한다. 보드 제물은 색 외에 점선/체크/문구로 구분한다. Board square에 좌표 label과 Enter/Space 조작, focus 표시를 추가했다. 모든 선택은 별도 취소 동작으로 해제 가능하다. 상태 안내에는 status/live 영역, 오류에는 alert를 사용한다.

Standard action 오류는 작은 UI allowlist로 알려진 공개 오류를 사용자 문구로 변환하고, 알 수 없는 내부 오류는 일반 안내로 바꾼다. 서버 원문 stack/ID를 그대로 노출하지 않는다. Legacy 기존 오류 메시지와 Pocket/Ability 사용 흐름은 보존한다. 별도 접근성 전문 감사나 스크린리더 실기 검증은 수행하지 않았다.

## 12. Replay / Analysis

완료 Replay의 양측 Hand/Pocket을 같은 `StandardReservePanel`로 표시하고 기존 G5 소환 패널에 Board/Hand 후보 강조를 연결했다. 현재 Hand에서 분석 Drop을 선택할 수 있으며 기존 Standard 중복 Hand 버튼 영역은 공용 패널로 대체했다. 선택 취소 버튼을 제공한다.

완료 Replay의 exact Draw 상세와 소환 상세, count-only notation은 유지한다. Analysis의 `draw_pending` 안내와 확정 저장 대기를 유지하며 이를 최종 Hand 상태처럼 표시하지 않는다. 기록/분석 규칙·DB·코덱은 수정하지 않았다.

## 13. G6 변경 파일

- `frontend/src/components/GameScreen.vue`: Standard 배치, Hand Drop 연결, 선택 취소/충돌·async 보호, 상태/시계/기보 표현.
- `frontend/src/components/StandardReservePanel.vue` (신규): 공용 Hand/Pocket, privacy 및 이미지 표현.
- `frontend/src/components/ExtraSummonPanel.vue`: 기존 G5 API 기반 단계/후보/점수/Board interaction 및 오류 UI 확장.
- `frontend/src/components/Board.vue`: 제물 후보/선택 표현, 좌표 label·키보드 조작.
- `frontend/src/standardGameUi.ts` (신규): clock 제외 gameplay key와 안전한 action 오류 문구.
- `frontend/src/views/ReplayPage.vue`: 공용 reserve 및 소환 후보 연결, 기존 Draw 상세 유지.
- `frontend/src/api/gameApi.ts`: sparse empty manifest 병합 및 stale manifest 제거.
- `frontend/src/standardGameUi.test.ts` (신규), `api/gameApi.test.ts`, `replayAnalysis.test.ts`: 실제 Vue setup/DOM render·API·기존 Replay/Panel 회귀.
- `frontend/package.json`: 신규 테스트를 기존 runner에 등록.
- 이 문서: 결과와 증거.

파일 수는 보드/게임/공용 reserve/소환/Replay 표현, sync 경계, 대응 테스트를 각각 연결하기 때문이다. 새 dependency/framework는 없다. 임시 브라우저 harness 4개 파일은 검증 후 제거했다. 기존 G1~G5의 engine/server/SQL 변경은 건드리지 않았다.

## 14. 자동 테스트와 검증

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | 22 test 파일 통과, 실패 0. 신규 G6 12개 test 포함 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 vue-tsc script |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 222 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 141 passed, 9 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| `git diff --check` 및 G6 baseline 비교 | 통과 |

기존 frontend 500 kB 번들 경고와 서버 미사용 생성자 경고 2개가 있다. engine ignored 7개는 벤치마크, server ignored 9개는 기존 외부 DB/진단 테스트다. 실행 로그는 `/tmp/g6-*.log`에 보관했다. 테스트를 삭제하거나 약화시키지 않았다. 기존 G5 Panel 테스트 fixture에는 실제 Extra 소속을 추가하여 현재 선택 guard와 같은 유효 상태를 사용한다.

## 15. 실제 브라우저 / 시각 테스트

외부 DB 없는 로컬 서버(18086)와 임시 Vite harness(15186)에서 **실제 GameScreen, gameApi, 실제 서버의 게임 생성/룸/후보/submit/heartbeat**를 사용했다. harness는 정상 유효 덱으로 게임 시작을 쉽게 하는 용도이며 engine state를 주입하거나 action API를 mock하지 않았다. 따라서 로비/로그인/DeckEditor를 거치는 전체 E2E와는 구분한다.

확인한 항목:

- Standard Local: 초기 Hand 4/3, 자기 Pocket 1/상대 2, 양측 Extra 이름·점수.
- Hand 선택 후 서버 legal target, d1 Drop 확정, Board에 Knight, 흑 Hand 4, 활성 Hand/턴 변경.
- 구행: Hand checkbox/카드 및 Board 직접 선택, 24점 미달 → 25점 충족 → 26점/초과 1점, target d8 선택, 확정, 흑 Extra 2→1, 공개 소환 상세 26점, 다음 백 Draw 반영.
- Standard Multiplayer 두 별도 탭/capability: 백 자기 4/상대 3, 흑 자기 3/상대 4, 상대 reserve DOM에 카드 없음, 양측 Extra 공개.
- Multiplayer heartbeat의 valid Hand 선택 유지; 백 Drop 뒤 양쪽 view에서 흑 Hand count 4/자기 새 카드; 같은 guest의 join 재입장 및 재조회 후 privacy 유지.
- Standard Bot: 인간 Hand identity/Bot count와 양측 Extra, 인간 Drop → 실제 Bot action → 인간 턴 Hand Draw. 이후 봇 preview의 내부 ID fallback 표현도 자동 테스트로 보강.
- Legacy: 기존 양측 Pocket, Hand/Extra UI 없음, 실제 Queen Pocket Drop a1 및 기보 반영.
- 1280px 데스크톱, 820px 태블릿, 390px 모바일 screenshot/DOM/치수 확인. 모바일 Hand 선택·취소, Extra 선택·취소 조작 확인.

초기 테스트에서 보드 도구 폭과 sparse manifest 오류를 찾아 수정하고 재검증했다. 브라우저 viewport를 복원하고 임시 탭·서버를 종료했다. screenshot은 도구 출력으로 직접 확인했으며 별도 이미지 파일은 산출물에 포함하지 않았다.

실제 브라우저 미실행: Replay/Analysis, custom package 실기, forced landing 실기, 모든 Ability 종류, 모바일 소환 최종 확정, 실제 외부 PostgreSQL/배포. 해당 규칙·기록·privacy 회귀는 기존 자동 suite와 새 Vue 테스트로 검증했지만 브라우저 실기와 동등하다고 주장하지 않는다.

## 16. Legacy 회귀

Legacy는 기존 양측 Pocket 표시와 Drop source, Ability 경로, 보드 기하/시계/게임 규칙을 유지한다. Hand/Draw/Extra/Summon 영역은 렌더링되지 않는다. 공통 Board의 선택 label/키보드 지원과 stale interaction 정리는 함께 적용된다. 실제 Pocket Drop, 자동 Legacy UI handler, 전체 엔진/서버/frontend 회귀가 통과했다. Chessembly 코드/문법, Challenge, 덱 코드, 기존 record/hash를 수정하지 않았다.

## 17. G7/G8에 넘기는 계약

G7은 기존 canonical Drop/ExtraSummon, exact DrawResolution, count-only live notation 및 완료 Replay 상세를 그대로 사용한다. Standard Deck Code와 개발 단계 기록 versioning은 여전히 G7 범위다. G3-C의 `game_analysis_nodes.draws` forward migration은 **새 서버 배포 전에 적용**해야 하며 이번에 수정/실행하지 않았다.

G8은 현재 서버의 선택 subset 기반 summon-options/generator, Hand/Pocket/Extra의 각 zone 및 공개 projection을 재사용한다. UI는 봇이 Extra를 사용하지 않더라도 양측 Extra를 계속 표시한다. Bot의 소환/불완전 정보 전략과 Standard Challenge는 추가하지 않았다.

## 18. 남은 위험

- 현재 API에는 상대 Pocket 장수가 없어 비공개로 표시한다. 장수 추가는 별도 서버 공개 계약 변경이며 G6에서 추정하지 않는다.
- 모바일 소환은 보드와 패널 사이 세로 스크롤을 사용한다. 모든 기종/입력 방식의 실기 검증은 하지 않았다.
- 큰 custom catalog/Hand의 gameplay key 생성 성능은 별도 측정하지 않았다. 현재 상태 의미 비교만 수행하며 전체 제물 조합 탐색은 없다.
- 외부 DB migration·SQL 동시성 실기, 운영 배포, G7 저장/버전 정책, G8 Bot 전략은 별도다.

## 19. G6 완료 조건 / 요구 테스트 대조

| 요구 번호 | 증거 |
| --- | --- |
| 1~7 Hand/Pocket | 신규 실제 GameScreen setup의 Hand 선택/서버 target/응답 전 불변/commit, reserve DOM renderer, Legacy 및 Standard Pocket guard; 실제 Local·Multiplayer Drop |
| 8~12 Draw | 서버 G3-B/C suite, 새 initial 4/3·same-player 회귀, 실제 Local/Multiplayer/Bot Draw. 클라이언트 Draw RNG 없음; Board의 기존 annotation ID RNG는 Draw와 무관 |
| 13~16 Extra | 실제 양측 UI·score·소환 후 수량, Extra DOM 및 상대 선택 guard 테스트 |
| 17~30 소환 | 기존 G5 엔진/서버/Panel + 새 후보 외 ID 거부·27점 유지·실패/취소·target·clock 회귀, 실제 26/25 Hand+Board 구행 완주 |
| 31~35 interaction | 새 Hand/Summon 배타성, Move/Ability draft 정리와 늦은 Ability 응답 거부, actual same-turn 변화 취소, reordered clock heartbeat 선택/target 유지 |
| 36~40 privacy | 기존 서버 full/sync/재접속/Bot/custom privacy tests; 새 reserve DOM 및 sparse manifest 교체 테스트; 실제 양측 Multiplayer·재입장·Bot 화면 |
| 41~44 Legacy | 실제 Legacy Pocket Drop, 신규 Legacy setup, 기존 Ability/엔진/서버 전체 suite |
| 45~48 responsive | 세 viewport 시각/치수 확인, 390px Hand/Extra 조작·취소, 가로 목록 스크롤 및 보드 유지 |
| 49~52 Replay/Analysis | 기존 실제 ReplayPage setup의 completed 양측 Hand·exact Draw·draw_pending/저장 대기·소환 상세 tests 및 공용 reserve DOM tests. 실제 Replay 브라우저는 미실행 |

G6 구현 완료: Hand 사용, Pocket 역할 분리, 상대 Hand count/privacy, 양측 Extra 공개, 실제 소환 완주·score/초과량, 배타적 Board interaction, authoritative Draw/turn/clock, Local/Bot/Multiplayer 및 responsive, 기존 Replay/Analysis/Legacy 보존을 구현하고 위 검증을 통과했다. G7/G8 또는 운영 배포 완료를 의미하지 않는다.
