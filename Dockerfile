# Moon — API backend en Titan (Docker)
# Usa el binario pre-compilado de Titan v1.0.0 (release oficial del repo).

FROM ubuntu:24.04

# Dependencias mínimas
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Descargar binario pre-compilado de Titan (v1.0.0)
ARG TITAN_VERSION=v1.0.0
RUN curl -fL https://github.com/alexsndersoto04-source/aio/releases/download/${TITAN_VERSION}/zett-linux-x86_64.tar.gz | tar xz \
    && test -x /app/zett && /app/zett --version || true

# Copiar el proyecto Moon (solo código fuente)
COPY projects/moon/ ./projects/moon/

WORKDIR /app/projects/moon

# Directorios de subida (los crea también el runtime al arrancar)
RUN mkdir -p uploads

# Puerto HTTP
EXPOSE 3000

# Healthcheck real contra /api/health
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3000/api/health || exit 1

# Ejecutar Moon
CMD ["/app/zett", "run", "src/main.titan"]
