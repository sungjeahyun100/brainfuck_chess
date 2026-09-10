# G9 — Standard Format Deployment Readiness / Production Migration Validation

검증일: 2026-09-10 KST. **최초 G9: NO-GO → G9-DB-01 수정·재검증 후: DB readiness GO.**

최신 결과는 §13이다. §1~§12는 최초 실패와 검증 이력을 보존한다. DB readiness GO는 운영 배포 실행이나 실제 production smoke 완료를 뜻하지 않는다. 이번 Goal에서는 운영 배포·Cloud SQL 접속을 하지 않았다.

**최초 검증 당시:** 기본 자동 회귀·migration·startup·격리는 통과했지만 필수 G3-C SQL 동시 create 멱등성 테스트가 두 번 실패했다. 따라서 “전체 자동 테스트 통과”와 G9 완료 조건은 충족되지 않았다. 검증 보고서 작성 완료를 배포 승인 또는 G9 모든 gate 완료로 해석하지 않는다.

## 1. 검증 대상과 변경 범위

- 시작 HEAD: `589a00a9fb31fe3e377c04644b4046cababdba57` + 작업 트리의 기존 G1~G8 변경. 이 HEAD만 배포하면 이번 검증 대상과 다르다.
- G9에서는 gameplay, Standard 규칙, Bot heuristic, 공개 Challenge, 기존 release SQL, migration runner, 개발단계 Standard record를 변경하지 않았다.
- 추가 내용은 이 문서, `server/db/testing/g9_deployment_fixture.sql`, 기존 test-only 파일 `server/src/draw/g3c_tests.rs`와 `server/src/challenge/g8b_tests.rs`의 opt-in 검증이다. fixture는 실행 가능한 계정/커스텀 저장 테이블 형태를 제공하고, 테스트는 기존 repository와 완료 처리 경로를 사용한다.
- 기존 tracked diff는 시작 사본과 최종 사본이 byte-identical이었다. 두 test-only 파일은 G9 시작 시 이미 untracked였으며 테스트만 추가했다.
- 운영 DB/Cloud SQL/서비스/계정 데이터에는 접속·migration·삭제·변환·배포를 수행하지 않았다. `release.sh`도 실행하지 않았다.

## 2. 실제 migration과 순서

G3-C 기존 forward release:

| 환경 | 파일 | SHA-256 |
| --- | --- | --- |
| prod | `server/db/prod/20260910000000_analysis_draws.sql` | `23011c8c0819561f2e5698ec09b0a30a16c21c6f6211b13e78e68876a03c104c` |
| test | `server/db/test/20260910000000_analysis_draws.sql` | `e4dc8c27905b00ea970b8052bd118f15007c49d8faf6d97a8d2efa9ad60d065f` |

각 파일은 해당 환경의 `game_analysis_nodes`에 `draws JSONB NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(draws) = 'array')`를 추가한다. transaction/advisory lock 및 임시 schema-owner membership 회수를 포함한다. 기존 column을 삭제하거나 JSON 기록을 upgrade하지 않는다. 새로운 schema 변경이나 새 migration은 없다.

`migrate-db.sh TARGET`의 실제 순서:

1. `server/db/admin/preflight.sql`: database가 `deck_chess`인지, 관리자 login인지, split bootstrap이 존재하는지 검사.
2. `server/db/shared/20260826000000_profile_visibility.sql`.
3. `server/db/TARGET/20260826000500_create_game_records.sql`.
4. `server/db/TARGET/20260826001000_game_record_ownership.sql`.
5. `server/db/TARGET/20260827000000_challenge_clears.sql`.
6. `server/db/TARGET/20260904000000_analysis_retention.sql`.
7. `server/db/TARGET/20260908000000_account_decks.sql`.
8. `server/db/TARGET/20260910000000_analysis_draws.sql`.
9. `server/db/admin/verify_TARGET_contract.sql`.
10. `--verify-isolation` 지정 시 `verify_environment_isolation.sql`의 rollback-only probe.

이 release SQL은 SQLx checksum migration history가 아니다. `server/migrations`는 보존된 옛 public-schema 이력이며 split DB에 `sqlx migrate run`을 실행하면 안 된다. 번호가 큰 파일 하나만 임의 실행하는 대신 canonical runner의 preflight와 postflight까지 사용한다. runner 전체가 단일 transaction인 것은 아니므로 중간 실패 시 이미 성공한 파일이 남을 수 있다. 원인 해결 후 같은 runner 재실행으로 진행하며 down migration을 하지 않는다.

실제 첫 disposable cluster에서는 2~7을 test→prod에 먼저 적용한 뒤 migration 전 새 서버를 실행했다. 이후 아래 명령을 **test→prod 순서로 3회** 실행했다. 1·2회는 초기/멱등성, 3회는 데이터가 저장된 후 보존 검사였다.

```sh
# 아래 주소는 이번 검증의 disposable loopback 주소일 뿐 운영 주소가 아니다.
export ADMIN_DATABASE_URL=postgresql://postgres@127.0.0.1:55439/deck_chess
bash migrate-db.sh test --verify-isolation
bash migrate-db.sh prod --verify-isolation  # disposable 대상임을 확인하고 prod 입력
```

두 번째 빈 cluster에서는 추가한 fixture 파일을 처음부터 실행하고 test→prod 전체 runner를 각각 1회 통과시켰다. test 첫 실행 때는 아직 prod 테이블이 없으므로 isolation probe를 prod migration 이후 실행했다. 기존 split 운영 환경에서는 양 환경 bootstrap/release 상태를 먼저 확인한다.

## 3. Disposable 환경과 재현

