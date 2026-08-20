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
runtime이 아니라 기존 table owner가 한 번 실행하는 관리자 migration이다.
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
