# 계정 덱 저장 배포 전 검토

검토일: 2026-09-08. 이미 구현된 계정 덱 저장 변경분을 기준으로 안정화했다.
운영 및 실제 테스트 서비스 배포/DB migration은 실행하지 않았다.

## 1. Storage Validation / Game Start Validation

**문제 있음 → 수정.** 서버 저장 경로가 `build_game_state_with_variant`를 호출했고,
에디터 저장 버튼도 `validateSavedDeck().valid`를 사용했다. 이 때문에 King/앞줄/점수 등
대국 규칙을 만족하지 않는 중간 상태를 저장할 수 없었다.

저장 경로는 JSON 타입과 허용 필드, 이름, 보드 크기, 맵, 좌표 범위, 중복 칸,
배치/포켓 수량의 안전 상한, 기물 참조, 소유권, 고정 버전과 해시를 검증한다.
커스텀 기물은 기존 `version(owner, id, version)` 조회를 재사용한다.
비활성화된 소유 기물의 고정 버전도 편집·저장할 수 있으며 런타임 이미지 로딩은 요구하지 않는다.

대국 시작 경로는 기존 `resolve_custom_packages` 및 엔진 검증을 그대로 사용한다.
King, 점수, 앞줄, 시작 진영, 포켓 규칙과 기물 활성 상태는 계속 검사한다.
프론트엔드의 좌표/수량/이름/기물 존재 확인은 `validateDeckStructure`로 공유하고,
저장 버튼에는 `validateDeckForStorage`, 대국 선택과 덱 코드 공유에는 기존 대국 검증을 연결한다.

회귀 테스트에서 빈 배치, King만 있는 배치, King 두 개, 과다 점수, King 포켓,
진영 밖 배치를 저장한 뒤 새 조회로 읽고 완성 덱으로 수정·저장했다.
동일 미완성 덱의 `/games` 요청은 400, 완성 덱은 200이다.
비활성 커스텀 고정 버전은 저장 가능하며 새 대국 요청은 422다.
좌표 중복·보드 밖 좌표·잘못된 수량 등 구조 오류는 계속 거부한다.

## 2. create_hash / 중복 생성

**문제 없음 → 구현 유지, 테스트 보강.** DB의 UNIQUE는 `(owner_id, request_key)`이며
`create_hash`에는 UNIQUE 제약이 없다.

- 일반 CREATE는 `create:{requestId}`를 사용한다. 내용과 이름까지 같은 요청도 requestId가 다르면 별개 덱이다.
- 복제 명령은 새 requestId를 발급하고 기존 revision을 제외한다. 실제 프론트엔드 복제 함수를 테스트했다.
- 동일 CREATE의 재전송은 기존 결과를 반환한다.
- import는 `import:{정규화된 내용의 해시}`를 사용하여 반복/동시 요청을 중복 생성하지 않는다.
- 같은 이름의 다른 내용은 별도로 가져온다. 이미 가져온 덱을 수정했어도 재import가 수정 내용을 덮어쓰지 않는다.

HTTP 테스트, 프론트엔드 repository 테스트 및 실제 PostgreSQL 재접속/동시 import 테스트가 통과했다.

## 3. X-Deck-Account

**문제 있음 → 수정.** 이전 `AuthUser.id`와 헤더 값은 `shared.users.id`였다.
기존 `public_id`는 nullable이며 사용자가 변경할 수 있어 안정적인 계정 전환 비교값으로 쓸 수 없다.

기존 AuthState의 HMAC-SHA256 서명 키에 전용 용도 문자열을 적용하여 불투명한 계정 비교값을 만든다.
DB 컬럼, ID 발급/등록 시스템, 새 의존성은 추가하지 않았다.
`/auth/me`, Google 완료, 프로필 수정 응답의 `user.id` 및 세션 JSON의 `userId`는 이 값이다.
프론트엔드는 기존 응답 연결을 그대로 사용한다. JSON 필드 이름과 구조는 유지한다.
내부 사용자 키는 이 JSON 응답들과 `X-Deck-Account`에 노출하지 않는다.
기존 HttpOnly 세션 쿠키, 토큰 검증 및 Google 로그인 절차는 변경하지 않았다.

owner는 항상 **서명 세션 → authenticated_user → 내부 owner**로 결정한다.
헤더는 해당 owner에서 계산한 비교값과 같은지만 검사한다.
유효 세션에서 헤더 누락/변조/다른 계정 값/기존 내부 ID는 409다.
헤더만 보내면 401이며 이 비교값을 세션 토큰으로 사용해도 인증되지 않는다.
정상 비교값을 가진 다른 계정의 read/update/delete는 기존대로 404다.
실제 서명 쿠키를 사용한 테스트와 공개 ID 변경 전후 비교값 유지 테스트가 통과했다.

호환성 영향: 인증 JSON의 식별자 값이 내부 키에서 불투명한 값으로 바뀐다.
저장 형식과 덱 ID는 그대로다. 구버전 화면은 409 후 새로고침이 필요하며,
서명 키 교체 시에도 계정 상태를 다시 조회해야 한다.

## 4. 64 KiB 제한

