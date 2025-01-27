# We use the latest Rust stable release as base image
# (기본 이미지로 최신 러스트 stable 릴리스를 사용한다.)
# Builder stage(Builder 단계) -> 이미지 크기 줄이는 방법
# lukemathewalker/cargo-chef:latest-rust-1.84.0 -> 도커 레이어 캐싱 (도커 파일 동작 순서 최적화 하는 방법)
FROM lukemathewalker/cargo-chef:latest-rust-1.84.0 as chef

# Let's switch out working directory to 'app' (equivalent to 'cd app')
# The 'app' folder will be created for us by Docker in case it does not exist already.
# (작업 디렉터리를 'app'으로 변경한다('cd app'과 동일)
# ('app' 폴더가 존재하지 않는 경우 도커가 해당 폴더를 생성한다.)
WORKDIR /app
# Install the required system dependencies for our linking configuration
# (구성을 연결하기 위해 필요한 시스템 디펜던시를 설치한다.)
RUN apt update && apt install lld clang -v
# 도커 레이어 캐싱 (추가된 시작 부분)
FROM chef as planner
COPY . .
# Compute a lock-like file for out project
# (프로젝트를 위한 lock 유사 파일을 계산한다.)
RUN cargo chef prepare -recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# Build out project dependecies, not our application
# (애플리케이션이 아닌 프로젝트 디펜던시를 빌드한다.)
RUN cargo chef cook --release --recipe-path recipe.json
# Up to this point, if out dependency tree stays the same,
# all layers should be cached.
# (이 지점까지 디펜던시 트리가 이전과 동일하게 유지되면, 모든 레이어는 캐시 되어야 한다.)
# 도커 레이어 캐싱 (추가된 마지막 부분)
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
# Runtime stage(Runtime 단계) - 이미지 크기 줄이는 방법
FROM debian:bullseye-slim AS runtime
WORKDIR /app
# Install OpenSSL - it is dynamically linked by same of our dependencies
# (OpenSSL을 설치한다. - 이부 디펜던시에 의해 동적으로 링크한다.)
# Install ca-certificates - it is needed to verify TLS certificates
# when establishing HTTPS connections
# (ca-certificates를 설치한다. - HTTPS 연결을 수립할 때 TSL 인증을 검증할 때 필요하다.)
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
# Copy the compiled binary from the builder environment.
# to our runtime environment
# (컴파일된 바이너리를 builder 환경에서 runtime 환경으로 복사한다.)
COPY --from=builder /app/target/release/zero2prod zero2prod
# We need the configuration file at runtime!
# (runtime에서의 구성 파일이 필요하다!)
COPY configuration configuration
# 0.0.0.0을 사용해서 애플리케이션이 로컬뿐만 아니라 모든 네트워크 인터페이스로부터의 커넷션을 받아들이도록 해야되어서 설정
ENV APP_ENVIRONMENT production
# When 'docker run' is executed, lanuch the binary!
# ('docker run'이 실행되면, 바이너리를 구동한다.)
ENTRYPOINT ["./target/release/zero2prod"]