- PostgreSQL `16.15 (Debian 16.15-1.pgdg13+2)`, Docker `postgres:16`, database `deck_chess`.
- 첫 컨테이너 `g9-postgres-20260910`, loopback `127.0.0.1:55439`.
- fixture 재현 컨테이너 `g9-fixture-check-20260910`, loopback `127.0.0.1:55440`.
- 운영 volume/credential/network DB를 mount하거나 사용하지 않았다. trust 인증은 이 disposable 검증 전용이다. 검증 후 이번에 만든 DB 2개 및 구 server 컨테이너와 해당 익명 volume을 정리했고 로그는 보존했다.
- 첫 준비 시 확장 fixture의 GRANT 실행 role 및 이미 생성된 index를 중복 실행하는 준비 오류가 있었다. disposable 준비 SQL만 바로잡았으며 release 파일은 수정하지 않았다. 최종 저장한 fixture는 두 번째 빈 cluster에서 성공했다.

재현 명령(새 disposable 환경에서만 실행; 기존 DB에는 금지):

```sh
docker run -d --name g9-pg-repro \
  -e POSTGRES_HOST_AUTH_METHOD=trust \
  -p 127.0.0.1:55440:5432 postgres:16
# docker exec g9-pg-repro pg_isready -U postgres 성공 후 진행
psql -X postgresql://postgres@127.0.0.1:55440/postgres \
  -v ON_ERROR_STOP=1 -f server/db/testing/g9_deployment_fixture.sql
psql -X postgresql://postgres@127.0.0.1:55440/template1 \
  -v ON_ERROR_STOP=1 -c 'ALTER DATABASE postgres RENAME TO deck_chess'
export ADMIN_DATABASE_URL=postgresql://postgres@127.0.0.1:55440/deck_chess
bash migrate-db.sh test
bash migrate-db.sh prod --verify-isolation
# 여기까지 성공 후 test/prod runner를 재실행해 idempotency 검증
psql -X "$ADMIN_DATABASE_URL" -v ON_ERROR_STOP=1 \
  -f server/db/testing/verify_account_decks.sql

export TEST_DATABASE_URL="$ADMIN_DATABASE_URL"
export TEST_ADMIN_DATABASE_URL="$ADMIN_DATABASE_URL"
export TEST_DECK_DATABASE_URL="$ADMIN_DATABASE_URL"
export TEST_ANALYSIS_DATABASE_URL="$ADMIN_DATABASE_URL"
export TEST_GAME_RECORD_DATABASE_URL="$ADMIN_DATABASE_URL"
export TEST_PROD_DATABASE_URL=postgresql://deck_chess@127.0.0.1:55440/deck_chess
export TEST_APP_DATABASE_URL=postgresql://deck_chess_test@127.0.0.1:55440/deck_chess
cargo test --offline -p brainfuck-chess-server postgres_ -- --ignored --test-threads=1 --nocapture
```

권한 변경 검증이 다른 테스트와 충돌하지 않도록 serial test runner를 쓴다. 개별 테스트 내부의 동시 create/append/update는 실제로 병렬 실행된다. 일부 repository 테스트가 양 환경의 fixture를 만들기 때문에 admin URL을 요구한다. 별도로 실제 `deck_chess_test` URL로 `postgres_g9_draws_roundtrip_serial_create_and_concurrent_append`를 실행해 runtime DML도 통과했다.

`verify_ownership_migrations.sql`은 pre-retention DELETE 권한을 전제로 하는 옛 fixture 검증이므로 현재 전체 migration의 postflight로 사용하지 않았다. 현재 prod/test contract, environment isolation 및 account-deck SQL probe를 사용했다.

## 4. 실제 DB integration 결과

최종 명령: `cargo test --offline -p brainfuck-chess-server postgres_ -- --ignored --test-threads=1 --nocapture`.

**11개 중 10 passed / 1 failed**, exit 101. 처음 실행도 기존 G3-C 테스트가 같은 지점에서 실패했다.

| 테스트/검사 | 결과 |
| --- | --- |
| `postgres_analysis_draws_roundtrip_and_concurrent_idempotency` | **실패: concurrent create** |
| `postgres_g9_draws_roundtrip_serial_create_and_concurrent_append` | 통과: SQL Draw, sequential create retry, concurrent/idempotent append, fresh repository reload/검증 |
| 위 분리 테스트를 `deck_chess_test` role로 실행 | 통과 |
| `postgres_persistence_concurrent_import_ownership_and_environment_isolation` | 통과: Legacy account deck CRUD/import/version conflict/ownership/prod-test 분리; 최대 JSONB text 109272 bytes |
| `postgres_g9_formats_and_challenge_clear_survive_reconnection` | 통과: 양 schema의 Legacy/Standard deck 및 record, 기존 clear timestamp, 개발단계 Standard unsupported |
| `postgres_g9_completed_challenge_persists_clear` | 통과: Legacy 및 test-only Standard Challenge 실제 완료→persist→SQL clear, 중복 persist 및 재연결 |
| `postgres_contract_rejects_opposite_schema_usage` | 통과: 정상 contract, deck UPDATE 권한 누락 거부, 반대 schema USAGE 부여 시 거부, 복구 후 정상 |
| `postgres_identity_and_profile_are_shared_between_prod_and_test` | 통과: 동일 identity가 동일 계정에 연결, 닉네임/profile 공유 |
| `postgres_guest_import_moves_only_the_current_environment` | 통과 |
| `postgres_repository_inserts_upserts_gets_and_lists_by_internal_user` | 통과: Legacy GameRecord 양 환경 저장/조회/목록 |
| `postgres_repository_survives_reconnection` | 통과: custom piece 저장 재연결 |
| `postgres_prod_and_test_images_are_isolated_with_the_same_owner` | 통과 |
| `verify_account_decks.sql` | 통과: rolled-back runtime CRUD/FK/isolation |

