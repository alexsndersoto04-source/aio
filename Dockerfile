# Moon — API backend en Titan (Docker)
# ============================================================
# Titan se construye desde las fuentes de ESTE repo sin Rust: la semilla (selfhost/semilla/) compila el
# compilador, el compilador se compila a sí mismo (punto fijo) y construye la CLI `titan`.
# La etapa 2 comprueba el backend con `titan check` y lo compila con `titan compile` en el
# momento del build: si Moon no compilara con esas fuentes, la imagen NO se construye, y el
# contenedor arranca el ejecutable ya hecho (sin recompilar mientras el healthcheck espera).

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
# La CLI busca el runtime en <ejecutable>/native/: se copia junto a ella. Hace falta en el
# momento del build (para comprobar y compilar Moon); el ejecutable que produce NO la
# necesita para correr: es autónomo.
COPY --from=titan /src/selfhost/titan /app/titan/titan
COPY --from=titan /src/selfhost/native/ /app/titan/native/
COPY projects/moon/ ./projects/moon/
WORKDIR /app/projects/moon
RUN mkdir -p uploads uploads/avatars uploads/covers uploads/posts
# Compilar en el build y no en el arranque. Antes el CMD era `titan run src/main.titan`, que
# recompilaba en cada arranque: ≈1 min 40 s en frío, porque la caché del runtime no viaja en
# la imagen. Con el HEALTHCHECK de abajo (start-period=30s y 3 reintentos cada 30 s, unos
# 2 min de margen) eso quedaba al límite y el servicio podía marcarse como no saludable y
# reiniciarse. Ahora el arranque es inmediato y el healthcheck no compite con la compilación.
RUN /app/titan/titan check src/main.titan \
 && /app/titan/titan compile src/main.titan -o /app/moon
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1
CMD ["/app/moon"]
