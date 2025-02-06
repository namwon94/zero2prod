-- Add migration script here
-- We wrap the whale migration in a transaction to make sure it succeeds or fails atomically.
-- We will discuss SQL transactions in more details towards the end of this chapter!
-- 'sqlx' does not do it automatically for us.
-- 전체 마이레이션을 트랜잭션으로 감싸서 단일하게 성공 또는 실패가 되도록 한다.
BEGIN;
    -- Backfill 'status' for historical entries (과거 데이터에 대한 'status'를 채운다)
    UPDATE subscriptions
        SET status = 'confirmed'
    WHERE status IS NULL;
    -- Make 'status' mandatory 'status'를 필수 컬럼으로 설정한다.
    ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
COMMIT;