### 배포 차단 결함 G9-DB-01

기존 테스트가 `repository.create(tree.clone(), same_request_id, ...)`를 동시에 호출하면 하나가 `Err("unavailable")`를 반환한다. PostgreSQL 로그는 다음을 기록했다.

```text
ERROR: duplicate key value violates unique constraint "game_analysis_trees_pkey"
INSERT INTO test.game_analysis_trees (...)
VALUES (...) ON CONFLICT (owner_user_id,request_id) DO NOTHING
```

위치는 `server/src/analysis.rs`의 `PostgresAnalysisRepository::create` 및 기존 G3-C opt-in 테스트다. 동일 tree ID/동일 request ID 동시 삽입에서 primary key 충돌이 발생하며 지정된 conflict target으로 처리되지 않는 경로가 관측됐다. 순차 재시도와 동시 append 통과가 동시 create 성공을 대신하지 않는다. HTTP 재시도마다 tree ID가 달라지는 경우 전체 영향 범위는 별도 조사 대상이며, 실패한 repository 계약 자체는 배포 gate다.

기존 실패 테스트를 제거·직렬화·약화하지 않았다. 분리된 새 테스트는 실패 이후 도달하지 못한 SQL roundtrip/append를 추가로 검사할 뿐이다. 이번 G9는 validation 범위이므로 SQL 구현 수정은 하지 않았다. 수정 후 기존 동시 create 테스트와 전체 DB suite가 안정적으로 통과하기 전에는 GO로 바꿀 수 없다.

### 최초 수정 승인 요청 당시의 제안 (현재 적용 결과는 §13)

현재 코드와 실패 로그를 재확인했으며 G9-DB-01은 해결되지 않았다. 검증 전용 범위를 넘어 production repository를 수정해야 하므로 별도 범위 확인이 필요하다.

검토 가능한 수정안은 `PostgresAnalysisRepository::create`의 transaction 시작 직후 owner/request ID 조합으로 transaction-scoped PostgreSQL advisory lock을 획득하여 동일 멱등 요청의 concurrent insert를 순서대로 처리하는 것이다. 기존 conflict target, ownership, 반환된 저장 tree 및 Draw resolution 경로를 유지하고, lock 대기 후 중복 요청은 기존 tree를 읽도록 한다. 서로 다른 request ID는 독립적으로 처리하며, 복합 키는 모호하지 않게 인코딩해야 한다. DB migration이나 gameplay/Draw 규칙의 변경은 필요하지 않다.

이 수정안은 아직 구현·검증되지 않았다. 승인 후에는 동일 ID 동시 요청, 서로 다른 tree ID의 동일 request 재시도, 다른 owner/request의 분리 및 Draw 재실행 방지를 확인하고 기존 실패 테스트·전체 PostgreSQL suite·서버 회귀/build/check를 재실행해야 한다. 단순히 모든 unique conflict를 무시하거나 기존 동시 테스트를 직렬화하는 것으로 성공 처리하지 않는다.

## 5. Startup와 schema isolation

실제 새 binary `target/debug/brainfuck-chess-server`를 각각 `APP_ENV=test/prod` 및 해당 runtime DB role로 실행했다. dummy signing key와 dummy Identity Platform project는 외부 로그인 호출 없이 startup 구성을 충족하기 위한 disposable 값이다.

| 상태 | test | prod |
| --- | --- | --- |
| G3-C 전, 그 이전 release 적용 | exit 101: `server startup blocked: analysis Draw storage is not provisioned; run the approved admin migration` | 동일 |
| G3-C 후 | 정상 시작, `/api/health` → `{"status":"ok"}`, `/api/challenges` 정상 | 동일 |
| `draws` 확인 | jsonb / NOT NULL / default `[]` | 동일 |

권한 조회 결과:

| login | shared USAGE | prod USAGE | test USAGE |
| --- | --- | --- | --- |
| deck_chess | true | true | false |
| deck_chess_test | true | false | true |

관리자 `postgres`의 direct role membership은 0개였다. release의 temporary membership이 남지 않았다. current contract와 rollback-only environment probe는 두 환경의 DML/FK 및 반대 환경 접근 거부를 통과했다. `shared`는 기존 계정 공유 경계이며 test/prod별 계정을 새로 분리하지 않는다.

단, disposable admin은 PostgreSQL superuser다. 실제 Cloud SQL admin과 동일한 managed-service 권한 모델을 완전히 재현한 것은 아니다. 배포 시 해당 실제 관리자로 read-only preflight/postflight를 재확인해야 한다. startup health는 DB 전체 내용이나 로그인 성공까지 보증하지 않는다.

## 6. 기존 데이터 및 Standard 호환성

