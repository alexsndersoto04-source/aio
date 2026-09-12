# Moon — API backend en Titan (Docker)
# ============================================================
# Compila TITAN desde las fuentes de ESTE repo (etapa 1) y arma una imagen
# mínima con el binario y el backend de Moon (etapa 2).
#
# Antes se descargaba un binario pre-compilado de la release v1.0.0. Se cambió
# a compilar desde las fuentes por dos razones:
#   1. Garantiza que el binario que se despliega es exactamente el que la CI
#      verifica (el E2E completo corre contra el `titan` recién compilado del
#      repo: `cargo test -p titan_cli --test moon_e2e`).
#   2. La release v1.0.0 es de una versión anterior del lenguaje y no está
#      garantizado que compile este Moon; la descarga además depende de que
#      GitHub sirva el binario correcto por arquitectura.
#
# La etapa 2 comprueba el backend con `zett check` en el momento del build:
# si Moon no compilara con esas fuentes, la imagen NO se construye (mejor un
# build roto y visible que un contenedor que arranca y muere).

# ------------------------------------------------------------
# Etapa 1 — compilar TITAN (rust:1 incluye gcc/make, necesarios para
# `ring` y para el SQLite embebido)
# ------------------------------------------------------------
FROM rust:1-bookworm AS titan

WORKDIR /src

# Manifiestos del workspace y de cada crate primero: así la capa de
# dependencias se reutiliza entre builds aunque cambie solo el código.
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Solo el CLI y lo que necesita (no el resto del workspace).
RUN cargo build --release -p titan_cli \
    && cp target/release/titan /usr/local/bin/zett \
    && /usr/local/bin/zett --version

# ------------------------------------------------------------
# Etapa 2 — imagen final
# ------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# El binario de Titan, compilado en la etapa anterior
COPY --from=titan /usr/local/bin/zett /app/zett

# Copiar el proyecto Moon (solo código fuente)
COPY projects/moon/ ./projects/moon/

WORKDIR /app/projects/moon

# Directorios de subida (los crea también el runtime al arrancar)
RUN mkdir -p uploads uploads/avatars uploads/covers uploads/posts

# Comprobación real en el build: si este Moon no compila con este Titan,
# la imagen no se construye y el despliegue se detiene aquí.
RUN /app/zett check src/main.titan

# Puerto HTTP
EXPOSE 3000

# Healthcheck real contra /api/health
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1

# Ejecutar Moon
CMD ["/app/zett", "run", "src/main.titan"]
