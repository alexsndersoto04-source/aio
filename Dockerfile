# Moon — API backend en Titan (Docker)
# Usa el binario pre-compilado de Titan v1.0.0 (release oficial del repo).
# La descarga falla DE FORMA EVIDENTE (sin `|| true`): si no se puede
# obtener el binario, el build se detiene con un mensaje claro en vez de
# arrancar un contenedor roto.

FROM ubuntu:24.04

# Dependencias mínimas
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Descargar binario pre-compilado de Titan (v1.0.0)
ARG TITAN_VERSION=v1.0.0
RUN curl -fsSL --retry 3 --retry-delay 2 \
      https://github.com/alexsndersoto04-source/aio/releases/download/${TITAN_VERSION}/zett-linux-x86_64.tar.gz \
      -o /tmp/zett.tar.gz \
    && tar -xzf /tmp/zett.tar.gz -C /app \
    && chmod +x /app/zett \
    && rm -f /tmp/zett.tar.gz \
    && test -x /app/zett \
    && /app/zett --version \
    || { echo "FATAL: no se pudo obtener/verificar el binario de Titan (${TITAN_VERSION}). Revisa la release."; exit 1; }

# Copiar el proyecto Moon (solo código fuente)
COPY projects/moon/ ./projects/moon/

WORKDIR /app/projects/moon

# Directorios de subida (los crea también el runtime al arrancar)
RUN mkdir -p uploads uploads/avatars uploads/covers uploads/posts

# Puerto HTTP
EXPOSE 3000

# Healthcheck real contra /api/health
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1

# Ejecutar Moon
CMD ["/app/zett", "run", "src/main.titan"]