- Legacy account deck: ruleset/Extra 없는 구형 JSON 입력으로 저장 및 새 repository에서 읽기 성공. 기존 대규모 CRUD/import 회귀도 성공. 최소 deck fixture는 편집 가능한 draft이며 그 자체로 게임 시작 유효성을 주장하지 않는다.
- Legacy GameRecord: 기존 SQL repository fixture 및 G9 최소 record의 읽기/version gate/state 조회 성공. 기존 엔진/Replay golden 회귀는 기본 suite에서 통과했다.
- 기존 Challenge clear: `raining_men`, 첫 timestamp 123을 다시 999로 저장해도 한 행/123 유지. 재연결 읽기 성공.
- 기존 auth/user: 실제 account SQL repository의 동일 identity→동일 user, 닉네임·profile 공유 및 guest import 통과. 외부 Google token 검증과 실제 사용자 로그인은 미실행이다.
- Standard account deck: 명시 ruleset 및 중복 Extra 두 인스턴스가 JSONB 저장/재연결 후 동일하다.
- Standard completed record: `deck-chess-standard-1`, initial Draw, state-at-ply 재연결 결과 동일. G3-C 별도 SQL analysis 테스트가 실제 Draw를 포함한 node 저장/reload/hash 검증을 수행한다.
- 개발단계 Standard record: `deck-chess-1` 버전의 Standard fixture를 읽을 수 있어도 `validate_rules_version()`은 `unsupported_development_standard_record`를 반환한다. 강제 upgrade하지 않는다. 해당 fixture는 오직 disposable 테스트에서 만든 값이다.
- G8 SQL clear: 등록 사용자 context와 서버 판정 종료는 test fixture로 주입한다. 실제 production 완료/persist 함수를 통과하며 새 공개 Challenge 등록은 없다. 이 테스트의 record 저장은 memory repository, clear 저장은 PostgreSQL이다. record SQL roundtrip은 별도 테스트로 검증했다.
- 데이터가 저장된 첫 DB에 전체 release를 재적용했고 shared.users/auth_identities, prod/test decks/game_records/challenge_clears 전체 행의 정렬된 JSON 집계 MD5가 전후 동일했다. 계정·덱·기록·clear 변환을 수행하지 않았다.

이는 synthetic fixture 및 로컬 회귀 근거다. 실제 운영 백업을 복원하여 모든 과거 row를 검증한 결과는 아니며, “운영 데이터 전부 호환”으로 확대 해석하지 않는다.

## 7. Frontend/backend pairing 및 rollback 호환

현재 Dockerfile은 frontend dist와 server binary를 **한 이미지**에 넣는다. 동일 검토 release의 한 image/revision으로 배포·rollback하는 경로를 권장한다. 브라우저에 이미 로드된 구형 JS와 진행 중인 게임은 image 전환만으로 동시에 바뀌지 않는다.

| 조합 | 위험 / 근거 |
| --- | --- |
| 구 frontend + 새 server | 구 codec은 DC4를 지원하지 않는다. Standard Hand/Extra/Draw/Replay 표시·조작을 신뢰할 수 없다. 신 Challenge summary의 추가 ruleset을 구 UI가 충분히 검증하지 못한다. 구 Legacy Challenge의 map/size 누락 요청은 새 서버가 명시적으로 호환 처리하나 Standard 전체 안전을 보장하지 않는다. |
| 신 frontend + 구 server | 구 ChallengeSummary에는 ruleset이 없다. 신 UI의 `parseDeckRuleset(deck.ruleset) !== challenge.ruleset`에서 Legacy까지 선택 불가가 될 수 있다. 신 map_id/board_size 제출과 구 strict PlayerDeckSpec, Standard Extra JSONB/semantic record version도 계약 불일치 위험이 있다. |
| 신 frontend + 신 server | 권장 조합. 여전히 이번 G3-C 결함 수정 및 DB gate 통과가 선행되어야 한다. |

코드 비교 기준은 작업 트리와 HEAD `589a00a9...`의 codec/Challenge/API/storage 계약이다. 모든 과거 frontend/server 조합을 실제 browser로 시험한 것은 아니다.

### 구버전 binary의 additive schema 확인

로컬 보유 이미지 `europe-west1-docker.pkg.dev/var-chess-bfc/cloud-run-source-deploy/brainfuck_chess/brainfuck-chess:test-7f17f9b-20260908-201319`를 disposable 새 schema에 `APP_ENV=test`, `deck_chess_test`로 연결했다. image ID:

```text
sha256:35a21d175cb28071cd90ee8e38365a92adc765cb9262e05dd94f560bf93150cd
```

실제 server startup 및 `/api/health` 200을 확인했다. 이는 해당 **로컬 test 이미지**의 시작 호환 증거이며 현재 production revision이라는 의미가 아니다. 운영 rollback 대상 image digest와 전체 Legacy HTTP read/write는 아직 검증하지 않았다.

- additive `draws` 컬럼을 남겨 두어도 위 구 binary는 시작한다. 실제 test runtime role의 rollback-only SQL에서 구 INSERT처럼 draws를 생략하면 `[]`가 저장되고, `{}` 입력은 CHECK로 거부되는 것을 확인했다.
- 구 server는 새 Standard records의 semantic version/Hand/Extra/Draw를 올바르게 지원한다고 볼 수 없다. replay/analysis 접근 또는 deck 편집 실패가 생길 수 있다. 특히 구 strict DeckData는 새 Extra 필드와 맞지 않는다.
- DC4를 DC3로 바꾸거나 Extra를 삭제하는 rollback을 하지 않는다. 구 frontend에서는 DC4 import/export가 제한된다.
- 기존 analysis draws JSONB는 보존한다. 구 분석 경로가 Draw 의미를 이해하지 못하므로 새 Standard analysis의 조회/재계산/편집을 허용해서는 안 된다. 컬럼 보존과 기능 호환은 다르다.
- 서버의 active game store는 memory이므로 revision 전환으로 진행 중인 게임이 계속된다고 보장할 수 없다. 배포/rollback 전에 신규 대국 유입을 제한하고 기존 대국 종료를 기다리는 운영 절차를 준비한다.

## 8. 전체 자동 회귀

