ARG RUST_VERSION=1.89.0
FROM rust:${RUST_VERSION}-slim-bullseye AS build-base
WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    m4 \
    build-essential \
    libgmp-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY sse ./sse
COPY participant ./participant
COPY app ./app

# Build dependencies first (this will be cached if dependencies don't change)
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release --workspace

# SSE Service Build
FROM build-base AS build-sse
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release --bin sse && \
    cp ./target/release/sse /bin/sse-server

# Participant Service Build
FROM build-base AS build-participant
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release --bin participant && \
    cp ./target/release/participant /bin/participant-server

# App Service Build
FROM build-base AS build-app
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release --bin app && \
    cp ./target/release/app /bin/app-server

# Base runtime image
FROM debian:bullseye-slim AS runtime-base
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

# SSE Service Runtime
FROM runtime-base AS sse
COPY --from=build-sse /bin/sse-server /bin/
EXPOSE 8080
CMD ["/bin/sse-server"]

# Participant Service Runtime
FROM runtime-base AS participant
COPY --from=build-participant /bin/participant-server /bin/
EXPOSE 50051
CMD ["/bin/participant-server"]

# App Service Runtime
FROM runtime-base AS app
COPY --from=build-app /bin/app-server /bin/
EXPOSE 8000
CMD ["/bin/app-server"]
