# 계정 덱 저장 구현 보고서

배포 전 안정화 결과와 최신 테스트 수치는 [배포 전 검토 보고서](account-deck-predeployment-review.md)를 참조한다.

1. **기존 덱 저장 구조 분석**

   `useSavedDecks.ts`가 `brainfuck_chess_saved_decks_v1` localStorage 키의 JSON
   배열을 동기적으로 읽고 썼다. `SavedDeck`은 ID·이름·맵·보드 크기·시작 배치·
   기물 종류별 포켓 수량·생성/수정 시각·커스텀 기물 참조를 가진다.
   이전 `mapId`/`customPieces` 누락 데이터의 정규화도 기존 동작대로 보존했다.
   DC1/DC2/DC3 덱 코드는 공유 포맷이며 계정 저장과 독립적이다.
   이름 길이는 서버와 같은 Unicode 문자 수로 확인하고 기존 fnv1a64: 해시를 허용한다.
   기물별 게임 런타임 상태는 원래 저장 덱에 없고 고정된 기물 정의에서 생성된다.
   커스텀 참조는 기존 논리적 `piece_id`와 버전·content hash·exposed key를 사용한다.
   DB의 custom piece version 행 내부 ID나 소유자 ID를 덱에 추가하지 않는다.

2. **변경한 아키텍처**

   비회원은 기존 브라우저 저장소, 회원은 인증된 Deck API → PostgreSQL을 사용한다.
   계정 데이터를 localStorage에 미러링하거나 로그아웃 시 게스트 덱으로 복사하지 않는다.
   로컬 실행에서 DATABASE_URL이 없으면 계정 덱 API는 명시적 503을 반환한다.
   계정 데이터를 메모리에 성공적으로 저장했다고 표시하는 대체 경로는 없다.

3. **추가한 DB migration/table**

   `server/db/{prod,test}/20260908000000_account_decks.sql`이 각각 `decks`를 만든다.
   컬럼은 `id`, `owner_id`, `name`, `format_version`, `deck_data JSONB`,
   `created_at_ms`, `updated_at_ms`, `version`, `request_key`, `create_hash`이다.
   `owner_id`는 `shared.users(id)` 외래키이며 사용자 삭제 시 cascade한다.
   계정별 요청 키 UNIQUE와 목록 인덱스를 둔다. 메타데이터와 기존 덱 내용을 분리했다.
   기존 SQLx migration 이력은 변경하지 않았다. 새 파일은 `migrate-db.sh`에 등록했다.

4. **추가/수정 API**

   기존 `/api` 라우터 아래 GET/POST `/decks`, GET/PUT/DELETE `/decks/:id`,
   POST `/decks/import`를 추가했다. 읽기 응답은 기존 SavedDeck 모양과 `version`이다.
   POST는 `{requestId, name, deckData}`, PUT은 `{expectedVersion, name, deckData}`,
   DELETE는 `{expectedVersion}`, import는 `{name, deckData}`를 받는다.
   모든 응답에 `Cache-Control: no-store`를 적용한다.
   `X-Deck-Account`는 로그인 응답의 불투명한 계정 비교값을 세션과 비교하는 전환 검사용
   헤더다. 내부 사용자 키는 JSON 응답이나 이 헤더에 노출하지 않는다. 이 값을 owner로 사용하지 않으며 owner는 반드시 서버 세션에서 결정한다.

5. **프론트엔드 DeckRepository 구조**

   `deckRepository.ts`의 LocalDeckRepository/AccountDeckRepository가
   `listDecks`, `getDeck`, `saveDeck`, `deleteDeck`의 비동기 계약을 구현한다.
   기존 로컬 구현은 `localDeckRepository.ts`로 이동했다.
   `useSavedDecks.ts`는 중앙 저장소 선택, 목록·오류·로딩·작업 상태,
   복제·이름 변경·로컬 가져오기를 제공한다. UI별 인증 분기는 반복하지 않는다.
   모든 소비 화면의 변경은 비동기 저장소 연결과 관련 상태 표시에 한정했다.

6. **로그인/로그아웃 전환**

   기존 AuthAccount의 `/auth/me`, Google 로그인, 로그아웃 결과로 저장소를 전환한다.
   확인 중/실패 상태를 비회원으로 간주하지 않는다. 로그인 성공 시 서버 목록을 읽고
   로그아웃 시 기존 게스트 목록을 읽는다. 전환 전 요청의 늦은 응답은 폐기한다.
   다른 탭에서 쿠키가 바뀐 경우 세션/헤더 비교로 이전 계정의 쓰기가 새 계정에
   적용되는 것을 막고 새로고침을 안내한다. 인증 시스템과 계정 생성은 재구현하지 않았다.