| 실제 실행 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` (동일 script인 `npm run test` 실행 후 정확한 요청 명령도 재실행) | 23 test files passed, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; script는 기존 `vue-tsc --noEmit`이며 별도 ESLint 아님 |
| `npm run build --prefix frontend` | 통과; 500 kB 초과 chunk warning |
| `cargo test --offline -p brainfuck-chess-engine` | 228 passed / 8 ignored / 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 최종 154 passed / 12 ignored / 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| `git diff --check` | 통과 |
| 전체 `postgres_ --ignored --test-threads=1` | **10 passed / 1 failed** |

server 기본 suite의 ignored 12개에는 PostgreSQL 11개와 payload diagnostic 1개가 있다. DB 11개는 별도로 실행했으므로 기본 suite의 ignored를 성공으로 계산하지 않는다. 엔진 ignored 성능 테스트와 payload diagnostic은 G9에서 실행하지 않았다. 기존 unused constructor warning 2개가 남는다.

로그는 `/tmp/g9-validation/`에 보관했다: `frontend-*.log`, `cargo-*.log`, `server-final.log`, `check-final.log`, `build-final.log`, `integration.log`, `integration-final.log`, `runtime-draws.log`, `migrate-{test,prod}-{1,2,3}.log`, `start-{before,after}-{test,prod}.log`, `schema-version.txt`, `snapshot-{before,after}.txt`, `fixture-clean*.log`, `old-insert.log`, `old-server.log`, `postgres.log`. /tmp는 영구 CI artifact가 아니므로 release 승인 시 로그를 별도 보존해야 한다.

## 9. 사람이 따라 할 배포 순서 — 현재 실행 금지, NO-GO 해제 후

1. **gate 해제:** G9-DB-01 수정의 검토와 기존 동시 create 재현 테스트/전체 SQL suite 통과를 확보한다. 이 문서의 NO-GO를 근거 없이 해제하지 않는다.
2. release commit을 확정한다. 현재 작업 트리는 dirty이므로 `release.sh`가 거부한다. 검토된 G1~G9 파일을 포함한 clean release에서 revision/image digest를 기록한다. prod는 main, test는 develop 계약을 따른다.
3. 현재 production의 실제 revision/image digest, traffic 배분, runtime env/schema/role, 사용 중인 frontend asset 버전을 기록한다. 이 문서의 로컬 구 이미지를 production revision으로 대체하지 않는다. rollback 대상 digest를 보관한다.
4. 현재 Cloud SQL backup을 생성하고 성공 상태·backup ID·복구 절차를 기록한다. shared 계정도 같은 DB에 있으므로 prod 테이블만의 백업으로 간주하지 않는다. 기존 운영 데이터의 숫자/보존 정책도 확인한다.
5. test 관리자 연결을 확인하고 `bash migrate-db.sh test --verify-isolation`을 실행한다. URL에 암호를 넣지 않고 승인된 password mechanism을 사용한다. test/prod는 shared를 공유하므로 test migration도 shared release를 실행한다는 점을 확인한다.
6. test postflight 성공 후 동일 release의 test image를 배포한다. startup 로그와 `/api/health`, Challenge summary ruleset/map/size, 아래 smoke를 확인한다. canonical `./release.sh test --verify-isolation`는 migration 후 배포를 함께 수행하며 migration을 다시 실행해도 이번 검증에서는 멱등했다.
7. prod 백업 상태와 관리자 연결의 DB/instance를 재확인한다. 단계별 실행이면 `bash migrate-db.sh prod --verify-isolation`을 실행하고 대상 확인 후 `prod`를 직접 입력한다. runner의 고정 안내 문자열만으로 실제 연결 대상을 판단하지 않는다.
8. `verify_prod_contract.sql`, 필요 시 `verify_test_contract.sql`, migration 로그와 임시 role 회수, draws type/null/default/array constraint를 확인한다. 오류가 있으면 배포하지 않는다.
9. 서버와 frontend가 함께 든 동일 release image를 prod에 배포한다. `./release.sh prod --verify-isolation`는 clean main에서 backup→migration→`reinstall-image.sh`를 수행하는 canonical 통합 경로이며 `release-prod` 확인을 받는다. 수동 migration과 통합 경로 중 어느 경로를 사용했는지 기록한다. G9에서는 이 운영 명령을 실행하지 않았다.
10. 새 revision startup 로그, DB role, APP_ENV=prod, health, frontend dist/config와 API 계약을 확인한다. 실제 로그인 smoke 전 health만으로 성공 판정하지 않는다.
11. 불가피하게 frontend/server를 나눠 전환해야 하면 사용자 접근을 통제한 창에서 **migration→server→frontend** 순서로 전환하고, 둘 다 맞기 전 Standard/Challenge를 공개하지 않는다. 별도 기능 flag를 이번 변경에서 추가한 것은 아니므로 사용 가능한 운영 접근 통제로 해결한다. 열린 구 탭은 새 release reload가 필요하다.
12. 아래 Legacy→Account→Standard smoke를 시행하고 결과/담당자/시간/revision을 기록한다. 기존 deck/record를 삭제하거나 재저장해서 호환성을 확인하지 않는다. 쓰기 smoke는 별도 검증 계정의 새 항목으로 한다.
13. startup 실패, 기존 데이터 조회 회귀, 잘못된 ruleset, reserve 노출, replay/hash 오류, analysis 중복/실패 등이 보이면 트래픽 확대를 멈추고 아래 rollback 판단을 따른다.

## 10. 배포 후 smoke checklist

아래 항목은 **운영 배포 후 수행할 미체크 항목**이다. 이번 로컬 테스트 결과를 운영 smoke 완료 표시로 바꾸지 않는다.

### Legacy

- [ ] 기존 account deck의 이름/맵/기물/Pocket 조회가 유지된다.
- [ ] 기존 `tempest_horde`, `raining_men`, `tempest_set` 카드·맵·clear가 정상이며 기존 Challenge를 시작할 수 있다.
- [ ] Legacy Pocket Drop 및 Bot 응수가 정상이며 Standard Hand UI/Draw가 잘못 적용되지 않는다.
- [ ] 배포 이전 Legacy Replay를 열어 초기 상태와 마지막 action이 정상 재생된다.

### Account

- [ ] 실제 로그인/세션 재접속이 성공한다.
- [ ] 기존 닉네임/public ID/profile가 유지된다.
- [ ] 기존 저장 덱 목록과 개별 조회가 정상이다.
- [ ] 기존 Challenge clear가 유지되고 새 정상 승리만 clear로 저장된다. 상대 기권 위조는 거부된다.

### Standard

- [ ] 새 검증 덱의 Starting/Pocket/중복 Extra를 저장하고 새 세션에서 다시 읽는다.
- [ ] DC4 export→import에서 ruleset/map/Extra가 동일하다. 기존 DC1~DC3는 Legacy 의미를 유지한다.
- [ ] 같은 ruleset/map의 게임 생성이 성공하고 불일치 조합은 거부된다.
- [ ] 충분한 Pocket으로 initial Hand White 4 / Black 3을 확인한다.
- [ ] Hand Drop이 Hand의 해당 인스턴스만 소비한다.
- [ ] 양측 Extra 표시가 공개되며 중복 인스턴스를 잃지 않는다.
- [ ] 유효 ExtraSummon의 제물/대상/Extra 소비 및 무효 요청의 상태 보존을 확인한다.
- [ ] 실제 턴 전환 Draw가 1회이며 상대 Hand/Pocket identity가 노출되지 않는다.
- [ ] Bot 턴의 Hand Drop/ExtraSummon/Draw와 난이도 응답을 확인한다.
- [ ] 완료 Replay에서 양측 초기 Draw와 action별 Draw/소환을 정확히 재생한다.
- [ ] Analysis create 재시도/동시성, append 재시도, reload 후 동일 state/Draw를 확인한다. G9-DB-01 해제 근거와 연결한다.
- [ ] 개발단계 Standard record는 unsupported 안내를 유지하며 자동 변환되지 않는다.

공개 Standard Challenge는 이번 release 범위에 없다. SQL/자동 fixture 검증을 위해 새 Challenge를 운영 registry에 추가하지 않는다.

## 11. Rollback 순서

1. 장애 범위·revision·오류를 기록하고 신규 Standard 생성 및 분석 쓰기 유입을 운영 접근 통제로 제한한다. 진행 중인 memory 게임 유실 가능성을 알리고, 가능한 경우 대국 종료를 기다린다.
2. DB를 되돌리기 전에 문제를 frontend/server 계약과 구분한다. backup은 보존하되 additive migration 때문에 데이터 삭제/down migration/백업 덮어쓰기를 하지 않는다.
3. 배포 직전에 기록한 **검증된 이전 revision/image digest**로 서버와 포함된 frontend를 함께 되돌린다. `latest` tag에 의존하지 않는다. Cloud Run 기존 revision으로 traffic을 돌리는 절차는 담당 운영자의 승인된 방식으로 실행하고, 설정/secret/DB role이 맞는지 확인한다.
4. 별도 frontend 배포였다면 접근 통제를 유지한 채 구 server와 구 frontend 쌍을 복원하고 캐시/열린 탭 reload를 확인한다. 중간 조합을 사용자에게 노출하지 않는다.
5. additive `draws` column 및 새 Standard records/DC4 저장 덱/analysis draws는 그대로 보존한다. 구버전에서 지원하지 못하는 Standard 경로는 제한된 상태로 두고 신버전 복구 후 재검증한다. DB 내용을 구형으로 강제 변환하지 않는다.
6. 이전 revision의 startup/health, 실제 로그인, Legacy deck/Challenge/Pocket Drop/Replay, 기존 clear를 다시 확인한다. health만 통과하면 제한을 해제하지 않는다.
7. 새 Standard 데이터가 생성된 뒤라면 rollback은 완전 기능 복구가 아니다. 해당 데이터 접근 제한과 원본 보존을 기록하고, 수정 release로 roll-forward할 계획을 세운다.

## 12. 최초 G9의 미검증 사항과 당시 gate

| Gate | 판정 |
| --- | --- |
| Disposable prod/test migration, 재적용, 데이터 내용 보존 | PASS |
| 새 server migration 전 차단/후 시작 | PASS |
| shared/prod/test runtime contract/isolation | PASS, Cloud SQL 실제 admin은 별도 |
| Legacy fixture 및 기본 gameplay/Replay 회귀 | PASS |
| Standard deck/record/Draw/clear SQL roundtrip | PASS |
| G3-C concurrent/idempotent create | **FAIL — G9-DB-01** |
| 전체 자동 테스트 | **FAIL — 기본 suite 통과와 별개** |
| 배포/rollback/smoke 절차 | 작성 완료 |
| 실제 production revision/schema/backup, 실제 운영 데이터 읽기 | 미실행 |
| 외부 로그인, 운영/브라우저 post-deploy 전체 smoke | 미실행 |
| 실제 production rollback image의 Legacy/Standard 데이터 전체 동작 | 미검증; 로컬 구 test 이미지 startup만 확인 |

**최초 G9 최종 NO-GO:** 필수 동시 create 멱등성 실패로 G9 완료 조건은 미달이다. gameplay/migration을 임의 변경해 통과시키지 않았다. 다음 승인 검토에는 G9-DB-01 수정 및 재검증, 고정 release artifact, 실제 환경 backup/revision/postflight와 배포 후 smoke 결과가 필요하다.


## 13. G9-DB-01 수정 및 재검증 — 최신 결과

2026-09-10 KST, 별도 Goal `G9-DB-01 — PostgreSQL Analysis Concurrent Create Idempotency Fix`의 명시적 수정 범위에 따라 수행했다. **G9-DB-01 해결, DB readiness GO.** 최초 두 차례 실패와 NO-GO 기록은 위에 그대로 남겼다.

### 13.1 조사한 실제 경로와 root cause

| 조사 항목 | 현재 코드에서 확인한 내용 |
| --- | --- |
| transaction 시작 | `PostgresAnalysisRepository::create`의 `pool.begin()` |
| tree/node ID 생성 | `analysis::new_tree`가 각각 UUID v4를 발급; DB 호출 전 생성 |
| HTTP retry | `create_analysis_tree`가 매 호출 `new_tree`를 실행하므로 동일 request에 다른 tree ID가 올 수 있음 |
| request 검사 | HTTP에서 비어 있지 않음/최대 100 bytes 검사; repository의 실제 멱등 identity는 owner/request |
| 기존 RNG | 신규 tree INSERT의 rows_affected > 0일 때 `resolve_tree`→`resolve_node`→`draw::turn_start`→`random_index` |
| 기존 INSERT | tree 먼저, root node 다음, 한 transaction; tree INSERT conflict target은 owner/request unique |
| 제약 | tree `PRIMARY KEY(id)`와 `UNIQUE(owner_user_id,request_id)`가 별도 제약 |
| 기존 conflict 반환 | owner/request로 저장 ID를 찾고 해당 owner의 tree/node를 reload |
| Memory semantics | write lock 안에서 owner/request 중복을 먼저 확인하고, 없을 때만 resolve 후 저장; 이번에 변경하지 않음 |

Root cause는 같은 tree ID와 owner/request의 동시 INSERT에서 PK violation이 발생하는 경로를 owner/request 대상 `ON CONFLICT`만으로 처리하려 한 것이다. 동일 request가 서로 다른 tree ID를 가진 HTTP retry도 멱등 key를 기준으로 하나의 저장 결과에 수렴해야 한다. PK만 같은 다른 논리 요청을 같은 결과로 처리해서는 안 된다.

### 13.2 적용한 최소 수정

production 코드 변경은 `server/src/analysis.rs`에 한정했다.

```text
BEGIN
→ advisory transaction lock(schema, owner, request)
→ 같은 transaction 연결로 owner/request tree + nodes 재조회
  → 있으면 COMMIT, 저장된 결과 반환 (RNG 없음)
  → 없으면 tree INSERT로 ID/FK/제약을 먼저 확인
     → 신규 INSERT 성공 시에만 기존 Draw resolution 실행
     → node의 draws/state/hash INSERT
     → COMMIT, 신규 저장 결과 반환
