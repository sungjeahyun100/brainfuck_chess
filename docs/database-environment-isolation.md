# Deck Chess PostgreSQL 환경 격리

## 적용 전 구조

기존 SQLx migration은 `public`에 `users`, `auth_identities`,
`custom_piece_versions`, `custom_piece_images`를 만들었다. Google Identity
Platform의 `(issuer, subject)`는 `auth_identities.user_id`로 내부 `users.id`에
연결되며, nickname은 `users.display_name`이다. 이미지 원문은 PostgreSQL
`BYTEA`에 저장된다.

서버는 시작할 때 `server/migrations` 전체를 자동 실행했고 모든 query가
`search_path`로 `public`을 암묵적으로 사용했다. 따라서 같은 DB를 쓰는 test
배포도 production table migration과 data에 도달할 수 있었다.

## 최종 구조와 불변조건

```text
deck_chess database
├── shared.users
├── shared.auth_identities
├── prod.custom_piece_versions
├── prod.custom_piece_images
├── test.custom_piece_versions
└── test.custom_piece_images
```

- `APP_ENV=prod`는 `shared.*`와 `prod.*`만 명시적으로 query한다.
- `APP_ENV=test`와 `APP_ENV=local`은 `shared.*`와 `test.*`만 query한다.
- local의 안전한 기본값은 `test`이며 `search_path`로 환경을 선택하지 않는다.
- 동일 `(issuer, subject)`는 양쪽 환경에서 같은 `shared.users.id`를 반환한다.
- nickname/public ID는 `shared.users`에 한 번만 존재한다.
- 계정의 `profile_visibility`도 `shared.users`에 저장되어 prod/test에서
  동일하게 적용된다.
- 기존 production custom piece와 `BYTEA` image는 `prod`로 이동한다.
- `test` custom piece/image table은 빈 상태로 시작한다.
- 향후 server deck은 `prod.decks`와 `test.decks`로 각각 추가한다.
- rating/achievement는 향후 `shared`에 추가하되 `test_app`에 write 권한을
  부여하지 않는다. 현재 빈 기능/table은 만들지 않는다.

서버 시작은 선택한 schema의 네 필수 table을 확인한다. Cloud 환경에서는
현재 DB role이 반대 환경 schema에 `USAGE`를 가지면 fail-safe로 시작을
차단한다.

## 비파괴적 전환 절차

`server/db/admin/20260820010000_split_shared_prod_test.sql`은 애플리케이션
runtime이 아니라 Cloud SQL 관리자가 승인된 DB release에서 실행하는 migration이다.
새 Cloud SQL instance를 만들지 않는다.

1. Cloud SQL on-demand backup과 복구 가능 상태를 확인한다.
2. production write를 잠시 멈추거나 maintenance window를 연다. migration은
   짧은 `ACCESS EXCLUSIVE` schema move가 필요하다.
3. 아래 preflight를 table owner로 실행하고 결과를 change record에 보관한다.

   ```sql
   SELECT 'users', count(*) FROM public.users
   UNION ALL SELECT 'auth_identities', count(*) FROM public.auth_identities
   UNION ALL SELECT 'custom_piece_versions', count(*) FROM public.custom_piece_versions
   UNION ALL SELECT 'custom_piece_images', count(*) FROM public.custom_piece_images;
   ```

4. password를 command line에 넣지 않는 관리 환경에서 Cloud SQL `postgres`
   관리자로 실행한다. runtime user로 실행하면 test schema ownership이 생길 수
   있으므로 script가 이를 거부한다.

   ```bash
   psql "$ADMIN_DATABASE_URL" -v ON_ERROR_STOP=1 \
     -f server/db/admin/20260820010000_split_shared_prod_test.sql
   ```

   script는 advisory lock과 단일 transaction을 사용한다. 기존 table에
   `SET SCHEMA`를 적용하므로 table OID, PK, index, FK와 모든 row가 유지된다.
   이동 전후 row count가 다르거나 일부 table만 이동된 상태이면 전체를
   rollback한다. `DROP`, `TRUNCATE`, production row 복사는 수행하지 않는다.

   이어서 ownership 안정화 migration을 같은 관리 release에서 실행한다.

   ```bash
   psql "$ADMIN_DATABASE_URL" -v ON_ERROR_STOP=1 \
     -f server/db/admin/20260820020000_stabilize_schema_ownership.sql
   ```

   Cloud SQL built-in login은 `cloudsqlsuperuser` 멤버이므로 그 group role이
   격리 schema를 소유하면 prod/test login이 반대 schema `USAGE`를 상속한다.
   두 번째 migration은 runtime에 부여하지 않는
   `deck_chess_schema_owner NOLOGIN` role로 세 schema와 test table ownership을
   옮긴다. ownership 전환에 필요한 database `CREATE`는 같은 transaction
   안에서만 부여하고 즉시 회수한다.