7. **기존 로컬 덱 migration**

   계정 목록을 조회한 뒤 로컬 덱 개수와 가져오기 버튼을 제공한다. 자동 업로드는 없다.
   사용자가 실행하면 덱별 서버 저장 결과를 확인하고 성공 수와 실패 이유를 표시한다.
   성공·부분 실패·전체 실패 모두 원본 localStorage를 유지한다.
   맵·이름·배치·양수 포켓 수량·커스텀 참조를 정렬 정규화해 SHA-256 식별값을 만든다.
   계정별 UNIQUE와 트랜잭션 잠금으로 반복/동시 가져오기를 중복 생성하지 않는다.
   이미 가져온 덱을 계정에서 수정했다면 반복 가져오기가 수정 내용을 덮어쓰지 않는다.
   같은 이름의 다른 내용은 별개 덱으로 보존하고 `(가져옴 N)` 접미사로 구분한다.
   사용자가 계정 덱을 삭제한 뒤 다시 가져오면 새 덱을 생성한다.

8. **production/test 격리**

   기존 DataSchema 설정을 재사용한다: prod → prod, test/local → test.
   계정/인증은 shared에 유지하고 덱은 선택된 환경 테이블만 쿼리한다.
   런타임에는 자기 환경 CRUD만 부여하며, 마이그레이션 중 임시 관리자 역할은 회수한다.
   두 환경의 릴리스 계약과 서버 시작 계약에 덱 테이블/컬럼/CRUD 검사를 추가했다.
   반대 환경 USAGE 차단도 유지한다. 실제 운영/테스트 서비스 DB에는 적용하지 않았다.

9. **보안 및 ownership 검증**

   서명된 게스트 세션만으로 접근할 수 없으며 `authenticated_user`로 등록 계정을 확인한다.
   모든 SQL CRUD는 owner 조건을 사용하고 다른 사용자의 read/update/delete는 404다.
   쓰기 origin 검사, 실측에 근거한 128 KiB 요청 제한, 이름 100자, 계정당 200개,
   보드 8~12, 배치 144개, 포켓 종류 256개/종류당 1024개/합계 4096개 제한을 둔다.
   예상치 못한 JSON 필드를 거부한다. 저장 시 맵·좌표·중복 배치·수량 및 참조 구조·
   커스텀 기물 소유권/고정 버전/해시를 확인한다. 미완성 덱도 저장할 수 있다.
   앞줄·King·점수·배치/포켓 규칙·커스텀 기물 활성 상태는 대국 시작 시 검사한다.
   수정과 삭제는 서버 revision을 원자적으로 비교하여 오래된 요청을 409로 차단한다.
   포맷을 읽을 수 없거나 DB가 실패하면 빈 목록/성공으로 변환하지 않는다.

10. **추가한 테스트**

    프론트엔드 9개 시나리오: 기존 로컬 JSON/새로고침, 계정 CRUD/두 클라이언트,
    로그인·로그아웃, 명시적 가져오기/반복, 실패 시 원본 보존, 인증 불명 상태,
    늦은 조회 응답 폐기, 코드 공유/실제 커스텀 해시, 부분 가져오기,
    전환 후 늦은 저장 성공 방지(일부 테스트는 여러 시나리오를 함께 검증한다).
    서버는 인증·CRUD·소유권·중복 요청·수정 충돌·입력 제한·커스텀 참조·
    가져오기 이름 충돌·DB 미설정 실패를 테스트한다.
    PostgreSQL 통합 테스트는 재접속, 동시 생성/수정, 실제 SQL 소유권 조건,
    환경별 데이터 독립성과 cascade를 검증한다. SQL fixture는 앱 역할 CRUD와
    FK/양방향 접근 거부를 확인한다. 시작 계약 테스트에는 덱 UPDATE 권한 누락을 추가했다.