```

사용한 잠금:

```sql
SELECT pg_advisory_xact_lock(hashtextextended($1, 0))
```

`$1`은 `serde_json::to_string`으로 만든 배열 `["analysis-create-v1", schema-qualified tree table, owner_user_id, request_id]`다. 문자열 단순 concat이 아니어서 필드 경계/따옴표/escape가 모호하지 않다. prod/test와 작업 namespace도 분리했다. 기존 deck/account repository의 PostgreSQL `hashtextextended`/transaction lock 방식을 재사용했고 새 hash 라이브러리를 추가하지 않았다. DB가 반환하는 signed bigint를 바로 advisory lock에 전달하므로 unsigned cast/endianness 차이가 없다.

64-bit hash는 수학적으로 충돌 불가능한 키가 아니다. 극히 드문 hash 충돌은 무관한 요청을 잠시 직렬화할 수 있지만, 실제 반환 identity는 항상 전체 owner/request로 재검사하므로 다른 tree를 반환하지 않는다. 모든 요청이나 한 owner 전체를 잠그는 전역 mutex는 없다. lock은 commit/rollback에 자동 해제된다.

기존 `ON CONFLICT (owner_user_id,request_id) DO NOTHING`은 유지했다. lock protocol에 참여하지 않는 이전 revision writer와의 owner/request 충돌에서도 exact-key reload를 수행한다. 다만 이전 writer의 동시 same-PK 경로까지 수정한 것은 아니므로 배포 시 서버 revision을 혼용한 상태에서 이 보장을 확대하지 않는다. 새 implementation을 사용하는 동시 요청들이 검증 대상이다.

다른 owner/request의 동일 tree ID는 `game_analysis_trees_pkey` 오류를 `conflict`로 반환한다. 기존 HTTP mapper의 409 경로를 사용한다. 다른 사용자의 tree를 반환하거나 모든 unique violation을 성공으로 바꾸지 않는다. node 오류/다른 DB 오류는 성공 처리하지 않는다.

조회 helper는 pool 또는 transaction executor를 받도록 최소 확장했다. 중복 결과를 같은 transaction 연결로 읽으므로 max_connections=1에서도 두 번째 연결을 기다리는 교착이 없다. 테이블/JSON decode/Analysis 결과 의미는 동일하다. Memory 구현과 append 구현의 본문은 변경하지 않았다.

### 13.3 RNG 및 실패 원자성

`with_draw_source`는 `#[cfg(test)]`로만 존재하는 PostgreSQL repository 테스트 hook이다. production build에는 주입 필드/메서드가 없고 기존 `draw::random_index`를 그대로 호출한다. RNG 알고리즘·Draw 규칙·HTTP 입력 계약을 변경하지 않았다.

