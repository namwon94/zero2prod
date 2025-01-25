# We use the latest Rust stable realease as base image
# (기본 이미지로 최신 러스트 stable 릴리스를 사용한다.)
FROM rust:1.84.0

# Let's switch out working directory to 'app' (equivalent to 'cd app')
# The 'app' folder will be created for us by Docker in case it does not exist already.
# (작업 디렉터리를 'app'으로 변경한다('cd app'과 동일)
# ('app' 폴더가 존재하지 않는 경우 도커가 해당 폴더를 생성한다.)
WORKDIR /app
# Install the required system dependencies for our linking configuration
# (구성을 연결하기 위해 필요한 시스템 디펜던시를 설치한다.)
RUN apt update && apt install lld clang -v
# Copy all files from out woking environment to out Docker image
# (작업 환경의 모든 파일을 도커 이미지로 복사한다.)
COPY . .
# cargo build를 실행하면 sqlx는 .env파일의 DATABASE_URL 환경변수가 가리키는 데이터베이스와 커넥션을 실패하므로 
# OFFLINE을 true로 설정하여 sqlx가 실제 데이터베이스에 쿼리를 시도하는 대신 저장된 메타데이터를 보게한다.
# Cargo.toml에 sqlx 설정에 offline 추가 및 cargo sqlx prepare -- --lib 명령어 입력 -> 해당 명령어는 쿼리의 결과를 메타데이터 파일(sqlx-data.json)에 저장한다.
ENV SQLX_OFFLINE true
# Let's build out binary!
# We'll use the release profile to make it faaaaast
# (바이너리를 빌드하자.)
# (빠르게 빌드하기 위해 release 프로파일을 사용한다.)
RUN cargo build --release
# 0.0.0.0을 사용해서 애플리케이션이 로컬뿐만 아니라 모든 네트워크 인터페이스로부터의 커넷션을 받아들이도록 해야되어서 설정
ENV APP_ENVIRONMENT production
# When 'docker run' is executed, lanuch the binary!
# ('docker run'이 실행되면, 바이너리를 구동한다.)
ENTRYPOINT ["./target/release/zero2prod"]