11. **실제 실행한 검증과 결과**

    - `cargo test -p brainfuck-chess-server --offline`: 94 통과, 8 기본 제외.
    - `cargo test -p brainfuck-chess-server --offline deck::tests::postgres_ -- --ignored`:
      임시 PostgreSQL 16에 TEST_DECK_DATABASE_URL을 지정해 통과.
    - `cargo test -p brainfuck-chess-server --offline database::tests::postgres_contract -- --ignored`:
      임시 관리자/prod/test 역할 URL로 통과.
    - `npm test --prefix frontend`: 19개 테스트 파일 통과.
    - `node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON --experimental-strip-types frontend/src/composables/deckRepository.test.ts`:
      새 테스트 9개를 직접 실행하여 통과 확인.
    - `npm run typecheck --prefix frontend`, `npm run lint --prefix frontend`,
      `npm run build --prefix frontend`: 통과. Vite의 500 kB 청크 경고는 남아 있다.
    - `cargo check -p brainfuck-chess-server --offline`, 로컬 서버 빌드/기동,
      `cargo fmt --all -- --check`, `git diff --check`: 통과.
      Rust의 기존 미사용 함수 경고 2건은 남아 있다.
    - 임시 PostgreSQL에서 ownership fixture → shared/prod/test 릴리스 SQL을 2회 적용:
      성공. `verify_prod_contract.sql`, `verify_test_contract.sql`, `verify_account_decks.sql`: 통과.
    - 실제 로컬 브라우저에서 게스트 덱 생성 → 이름 편집 → 저장 → 새로고침 →
      동일 덱 복원 → 덱 코드 복사 성공을 확인했다.
    - 첫 DB 실행은 샌드박스 네트워크 차단으로 실패하여 로컬 임시 DB 연결 권한으로
      재실행했다. 넓은 postgres 필터로 선택된 기존 외부 DB 테스트 5건은 별도
      환경변수가 없어 실행되지 못했으며, 필요한 덱/시작 계약 테스트만 정확히 재선택했다.
    - 기존 `verify_ownership_migrations.sql`은 새 retention 이전의 game_records DELETE
      금지를 전제로 하여 전체 최신 migration 후에는 실패했다. 해당 기존 테스트는
      변경하지 않았으며, 현재 릴리스 계약과 덱 전용 권한 테스트는 통과했다.

12. **변경한 파일 목록과 이유**

    저장/API/상태 계약:
    `frontend/src/api/deckApi.ts`, `frontend/src/composables/deckRepository.ts`,
    `frontend/src/composables/localDeckRepository.ts`, `frontend/src/composables/useSavedDecks.ts`,
    `frontend/src/types/deck.ts`.

    인증 결과 연결 및 비동기 소비 화면:
    `frontend/src/components/AuthAccount.vue`, `frontend/src/App.vue`,
    `frontend/src/views/DeckLibrary.vue`, `frontend/src/views/DeckEditor.vue`,
    `frontend/src/views/DeckSelect.vue`, `frontend/src/views/MultiplayerLobby.vue`,
    `frontend/src/views/BotDebugger.vue`, `frontend/src/views/Challenges.vue`.
    봇/챌린지 규칙 변경 없이 덱 로드와 상태 표시만 연결했다.

    기존 커스텀 코드 호환성 및 회귀 테스트:
    `frontend/src/composables/useDeckCodeCodec.ts`,
    `frontend/src/composables/deckRepository.test.ts`, `frontend/package.json`.

    서버 저장소/검증/배선:
    `server/src/deck.rs`, `server/src/deck/tests.rs`, `server/src/app_state.rs`,
    `server/src/routes.rs`, `server/src/main.rs`, `server/src/database.rs`,
    `server/src/auth.rs`(기존 origin 검사의 모듈 내 재사용만 허용).

    DB 릴리스/검증/운영 문서:
    `server/db/prod/20260908000000_account_decks.sql`,
    `server/db/test/20260908000000_account_decks.sql`,
    `server/db/admin/verify_prod_contract.sql`, `server/db/admin/verify_test_contract.sql`,
    `server/db/testing/verify_account_decks.sql`, `migrate-db.sh`,
    `server/db/prod/README.md`, `server/db/test/README.md`, `server/db/testing/README.md`,
    `docs/account-deck-persistence.md`.
    작업 시작 전부터 있던 `frontend/src/views/PieceLab.vue` 수정은 그대로 보존했다.

13. **남아 있는 위험요소 및 후속 작업**

    배포 전에 승인된 기존 DB 릴리스 절차로 해당 환경 migration을 적용해야 한다.
    미적용 상태에서는 서버 시작 계약이 배포를 차단한다. 운영 배포는 수행하지 않았다.
    실제 Google 로그인으로 두 물리 기기를 연결한 UI 검증은 하지 않았다.
    계정 전환/두 클라이언트 동작은 프론트엔드 시뮬레이션, 인증된 서버 요청,
    독립 PostgreSQL 연결로 확인했다.
    권한을 이전하지 않은 게스트 커스텀 기물이나 현재 규칙에 맞지 않는 로컬 덱은
    가져오기에 실패할 수 있다. 이유를 표시하며 원본은 보관한다. 커스텀 기물의 기존
    로그인 시 데이터 이전 선택은 덱 가져오기와 별개로 유지된다.
    실시간 동기화는 추가하지 않았다. 다른 기기의 변경은 목록 재진입/새로고침으로
    조회하며 충돌 시 기존 편집 내용을 덱 코드로 보관한 뒤 다시 열 수 있다.
