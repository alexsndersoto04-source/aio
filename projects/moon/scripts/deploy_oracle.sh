#!/bin/bash
# ============================================================
# Nexo — Script de instalación en Oracle Cloud Free Tier
# ============================================================
# Ejecutar como root en una VM Oracle Cloud (Ubuntu 22.04 ARM64)
#
# Paso 1: Crear VM en Oracle Cloud
#   1. Ir a https://cloud.oracle.com
#   2. Compute → Instances → Create Instance
#   3. Image: Canonical Ubuntu 22.04 aarch64 (ARM)
#   4. Shape: VM.Standard.A1.Flex (gratis)
#      - OCPU count: 4
#      - Memory: 24 GB
#   5. Networking: crear VCN si no existe
#   6. Agregar tu SSH public key
#   7. Create
#
# Paso 2: Conectar por SSH
#   ssh ubuntu@<IP_PUBLICA>
#
# Paso 3: Ejecutar este script
#   sudo bash deploy_oracle.sh
# ============================================================

set -e

echo "==========================================="
echo "  NEXO — Instalación en Oracle Cloud"
echo "==========================================="
echo ""

# 1. Actualizar sistema
echo "[1/8] Actualizando sistema..."
apt update && apt upgrade -y

# 2. Instalar dependencias
echo "[2/8] Instalando dependencias..."
apt install -y curl build-essential pkg-config libssl-dev git postgresql postgresql-contrib redis-server nginx

# 3. Instalar Rust
echo "[3/8] Instalando Rust..."
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
echo "Rust instalado: $(rustc --version)"

# 4. Configurar PostgreSQL
echo "[4/8] Configurando PostgreSQL..."
systemctl enable postgresql
systemctl start postgresql

sudo -u postgres psql -c "CREATE USER nexo WITH PASSWORD 'nexo_password';" || true
sudo -u postgres psql -c "CREATE DATABASE nexo OWNER nexo;" || true
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE nexo TO nexo;" || true

echo "PostgreSQL configurado."

# 5. Configurar Redis
echo "[5/8] Configurando Redis..."
systemctl enable redis-server
systemctl start redis-server

# Configurar Redis con password (opcional, recomendado para producción)
# sed -i 's/# requirepass foobared/requirepass tu_password_aqui/' /etc/redis/redis.conf
# systemctl restart redis

echo "Redis configurado."

# 6. Clonar y compilar Nexo
echo "[6/8] Clonando y compilando Nexo..."
cd /opt
git clone https://github.com/alexsndersoto04-source/aio.git nexo-repo || true
cd nexo-repo

# Compilar Titan CLI
cargo build --release -p titan_cli

# Crear symlink
ln -sf /opt/nexo-repo/target/release/titan /usr/local/bin/titan

echo "Titan compilado: $(titan version)"

# 7. Configurar Nexo como servicio systemd
echo "[7/8] Configurando servicio systemd..."

cat > /etc/systemd/system/nexo.service << 'EOF'
[Unit]
Description=Nexo Social Network Backend
After=network.target postgresql.service redis-server.service
Requires=postgresql.service redis-server.service

[Service]
Type=simple
User=ubuntu
WorkingDirectory=/opt/nexo-repo
ExecStart=/usr/local/bin/titan run projects/nexo/src/main.titan
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=nexo

# Environment variables
Environment=NEXO_JWT_SECRET=cambia-esto-por-un-secreto-muy-largo-y-seguro-de-al-menos-64-caracteres-aleatorios
Environment=RUST_LOG=info

# Security
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/nexo-repo/projects/nexo/uploads

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable nexo
systemctl start nexo

echo "Servicio Nexo configurado e iniciado."

# 8. Configurar Nginx como reverse proxy
echo "[8/8] Configurando Nginx..."

cat > /etc/nginx/sites-available/nexo << 'EOF'
server {
    listen 80;
    server_name _;  # Cambiar por tu dominio si tienes uno

    # Uploads estáticos
    location /uploads/ {
        alias /opt/nexo-repo/projects/nexo/uploads/;
        expires 30d;
        add_header Cache-Control "public, immutable";
    }

    # API proxy
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        
        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
        
        # Upload size
        client_max_body_size 20M;
    }
}
EOF

ln -sf /etc/nginx/sites-available/nexo /etc/nginx/sites-enabled/nexo
rm -f /etc/nginx/sites-enabled/default

nginx -t
systemctl restart nginx

echo ""
echo "==========================================="
echo "  ¡INSTALACIÓN COMPLETADA!"
echo "==========================================="
echo ""
echo "Nexo está corriendo en:"
echo "  http://$(curl -s ifconfig.me)"
echo ""
echo "Endpoints:"
echo "  Health:  http://$(curl -s ifconfig.me)/api/health"
echo "  API:     http://$(curl -s ifconfig.me)/api/"
echo ""
echo "Comandos útiles:"
echo "  systemctl status nexo      — Ver estado del servicio"
echo "  systemctl restart nexo     — Reiniciar"
echo "  journalctl -u nexo -f      — Ver logs en tiempo real"
echo ""
echo "Próximos pasos:"
echo "  1. Cambiar el JWT_SECRET en /etc/systemd/system/nexo.service"
echo "  2. Configurar dominio y SSL con Let's Encrypt (certbot)"
echo "  3. Configurar backups de PostgreSQL"
echo "  4. Construir el frontend (React/Vue)"
echo ""
