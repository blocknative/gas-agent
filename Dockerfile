FROM rust:slim-bookworm as builder

WORKDIR /app
COPY . .

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin gas-agent && \
    mv /app/target/release/gas-agent /usr/local/bin/gas-agent

FROM debian:bookworm as runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl3 ca-certificates && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd -o -g 1000 ubuntu \
    && useradd -o -m -d /app -u 1000 -g 1000 ubuntu

COPY --from=builder /usr/local/bin/gas-agent /usr/local/bin/gas-agent

USER ubuntu

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/gas-agent"]