5. script는 기존 production/test login에 아래 group role 하나씩을 연결한다.
   실제 password는 Secret Manager에서만 관리한다.

   ```sql
   GRANT prod_app TO deck_chess;
   GRANT test_app TO deck_chess_test;
   ```

   현재 Cloud SQL에는 `deck_chess`, `deck_chess_test` login이 이미 존재한다.
   production/test용 `DATABASE_URL` Secret은 이 서로 다른 username을 사용해야
   한다. 두 URL은 같은 instance와 같은 `deck_chess` database를 가리킨다.

6. table owner로 권한 probe를 실행한다. probe data는 transaction 끝에서
   rollback된다.

   ```bash
   psql "$ADMIN_DATABASE_URL" -v ON_ERROR_STOP=1 \
     -f server/db/admin/verify_environment_isolation.sql
   ```

7. production에는 `APP_ENV=prod`와 production DB Secret, test에는
   `APP_ENV=test`와 test DB Secret을 연결한 뒤 새 revision을 배포한다.
   두 배포 모두 `AUTH_SIGNING_KEY`가 필수다. 2026-08-20 점검 당시 test에는
   이 Secret mapping이 없었으므로 다음 배포 전에 추가해야 한다.
8. health/login/profile/custom-piece/image smoke test와 아래 postflight count를
   확인한 뒤 maintenance를 종료한다.

   ```sql
   SELECT 'users', count(*) FROM shared.users
   UNION ALL SELECT 'auth_identities', count(*) FROM shared.auth_identities
   UNION ALL SELECT 'custom_piece_versions', count(*) FROM prod.custom_piece_versions
   UNION ALL SELECT 'custom_piece_images', count(*) FROM prod.custom_piece_images
   UNION ALL SELECT 'test_custom_piece_versions', count(*) FROM test.custom_piece_versions
   UNION ALL SELECT 'test_custom_piece_images', count(*) FROM test.custom_piece_images;
   ```

test의 마지막 두 count는 최초 전환 직후 `0`이어야 한다. production의 앞 네
count는 preflight와 같아야 한다.

### 재실행 특성

이 migration은 완전히 적용된 상태에 한해 idempotent하다. 다시 실행하면 schema,
test table/index, role과 grant가 `IF NOT EXISTS` 또는 반복 가능한 GRANT/REVOKE로
확인되고 기존 row를 복사하거나 변경하지 않는다. `public`과 target에 application
table이 일부씩 존재하는 partial 상태는 명시적으로 거부한다. 재실행 가능하더라도
운영에서는 migration history를 우회하는 일반 복구 수단으로 사용하지 않는다.

### PostgreSQL 16 실행 검증

2026-08-21에 production과 연결되지 않은 tmpfs Docker `postgres:16` 환경
(PostgreSQL 16.15)에서 다음을 확인했다.

- repository의 기존 migration 3개로 실제 `public` 구조를 생성
- 사용자 2, identity 2, custom-piece version 3, BYTEA image 3 fixture 적용
- 관리자 migration 최초 실행과 동일 파일 재실행 모두 commit 성공
- 네 기존 table의 OID와 owner, constraint OID, index OID, column default 유지
- FK가 이동 후 동일한 `shared.users` table OID를 참조
- PK/unique/check constraint 유지, serial/identity sequence는 기존 구조에 없음
- display name, 내부 ID, owner ID, 모든 row와 image BYTEA hex가 동일
- test custom-piece/image row는 최초 전환 후 0
- 의도적 충돌 table로 index 생성 단계 실패를 유도했을 때 네 기존 table이 모두
  `public`으로 rollback되고 새 `shared`/`prod` schema도 남지 않음
- migration transaction 종료 후 advisory lock이 남지 않음
- 실제 `deck_chess`/`deck_chess_test` login으로 허용 경로 성공 및 반대 schema의
  SELECT/INSERT/DELETE가 `permission denied for schema`로 거부됨
- test role에 prod `USAGE`를 임시 부여하면 startup contract가 거부하고, REVOKE 후
  다시 성공함
- shared identity/nickname, prod/test 기물·BYTEA image, 양 환경 guest import 격리
  PostgreSQL integration test 5개 통과

검증 중 기존 persistence integration test가 합성 `shared.users` row를 cleanup하지
않는 문제를 발견해 수정했다. 운영 DB에는 이 검증을 실행하지 않았다.

## 권한 계약

`prod_app`:

- `shared`: account/auth 흐름에 필요한 select/insert와 제한된 update
- `prod`: 현재 custom-piece table read/write
- `test`: schema usage 없음

`test_app`:

- `shared`: select, 신규 Google login/guest 생성에 필요한 insert, nickname과
  login metadata에 필요한 column update; delete 없음
- `test`: 현재 custom-piece table read/write
- `prod`: schema usage 없음

현재 backend가 authentication과 game API를 한 process에서 제공하므로 test의
첫 Google login도 shared identity를 생성해야 한다. 이것이 test role에 일부
shared insert/update가 필요한 이유다. rating/achievement 같은 향후 공식 기록은
별도 table로 만들고 `test_app`에 권한을 추가하지 않는다. 인증 write까지 더
강하게 격리하려면 추후 security-definer auth API 또는 별도 auth service role을
도입해야 하며, 이번 변경에서는 기존 Google login 구조를 재작성하지 않았다.

