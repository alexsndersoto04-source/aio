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

**Método reproducibles (recomendado)** — Postgres embebido en `projects/moon/ops/pg/`:

```sh
cd projects/moon
bash ops/setup-local.sh     # instala el paquete si falta, crea cluster + BD "moon"
```

Es idempotente: lo puedes ejecutar siempre que quieras; deja Postgres
corriendo en `127.0.0.1:5432` (usuario `moon`, sin password local) e
imprime la URL a usar. Alternativas de herramientas de BD:

```sh
node ops/db.mjs create-db    # crea la BD moon si no existe
node ops/db.mjs reset        # borra todas las tablas (dev)
node ops/db.mjs query "SELECT count(*) FROM users"
```

URL que usará la API:

```
postgres://moon@127.0.0.1:5432/moon
```

Las **migraciones** las aplica la API sola al arrancar (`db_init`),
no hay paso manual. (Si tienes tu propio Postgres, crea el usuario/BD
que quieras y ajusta `DATABASE_URL`.)

---

## 4. Arrancar la API

```sh
cd projects/moon
bash ops/start-api.sh        # envs saneos + JWT_SECRET persistente + zett run
```

El script espera el binario en `projects/moon/bin/zett` (descárgalo con
`bash ops/fetch-zett.sh`). También puedes exportar las variables a mano
(mínimo `DATABASE_URL` + `JWT_SECRET` ≥ 32 chars; opcionales `CORS_ORIGIN`,
`PUBLIC_BASE_URL`, `SMTP_*`) y ejecutar `bin/zett run src/main.titan`.

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

> **Corrida limpia (recomendada)**: la suite asume BD vacía — el PRIMER
> usuario registrado recibe el rol admin (bootstrap) y el rate limit de
> registro es de 3 cuentas por IP y 24 h. Antes de correr:
>
> ```sh
> bash ops/reset-db.sh         # borra todas las tablas (solo dev)
> # (re)arranca la API: los rate limits viven en memoria y se resetean
> bash ops/start-api.sh > /tmp/moon-server.log 2>&1 &
> sleep 3
> API_BASE=http://127.0.0.1:3000 MOON_LOG=/tmp/moon-server.log \
>   node projects/moon/test/e2e.mjs
> ```
>
> La suite crea usuarios con sufijo aleatorio (`alice_x7k2…`). Los tests
> de **2FA leen el código del log del servidor** (`MOON_LOG`): sin SMTP,
> el API imprime `Tu código para activar 2FA es: XXXXXX` y
> `Tu código de verificación es: XXXXXX` en consola.

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
