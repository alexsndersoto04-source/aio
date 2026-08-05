FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

# Native build dependencies used by Titan's optional runtime integrations.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev clang cmake \
    && rm -rf /var/lib/apt/lists/*