## migration release 분리

- `server/db/shared`: 계정 단위의 특별 release. prod/test 배포와 독립 승인.
- `server/db/prod`: production game-data release.
- `server/db/test`: test game-data release. 이것만 test release에서 실행 가능.
- `server/db/admin`: bootstrap/권한처럼 table owner가 실행하는 작업.

2026-08-26 공개 설정 release는 관리자가 다음 순서로 적용한다.

1. backup과 preflight를 확인한 뒤
   `server/db/shared/20260826000000_profile_visibility.sql`을 적용한다.
2. production release는 `server/db/prod/20260826000500_create_game_records.sql`을
   적용한 뒤 `server/db/prod/20260826001000_game_record_ownership.sql`을 적용한다.
3. test release는 production 파일 대신
   `server/db/test/20260826000500_create_game_records.sql`과
   `server/db/test/20260826001000_game_record_ownership.sql`만 순서대로 적용한다.
4. `server/db/admin/verify_environment_isolation.sql`로 양쪽 runtime role의
   GameRecord INSERT/UPSERT/SELECT와 반대 schema 차단을 확인한다.
5. 새 application revision은 shared visibility column과 현재 환경의 ownership
   column을 startup contract로 확인하므로, 모든 해당 migration 완료 후 배포한다.

소유권 column은 현재 public ID와 유일하게 매칭되는 기존 기록만
backfill한다. 게스트나 매칭할 수 없는 이전 기록은 nullable로 남고,
제3자에게는 fail-closed로 공개되지 않는다.

신규 환경에서 `00500` migration은 ownership column, nullable FK와 user ID
조회 index를 최초 생성에 포함한다. 기존 환경에서는 먼저 table의
기본 column/PK 계약을 검증하고, 예상과 다른 partial schema는 덩어쓰지
않고 transaction을 실패시킨다. `01000` migration은 기존 row의 ownership만
보수적으로 backfill하며 DROP, TRUNCATE, 환경 간 복사를 하지 않는다.

`server/migrations/20260824000000_game_records.sql`은 pre-isolation SQLx history를
위해 checksum을 변경하지 않고 보존한다. application과 deployment는 이
디렉터리를 실행하지 않으며, 현재 release에서는 반드시 schema별
`server/db/prod` 또는 `server/db/test` migration을 사용한다.

애플리케이션 startup의 `sqlx::migrate!`는 제거했다. `cloudbuild.yaml`에도 DB
migration step이 없다. 따라서 test deployment만으로 `shared`나 `prod`가
변경되지 않는다. migration은 backup, 승인, pre/postflight가 있는 별도 작업이다.

## Cloud Run

두 서비스는 같은 Cloud SQL instance connection을 유지한다. build config는
`--min-instances=0`을 명시해 test trigger가 새 revision을 배포해도 idle instance
비용이 생기지 않게 한다. production/test trigger는 각각 실제 service region,
`APP_ENV`, DB Secret을 override해야 한다.

## rollback

script 실행 중 실패는 transaction rollback으로 원래 `public` 구조를 복원한다.
commit 후 code rollback이 필요하면 새 revision 대신 직전 revision으로 traffic을
되돌리기 전에 구버전이 `public` table을 요구한다는 점을 고려해야 한다. DB를
되돌려야 할 때는 maintenance 상태에서 검증된 backup restore 또는 별도 승인된
역방향 `SET SCHEMA` change를 사용한다. 애플리케이션이 자동으로 schema를 되돌리거나
Cloud SQL을 재시작하지 않는다.

긴급 역방향 전환은
`server/db/admin/rollback_20260820010000_split_shared_prod_test.sql`을 사용한다.
이 파일은 shared account와 prod game-data table을 기존 `public` 위치로 되돌리고
row count를 transaction 안에서 검증한다. `test.*` table과 그 안의 데이터는
삭제하거나 public/prod에 병합하지 않고 그대로 격리 보존한다. 역방향 전환 뒤에는
새 application이 schema contract 실패로 시작하지 않으므로 기존 revision으로만
traffic을 전환해야 한다. production/test table이 일부씩 이동된 partial 상태에서는
실행을 거부한다.

## 운영 반영 체크리스트

실제 운영 변경은 별도 승인 후 아래 순서로만 진행한다.

1. Cloud SQL backup 확인 또는 생성
2. production row count, FK와 주요 object baseline 기록
3. Cloud SQL 관리자로 schema split migration 실행
4. 같은 관리 release에서 schema ownership 안정화 migration 실행
5. `verify_environment_isolation.sql` 실행
6. migration 후 row count, FK, owner와 권한 확인
7. 새 application revision 배포
8. Google login smoke test
9. nickname 공유 확인
10. prod/test custom piece 격리 확인
11. prod/test image 격리 확인

test revision 배포 전 `AUTH_SIGNING_KEY` Secret mapping도 반드시 추가한다.
