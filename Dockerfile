FROM rust:slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin gas-agent

FROM debian:bookworm-slim as runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl3 ca-certificates && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd -o -g 1000 ubuntu \
    && useradd -o -m -d /app -u 1000 -g 1000 ubuntu

COPY --from=builder /app/target/release/gas-agent /usr/local/bin

USER ubuntu

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/gas-agent", "start"]