추측 대신 기존 camelCase 요청 JSON 전체를 UTF-8 직렬화하여 측정했다.
CREATE에는 UUID requestId, UPDATE에는 최대 길이 expectedVersion을 포함했다.

| fixture | CREATE bytes | UPDATE bytes |
|---|---:|---:|
| 대표 일반 덱: 8×8, 시작 9개, Knight 포켓 2개 | 628 | 615 |
| 12×12 대형: 시작 36개, 커스텀 참조/포켓 32종 | 16,778 | 16,765 |
| 최대급 형식: 시작 144개, 커스텀 참조/포켓 256종 | 106,172 | 106,159 |

대형/최대급에는 100개의 4-byte Unicode 문자 이름, 최대 64자 exposed key,
UUID 기물 ID, 실제 길이 fnv1a64 해시, 10자리 DB 버전을 사용했다.
포켓 합계는 4,096개다. 최대급 fixture는 한 논리 기물의 여러 고정 버전을 사용하므로
100개 논리 기물 제한을 피하기 위한 가짜 ID를 추가하지 않는다.

**문제 있음 → 요청 제한 65,536 → 131,072 bytes(128 KiB)로 수정.**
최대급 대비 약 23.5% 여유다. 라우터와 저장 검증이 같은 상수를 사용한다.
한도를 넘는 요청은 413으로 거부한다.
최대급 fixture의 실제 PostgreSQL `octet_length(deck_data::text)`는 **109,272 bytes**였다.
기존 JSONB 제한 131,072 bytes 안에 들어가므로 DB migration/제약은 변경하지 않았다.

별도 테스트는 커스텀 기물 API로 실제 256개 고정 버전을 만든 뒤
144개 시작 배치와 256종 포켓을 가진 **101,256 bytes** 덱의 CREATE → GET → UPDATE를 실행해 통과했다.
따라서 크기 측정만 하고 소유 기물 참조 검증을 생략한 성공으로 간주하지 않는다.
고정 길이 최대급의 DB 저장·재접속 검증과 실제 버전 API 검증을 각각 수행한다.

## 5. Session transition

**문제 없음 → 기존 구현 유지, 회귀 테스트 보강.**

A → 로그아웃 → B 전환 때 repository generation/identity revision이 바뀐다.
늦은 A GET은 게스트 또는 B 목록을 교체하지 않는다.
늦은 A SAVE는 Promise를 거부하며 저장 성공으로 처리되지 않는다.
B의 기존 덱 목록 및 mock 서버 데이터가 유지되고 계정 덱이 localStorage로 복사되지 않는다.
서버의 세션/헤더 비교와 모든 CRUD의 owner 조건도 별도로 검증했다.

기존 게스트 키, 로그인 시 명시적 import, 부분/전체 import 실패 시 원본 보존,
DB 오류의 명시적 오류 표시, 로그아웃 시 게스트 복귀 테스트가 모두 통과했다.

## 6. 이번 안정화에서 변경한 파일

초기 계정 저장 구현으로 이미 변경되어 있던 파일 전체가 아니라 이번 검토의 추가 변경 목록이다.

| 파일 | 변경 이유 |
|---|---|
| `server/src/deck.rs` | 저장/대국 검증 분리, 중복 칸 구조 검사 유지, 불투명 계정 비교값, 실측 크기 제한 |
| `server/src/auth.rs` | 기존 서명 키로 비교값 생성 및 브라우저 응답에서 내부 키 제외 |
| `server/src/routes.rs` | 요청 크기 상수 공유 |
| `server/src/deck/tests.rs` | 미완성 저장/대국 거부, 고정 버전, 동일 생성, 헤더, 실제 크기, PostgreSQL 회귀 |
| `frontend/src/composables/useDeckValidation.ts` | 공통 구조 검증과 저장 검증 분리 |
| `frontend/src/views/DeckEditor.vue` | 저장 버튼에 저장 검증 연결, 저장 가능/대국 불가 상태 표시 |
| `frontend/src/composables/deckRepository.test.ts` | 미완성 저장, 복제, A→로그아웃→B 늦은 GET/SAVE 회귀 |
| `frontend/src/composables/useDeckCode.test.ts` | 보드 밖 좌표가 공통 구조 단계에서 거부됨에 따라 오류 문구 기대값 갱신; 거부 assertion 유지 |
| `frontend/src/api/authApi.ts` | 기존 id 필드가 불투명 비교값임을 문서화 |
| `docs/account-deck-persistence.md` | 초기 보고서의 저장 검증/ID/64KiB 설명 정정 |
| `docs/account-deck-predeployment-review.md` | 이번 검토 결과, 실측치와 E2E 절차 |

일반 CREATE/import/복제 구현, 로컬 repository, 계정 전환 상태 관리, migration 및 환경 격리 구현은 유지했다.
게임 엔진, Chessembly, 커스텀 기물 시스템, 매치메이킹, 대국 기록, 봇 및 타임 컨트롤을 변경하지 않았다.
기존 사용자 `PieceLab.vue` 변경은 그대로 보존했다. 새 의존성은 없다.

## 7. 테스트 결과

최종 실행 결과를 기준으로 기록한다.

