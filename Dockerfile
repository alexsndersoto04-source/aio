# Moon — API backend en Titan (Docker)
# ============================================================
# Titan se construye desde las fuentes de ESTE repo sin Rust: la semilla (selfhost/semilla/) compila el
# compilador, el compilador se compila a sí mismo (punto fijo) y construye la CLI `titan`.
# La etapa 2 comprueba el backend con `titan check` en el momento del build: si Moon no compilara con
# esas fuentes, la imagen NO se construye.

# Etapa 1 — construir Titan (solo necesita bash y gzip)
FROM debian:bookworm-slim AS titan
RUN apt-get update && apt-get install -y --no-install-recommends bash gzip coreutils \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY selfhost/ ./selfhost/
RUN bash selfhost/bootstrap.sh

# Etapa 2 — imagen final
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# La CLI busca el runtime en <ejecutable>/native/: se copia junto a ella.
COPY --from=titan /src/selfhost/titan /app/titan/titan
COPY --from=titan /src/selfhost/native/ /app/titan/native/
COPY projects/moon/ ./projects/moon/
WORKDIR /app/projects/moon
RUN mkdir -p uploads uploads/avatars uploads/covers uploads/posts
RUN /app/titan/titan check src/main.titan
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1
CMD ["/app/titan/titan", "run", "src/main.titan"]
