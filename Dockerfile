FROM ubuntu:24.04

# Instalar dependencias
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Descargar binario pre-compilado de Titan (v1.0.0)
RUN curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.0/zett-linux-x86_64.tar.gz | tar xz

# Copiar el proyecto Moon
COPY projects/moon/ ./projects/moon/

WORKDIR /app/projects/moon

# Crear directorio de uploads
RUN mkdir -p uploads

# Exponer puerto
EXPOSE 3000

# Ejecutar Moon
CMD ["/app/zett", "run", "src/main.titan"]
