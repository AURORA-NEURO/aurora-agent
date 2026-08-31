# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder
WORKDIR /src
COPY . .
# The repository pins net.offline=true in .cargo/config.toml; the build overrides it.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --config net.offline=false -p bioprism-mcp -p bioprism-cli \
    && mkdir -p /out \
    && cp target/release/bioprism-mcp target/release/bioprism /out/

FROM debian:bookworm-slim

LABEL io.modelcontextprotocol.server.name="io.github.MurariAmbati/aurora-agent" \
      org.opencontainers.image.title="AURORA Agent" \
      org.opencontainers.image.description="FIBER decision-context compiler: bioprism-mcp stdio MCP server (259 tools) and bioprism CLI. Fully local; no network access, no data collection." \
      org.opencontainers.image.source="https://github.com/AURORA-NEURO/aurora-agent" \
      org.opencontainers.image.url="https://aurora-neuro.github.io/aurora-agent/" \
      org.opencontainers.image.licenses="Apache-2.0"

COPY --from=builder /out/bioprism-mcp /usr/local/bin/bioprism-mcp
COPY --from=builder /out/bioprism /usr/local/bin/bioprism

# Default confined root: the same reference fixtures and schemas the .mcpb bundle
# stages (mcpb/build_mcpb.py ROOT_CONTENT), so the server works out of the box.
COPY --chown=10001:10001 fixtures/fiber-v0.1 /data/fixtures/fiber-v0.1
COPY --chown=10001:10001 fixtures/fiber-v0.3 /data/fixtures/fiber-v0.3
COPY --chown=10001:10001 fixtures/fiber-v0.4 /data/fixtures/fiber-v0.4
COPY --chown=10001:10001 fixtures/fiber-v0.5 /data/fixtures/fiber-v0.5
COPY --chown=10001:10001 fixtures/generated /data/fixtures/generated
COPY --chown=10001:10001 schemas /data/schemas

RUN useradd --create-home --uid 10001 aurora
USER aurora
WORKDIR /data

ENTRYPOINT ["bioprism-mcp", "--root", "/data"]