신규 successful create의 root가 한 장을 Draw하는 fixture에서, 같은 key의 8개 concurrent task 전체 RNG 호출 합계가 **1회**다. 같은 key/different tree IDs의 8개 task도 **1회**다. 이후 sequential retry와 새 repository reload에서는 **0회 추가 호출**이다. 같은 key lock을 실제 다른 transaction이 보유 중일 때 대기 task가 RNG를 호출하지 않는 것도 검사했다.

실패한 미commit 시도가 이미 소비한 OS entropy 자체를 되돌리는 계약은 아니다. 이는 기존 G3-C §8과 동일하다. RNG 실패, Draw 후 node FK 실패, 첫 node INSERT 뒤 두 번째 node INSERT 실패에서 tree/node/DrawResolution이 DB에 남지 않는다. 정상 retry가 새 transaction에서 version 1로 생성됨을 확인했다. 이미 성공·저장된 node에 대한 retry는 재추첨하지 않는다.

### 13.4 PostgreSQL 검증과 요구사항 대응

새 disposable `g9-db01-postgres`, `127.0.0.1:55441`, PostgreSQL 16.15에서 기존 `g9_deployment_fixture.sql`을 실행하고 database를 `deck_chess`로 준비했다. 기존 `migrate-db.sh test`→`prod --verify-isolation`을 적용했다. 운영 DB URL/자격증명/volume은 사용하지 않았다.

