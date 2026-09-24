FROM node:22-slim

# Instalar certificados y utilidades esenciales
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copiar definiciones de dependencias
COPY projects/moon/server/package*.json ./projects/moon/server/
COPY frontend/package*.json ./frontend/

# Instalar dependencias limpias
RUN npm --prefix projects/moon/server ci --omit=dev
RUN npm --prefix frontend ci

# Copiar frontend y compilar
COPY frontend/ ./frontend/
RUN npm --prefix frontend run build

# Copiar servidor de Moon
COPY projects/moon/server/ ./projects/moon/server/

# Configuración para Hugging Face Spaces (Puerto 7860 estándar)
ENV PORT=7860
ENV NODE_ENV=production

EXPOSE 7860

# Iniciar servidor de Moon
CMD ["node", "projects/moon/server/src/index.mjs"]
