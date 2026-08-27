# Moon — Desarrollo local (API real + Postgres real + frontend real)

Todo lo de esta guía corre de verdad: misma API Titan que Render, misma base
de datos PostgreSQL, mismo frontend. CERO simulaciones.

---

## 1. Requisitos

| Herramienta | Para qué | Cómo obtenerla |
|---|---|---|
| Binario `zett` (Titan v1.0.0) | Compilar/ejecutar la API | ver sección 2 |
| PostgreSQL 16+ (cualquier versión ≥16) | Base de datos | `apt install postgresql`, Homebrew, Docker, o [embedded-postgres](https://www.npmjs.com/package/embedded-postgres) |
| Node.js 22.x | Frontend (Vite) | `nvm install 22` o tu gestor |

> **¿Sin acceso al CDN de GitHub Releases (red restringida)?**
> La CI de este repo espeja el binario oficial a la rama `tools-zett-x86_64`
> (lo hace el build script de `titan_lexer` en cada corrida de
> `cross-platform`). Bájalo con git:
>
> ```sh
> git fetch origin tools-zett-x86_64
> git show tools-zett-x86_64:tools/zett-linux-x86_64 > ./zett
> chmod +x ./zett
> ./zett --version
> ```

---

## 2. Obtener el binario `zett` (método normal)

```sh
# Linux x86_64
curl -L https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.0/zett-linux-x86_64.tar.gz | tar xz
./zett --version
```

(Windows/macOS/ARM: los assets están en la misma release.)

---

## 3. Base de datos local

```sh
# Ejemplo con Postgres local (ajusta usuario/pass)
sudo -u postgres createuser moon
sudo -u postgres createdb -O moon moon

# O con un usuario y password:
createuser moon
psql -c "ALTER USER moon WITH PASSWORD 'moon_pass_local';"
createdb -O moon moon
```

URL que usará la API:

```
postgres://moon:moon_pass_local@127.0.0.1:5432/moon
```

Las **14 migraciones** las aplica la API sola al arrancar
(`db_init` → `std::postgres::migrate`), no hay paso manual.

---

## 4. Arrancar la API

```sh
cd projects/moon

# Variables (mínimo: DATABASE_URL + JWT_SECRET de >=32 chars)
export DATABASE_URL="postgres://moon:moon_pass_local@127.0.0.1:5432/moon"
export JWT_SECRET="$(head -c 48 /dev/urandom | base64)"
export PORT=3000
# Opcionales:
# export CORS_ORIGIN="*"            (dev; en prod: la URL del frontend)
# export PUBLIC_BASE_URL="http://localhost:3000"
# export SMTP_HOST=... SMTP_PORT=... SMTP_USER=... SMTP_PASS=... SMTP_FROM=...

../../zett run src/main.titan
```

Arranque correcto se ve así:

```text
Conectando a PostgreSQL...
Pool PostgreSQL creado (10 conexiones, TLS).
Migraciones aplicadas: 14
...
Moon API escuchando en 0.0.0.0:3000
```

Verifica:

```sh
curl http://127.0.0.1:3000/api/health
# {"status":"ok","db":true}
```

> **Sin SMTP configurado**: los códigos de 2FA/recuperación se imprimen en la
> consola (línea `Tu código de verificación es: XXXXXX`).

---

## 5. Frontend local (dev, con proxy)

En otra terminal:

```sh
cd frontend
npm ci
API_PROXY_TARGET=http://127.0.0.1:3000 npm run dev
```

Abre la URL que imprime Vite (usualmente `http://localhost:5173`). El proxy
reenvía `/api/*` a la API local, así no necesitas `VITE_API_URL` en dev.

Build de producción:

```sh
npm run build      # genera dist/
npm start          # servidor estático (server.cjs) en $PORT
```

---

## 6. Pruebas E2E (la API + Postgres de verdad)

Con la API del paso 4 corriendo:

```sh
node projects/moon/test/e2e.mjs
# o con otra base:
API_BASE=http://127.0.0.1:3000 node projects/moon/test/e2e.mjs
```

Cubre: registro, login, 2FA tokens, refresh con rotación, posts + hashtags,
feed (4 variantes), comentarios, likes/saves, follow/unfollow, búsqueda,
sugerencias, **mensajería en vivo por WebSocket**, notificaciones, **subida
real de imágenes (JPEG)** y servido binario, reportes, panel admin, 404/405,
JWT inválido y escape XSS. Sale `0` solo si todo pasa.

> **Para una corrida limpia** (recomendado): la suite asume BD vacía — el
> PRIMER usuario registrado recibe el rol admin (bootstrap, ver
> `h_auth_register`) y el rate limit de registro es de 3 cuentas por IP y
> 24 h. Antes de correr la suite:
>
> ```sh
> node /home/user/reset-moon-db.mjs   # borra todas las tablas (solo dev)
> # (re)arranca la API para resetear el estado en memoria de los rate limits
> ```
>
> El script crea usuarios con sufijo aleatorio (`alice_x7k2…`) para no chocar
> con datos previos; los códigos de 2FA se leen del log del servidor si se
> pasa `MOON_LOG` (p. ej. el archivo donde redirigiste la salida de `zett run`).

---

## 7. Check rápido de compilación del backend (como la CI)

Si tienes Rust instalado:

```sh
# desde el root del repo
cargo run -q -p titan_cli -- check projects/moon/src/main.titan
```

`CHECK OK` = el backend parsea, tipa y genera bytecode sin errores.

---

## 8. Matriz de variables de entorno

| Variable | Requerida | Default | Notas |
|---|---|---|---|
| `DATABASE_URL` | **sí** | — | `postgres://user:pass@host:5432/db` |
| `JWT_SECRET` | **sí** | — | ≥ 32 caracteres |
| `PORT` | no | `3000` | Puerto HTTP |
| `CORS_ORIGIN` | no | `*` (dev) | URL exacta del frontend en prod |
| `PUBLIC_BASE_URL` | no | `http://localhost:3000` | Base para enlaces de emails |
| `SMTP_HOST/PORT/USER/PASS/FROM` | no | — | Sin SMTP, los códigos salen a consola |

---

## 9. Limpieza

- Imágenes subidas: `projects/moon/uploads/` (se puede borrar todo; la BD
  queda con URLs que ya no sirven — en local es indiferente).
- BD: `dropdb moon` (o `DROP DATABASE moon;`).
- En Render: las migraciones son idempotentes; para resetear, borra la DB
  desde el dashboard y deja que `db_init` reaplique.
