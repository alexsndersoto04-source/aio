# 🚀 Guía de Inicio Rápido — Nexo

## Instalación Local (5 minutos)

### 1. Instalar dependencias

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install postgresql postgresql-contrib redis-server
```

**macOS:**
```bash
brew install postgresql redis
brew services start postgresql redis
```

### 2. Configurar PostgreSQL

```bash
# Crear usuario y base de datos
sudo -u postgres psql << EOF
CREATE USER nexo WITH PASSWORD 'nexo_password';
CREATE DATABASE nexo OWNER nexo;
GRANT ALL PRIVILEGES ON DATABASE nexo TO nexo;
EOF
```

### 3. Iniciar Redis

```bash
# Ubuntu
sudo systemctl start redis-server

# macOS (si usas brew services, ya está corriendo)
# redis-server &
```

### 4. Compilar Titan (si no lo has hecho)

```bash
cd /home/user/aio
cargo build --release -p titan_cli
```

### 5. Ejecutar Nexo

```bash
./target/release/titan run projects/nexo/src/main.titan
```

Verás:
```
=================================
  NEXO — Red Social Backend
  v0.1.0
=================================

Conectando a PostgreSQL...
Base de datos conectada.
Ejecutando migraciones...
Migraciones completadas.
Conectando a Redis...
Redis conectado.

Router configurado con 40+ endpoints.

=================================
  Servidor Nexo escuchando en:
  http://0.0.0.0:3000
=================================
```

### 6. Probar la API

```bash
# Health check
curl http://localhost:3000/api/health

# Registrar usuario
curl -X POST http://localhost:3000/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"test","email":"test@example.com","password":"password123"}'

# Guardar el token de la respuesta
TOKEN="eyJ..."

# Crear un post
curl -X POST http://localhost:3000/api/posts \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"content":"¡Mi primer post!"}'
```

## Deploy en Oracle Cloud (Gratis para siempre)

### ¿Por qué Oracle Cloud?

- ✅ **Gratis PARA SIEMPRE** (no 12 meses, SIEMPRE)
- ✅ **4 CPUs ARM + 24 GB RAM** (una bestia)
- ✅ **200 GB de almacenamiento**
- ✅ **IP pública fija**
- ✅ Perfecto para una red social real

### Paso 1: Crear cuenta en Oracle Cloud

1. Ir a https://cloud.oracle.com
2. Sign Up (necesitas tarjeta de crédito para verificar, **NO te cobran nada**)
3. Verificar email

### Paso 2: Crear VM gratuita

1. Menú → Compute → Instances
2. Create Instance
3. Configuración:
   - **Name:** nexo-server
   - **Image:** Canonical Ubuntu 22.04 aarch64 (ARM)
   - **Shape:** VM.Standard.A1.Flex (Always Free)
     - OCPU count: **4**
     - Memory (GB): **24**
   - **Networking:** Crear VCN si no existe
   - **SSH keys:** Agregar tu clave pública (`~/.ssh/id_rsa.pub`)
4. Click **Create**

### Paso 3: Conectar a la VM

```bash
# Esperar 1-2 minutos a que la VM arranque
ssh ubuntu@<IP_PUBLICA>
```

### Paso 4: Ejecutar script de instalación

```bash
# Clonar el repo
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio

# Ejecutar script de instalación
sudo bash projects/nexo/scripts/deploy_oracle.sh
```

El script automáticamente:
- ✅ Instala PostgreSQL, Redis, Nginx
- ✅ Instala Rust y compila Titan
- ✅ Configura la base de datos
- ✅ Crea servicio systemd para Nexo
- ✅ Configura Nginx como reverse proxy
- ✅ Inicia todo

### Paso 5: Acceder a tu API

```bash
# Tu API está en:
http://<IP_PUBLICA>/api/health

# Ejemplo:
curl http://123.45.67.89/api/health
```

### Paso 6: Configurar JWT secret (IMPORTANTE)

```bash
# Generar secreto seguro
sudo bash projects/nexo/scripts/generate_secret.sh

# Editar servicio
sudo nano /etc/systemd/system/nexo.service

# Cambiar la línea:
Environment=NEXO_JWT_SECRET=tu_secreto_generado_aqui

# Reiniciar
sudo systemctl restart nexo
```

### Paso 7: Configurar dominio (opcional pero recomendado)

```bash
# Instalar certbot
sudo apt install certbot python3-certbot-nginx

# Obtener certificado SSL
sudo certbot --nginx -d tusocial.com -d www.tusocial.com

# Auto-renovar
sudo certbot renew --dry-run
```

## Comandos útiles

```bash
# Ver estado del servicio
sudo systemctl status nexo

# Ver logs en tiempo real
sudo journalctl -u nexo -f

# Reiniciar servicio
sudo systemctl restart nexo

# Detener servicio
sudo systemctl stop nexo

# Ver uso de recursos
htop

# Backup de PostgreSQL
sudo -u postgres pg_dump nexo > backup_$(date +%Y%m%d).sql

# Restaurar backup
sudo -u postgres psql nexo < backup_20240101.sql
```

## Troubleshooting

### PostgreSQL no conecta

```bash
sudo systemctl status postgresql
sudo systemctl restart postgresql
```

### Redis no conecta

```bash
sudo systemctl status redis-server
sudo systemctl restart redis-server
```

### Nexo no arranca

```bash
# Ver logs
sudo journalctl -u nexo -n 50

# Probar manualmente
cd /opt/nexo-repo
titan run projects/nexo/src/main.titan
```

### Puerto 80 ocupado

```bash
sudo lsof -i :80
sudo systemctl stop apache2  # si hay Apache corriendo
```

## Próximos pasos

1. ✅ **Backend listo** — Tu API está corriendo
2. 📱 **Frontend** — Construir UI con React/Vue
3. 🌐 **Dominio** — Comprar dominio y configurar DNS
4. 🔒 **SSL** — Certificado Let's Encrypt
5. 💾 **Backups** — Configurar backups automáticos
6. 📊 **Monitoreo** — Configurar Prometheus + Grafana

---

**¡Listo!** Tu red social está corriendo en Oracle Cloud gratis. 🚀