| 실제 실행 명령/검증 | 결과 |
|---|---|
| `cargo test -p brainfuck-chess-server --offline --quiet` | 99 pass / 0 fail / 8 ignored |
| `cargo test -p brainfuck-chess-server --offline deck::tests::postgres_ -- --ignored --nocapture` | 1 pass / 0 fail |
| `cargo test -p brainfuck-chess-server --offline database::tests::postgres_contract -- --ignored --nocapture` | 1 pass / 0 fail |
| `npm test --prefix frontend` | 19개 테스트 파일 pass / 0 fail |
| `node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON --experimental-strip-types frontend/src/composables/deckRepository.test.ts` | 13개 시나리오 pass / 0 fail |
| `npm run typecheck --prefix frontend` | pass |
| `npm run lint --prefix frontend` | pass; 저장소 lint 명령은 vue-tsc 검사 |
| `npm run build --prefix frontend` | pass |
| `cargo check -p brainfuck-chess-server --offline` | pass |
| `cargo fmt --all -- --check` | pass |
| `git diff --check` 및 이번 변경 diff 검토 | pass |

PostgreSQL 16 일회용 컨테이너에서 ownership fixture를 만든 후 shared/prod/test 릴리스 SQL
11개를 두 번 실행했다. `verify_prod_contract.sql`, `verify_test_contract.sql`,
`verify_account_decks.sql`이 모두 통과했다. 외부 DB 환경변수는 이 로컬 컨테이너 주소로만 지정했다.
실제 운영/테스트 DB에는 접속하지 않았다. 검증 컨테이너는 종료·자동 삭제했다.

Browser UI: 로컬 빌드에서 King 제거 → 저장 가능/대국 불가 표시 → 저장 → 새로고침 →
다시 편집 → 이름 수정 → 저장을 확인했다. 덱 코드 복사는 미완성 상태에서 계속 비활성화된다.
검증 탭과 로컬 서버는 종료했다.

초기 테스트 과정의 실패: 새 프론트엔드 fixture가 목록을 조회하지 않고 복제를 호출한 문제와
기존 맵 정규화를 잘못 가정한 문제를 고쳤다. 기존 덱 코드의 보드 밖 좌표 테스트는 새 구조 검증의
정확한 오류 문구를 기대하도록 수정했다. 거부 동작을 완화하거나 테스트를 제거하지 않았다.
최종 실패는 없다.

남은 경고는 기존 Rust dead_code 2건과 Vite 500KB 초과 청크 경고다.
전체 서버 테스트의 외부 환경 의존 ignored 테스트 중 이번 요청과 관련된 2개만 일회용 DB로 별도 실행했다.
나머지는 환경을 임의로 연결하지 않았다. 실제 Google 로그인/두 브라우저 계정 E2E는 아래 수동 검증 대상이다.

## 8. 테스트 서버 E2E 체크리스트

테스트 서버에 기존 test 릴리스 migration을 적용하고 런타임 계약 확인을 마친 후 실행한다.
이번 작업에서는 해당 배포나 migration을 실행하지 않았다.

1. 브라우저 A 비로그인 상태에서 덱을 만든다. 미완성 상태도 저장한다.
2. 새로고침 후 게스트 덱이 유지되는지 확인한다.
3. Google 계정으로 로그인한다.
4. 로컬 덱 import 안내가 표시되고 자동 업로드가 일어나지 않는지 확인한다.
5. 로컬 덱을 계정으로 가져온다. 반복 클릭해도 중복되지 않고 원본은 유지되어야 한다.
6. 브라우저 B에서 같은 Google 계정으로 로그인한다.
7. A에서 가져온 계정 덱이 B에 보이는지 확인한다.
8. A에서 해당 덱을 열어 둔 상태로 B에서 이름/배치/포켓을 수정하고 저장한다.
9. A의 오래된 편집 내용을 저장하면 409 충돌을 안내하고 B의 변경을 덮어쓰지 않는지 확인한다.
10. 브라우저 A에서 로그아웃한다.
11. 로그인 이전 게스트 덱이 그대로 다시 보이는지 확인한다. 계정 편집 내용은 자동 복사되면 안 된다.
12. A에서 같은 Google 계정으로 다시 로그인한다.
13. B가 수정한 계정 덱이 유지되는지 확인한다.

추가 확인: 미완성 덱은 새 대국 선택/시작이 거부되고 완성 후에는 성공해야 한다.
동일 덱 복제는 별개 ID여야 한다. 네트워크 지연 중 A→로그아웃→다른 계정 B 전환 시
늦은 저장 성공/덱 섞임이 없어야 한다. 요청 헤더와 인증 JSON에는 내부 사용자 키가 없어야 한다.

## 9. 최종 판단

**READY FOR TEST DEPLOYMENT**

확인된 저장 과검증, 내부 ID 노출, 64KiB 부족 문제를 수정했고 배포를 막는 코드 문제는 남아 있지 않다.
이는 테스트 서버 배포 준비 판단이다. 실제 Google 로그인/다중 브라우저 E2E는 배포 후 수행해야 하며,
production 배포 승인을 의미하지 않는다.