명령:

```sh
# 모든 TEST_*_DATABASE_URL은 위 disposable DB만 가리킨다.
cargo test --offline -p brainfuck-chess-server postgres_ -- --ignored --test-threads=1 --nocapture
```

결과: **12/12 passed, 실패 0**. 기존 11개 전부 통과 + 신규 집중 테스트 1개다. 최초 수정 검증 후 같은 전체 suite를 **3회 추가 반복**, 합계 **4회 모두 12/12** 통과했다. 이전 실패를 감추기 위한 재시도가 아니라 동시성 수정의 안정성 검증이며 수정 후 실패는 없었다.

신규 `postgres_g9_db01_create_identity_rng_atomicity_and_lock_scope`는 아래 항목을 **prod/test 양 schema**에서 실제 repository/SQL로 검사한다.

| 요구 검증 | 근거 |
| --- | --- |
| sequential same request | 다른 UUID로 retry, 저장된 전체 결과 동일, RNG 증가 없음 |
| concurrent same owner/request/same ID | barrier로 동시에 시작하는 8개 task, 모두 성공, tree 1/node 1 |
| concurrent same owner/request/different IDs | 서로 다른 8개 UUID 중 최초 저장 tree 하나, 모두 같은 state/hash/Draw 반환 |
| same owner/different request | 서로 다른 tree 성공 |
| different owner/same request text | 서로 다른 owner/tree 성공 |
| non-idempotent PK collision | 같은 owner/다른 key 및 다른 owner 모두 conflict, RNG 0 추가, 다른 tree 반환 없음 |
| RNG/Draw 재사용 | 두 종류의 concurrent group 각각 RNG 1회, 저장 DrawResolution과 반환값 동일 |
| RNG 실패 | draw_failed, tree 없음 |
| SQL 실패/partial row | Draw 후 node FK 오류 및 여러 node 중 후속 오류의 전체 rollback |
| unrelated requests 독립 | 한 key lock을 보유하고 pg_locks에서 실제 waiter를 확인; 다른 request 및 다른 owner/same text가 lock 해제 전 완료 |
| reload | 새 max_connections=1 pool에서 동일 create 복구, RNG hook 호출 시 panic으로 탐지, 저장 결과 동일 |
| exact 분석 | 재조회된 모든 tree의 `validate_analysis_trees`/state hash 통과, version 1/node 1 유지 |

원래 `postgres_analysis_draws_roundtrip_and_concurrent_idempotency` 함수는 수정 전 사본과 **byte-identical**이며 정상 통과했다. concurrent append, Draw reload/hash assertion을 삭제·직렬화·sleep 처리하지 않았다. 추가 G9 분리 테스트 `postgres_g9_draws_roundtrip_serial_create_and_concurrent_append`도 정상 통과했다.

### 13.5 기본 회귀와 변경 경계

| 실제 재실행 명령 | 결과 |
| --- | --- |
| `cargo test --offline -p brainfuck-chess-server` | 154 passed, 13 ignored, 실패 0; DB 12개는 별도 실행 |
| `cargo test --offline -p brainfuck-chess-engine` | 228 passed, 8 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| `npm test --prefix frontend` | 23 test files passed, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 추가 확인, 기존 vue-tsc script |
| `npm run build --prefix frontend` | 통과 |
| `git diff --check` | 통과 |

기존 frontend chunk-size warning과 server unused constructor warning 2개는 유지된다. 추가 의존성/column/index/constraint/migration은 없다. prod/test draws release 파일의 SHA-256은 §2와 동일하다. 이번 수정 파일은 `server/src/analysis.rs`, 직접 테스트 `server/src/draw/g3c_tests.rs`, 이 문서 세 개뿐이다. G1~G8 gameplay/Draw algorithm/GameRecord/frontend/Bot/Challenge/migration은 변경하지 않았다.

근거 로그: `/tmp/g9-db01/setup.log`, `integration.log`, `integration-repeat-{1,2,3}.log`, `cargo-*.log`, `frontend-*.log`, migration SHA-256 확인 및 수정 전 소스 사본. 최초 G9 로그는 `/tmp/g9-validation/`에 그대로 보존한다. 검증용 DB 컨테이너와 해당 익명 volume은 로그 보존 후 정리했다. PostgreSQL 버전은 `postgres-version.log`, DB 로그는 `postgres.log`에 보존했다.

### 13.6 최신 gate 판정

**GO — G9-DB-01 및 로컬/disposable DB readiness 검증 통과.** 기존 실패 테스트, RNG 1회, same request/different UUID, 무관 요청 독립, PK 충돌 오인 방지, SQL 전체 suite와 기본 server/engine/frontend 회귀가 모두 통과했다. G9-DB-01 때문에 걸었던 NO-GO 조건은 해제한다.

실제 production revision/backup/Cloud SQL 권한 및 migration postflight, 외부 로그인, 배포 후 browser smoke, 실제 production rollback image 검증은 §9~§11 절차대로 배포 담당자가 별도로 확인해야 한다. 이것들은 이번 DB-01 수정 Goal에서 실행한 것으로 표시하지 않는다. 운영 배포, Cloud SQL migration, `release.sh` 실행은 **하지 않았다**.
