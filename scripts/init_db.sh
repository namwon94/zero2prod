#!/usr/bin/env bash
set -x
set -eo pipefail

#if ! [ -x "$(command -v  psql)"]; then
#   echo >&2 "Error: psql is not installed."
#   exit 1
#fi

#if ! [ -x "$(command -v sqlx)"]; then
#   echo >&2 "Error: sqlx is not installed."
#   echo >&2 "Use:"
#   echo >&2 " cargo install --version='~0.6' sqlx-cli --no-default-features --features rustls,postgres"
#   echo >&2 "to install it."
#   exit 1
#fi

# 커스텀 유저가 설정되었는지 확인한다. 기본값은 'postgres'이다
DB_USER=${POSTGRES_USER:=postgres}
# 커스텀 비밀번호가 설정되었는지 확인한다. 기본값은 'password'이다
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
# 커스텀 데이터베이스명이 설정되었는지 확인한ㄷ. 기본값은 'newsletter'이다
DB_NAME="${POSTGRES_DB:=newsletter}"
# 커스텀 포트가 설정되었는지 확인한다. 기본값은 '5432'이다
DB_PORT="${POSTGRES_PORT:=5432}"

# 도커를 사용해서 postgres를 구동한다
# 도커화된 postgres 데이터베이스가 이미 실행 중이면 도커가 이 단계를 건너뛸 수 있게 한다.
if [ -z "${SKIP_DOCKER}"]
then
   docker run \
      -e POSTGRES_USER=${DB_USER} \
      -e POSTGRES_PASSWORD=${DB_PASSWORD} \
      -e POSTGRES_DB=${DB_NAME} \
      -p "${DB_PORT}":5432 \
      -d postgres \
      postgres -N 1000
      # ^ 테스트 목적으로 최대로 증가시킨 컨넥션 수
fi
# Postgres가 명령어를 받아들일 준비가 될 때까지 핑을 유지한다.
export PGPASSWORD="${DB_PASSWORD}"
until psql -f "localhost" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q'; do
   >&2 echo "Postgres is still unavailable - sleeping"
   sleep 1
done

>&2 echo "postgres is up and running on port ${DB_PORT}!"

DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}:@localhost:${DB_PORT}/${DB_NAME}
export DBATABASE_URL
sqlx database create
sqlx migrate run

>&2 echo "Postgres has been migrated, ready to go!"