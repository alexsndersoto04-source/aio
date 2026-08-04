# 🏗️ TitanForge

**Plataforma serverless *sandboxed* para TITAN, escrita en TITAN.**

Es un "online judge / cloud functions" para tu propio lenguaje: los clientes
envían código `.titan` por HTTP, se ejecuta dentro de un **sandbox por
capacidades** y devuelven la salida en JSON. Es el punto de partida para
cosas más grandes (un SaaS, un juez de programación, CI remoto, etc.).

## ¿Qué combina?

| Capacidad | Uso en TitanForge |
|---|---|
| `std::server` + `std::router` | API HTTP con matchit |
| `std::json` | parsear / serializar requests y respuestas |
| `std::process` + `--sandbox` | ejecutar el código aislado como subproceso |
| `std::metrics` | contadores + histogramas + export Prometheus |
| `std::try::catch` | errores limpios (JSON 4xx) sin crashear |
| imports multiarchivo | `runner.titan`, `store.titan` |

## Estructura

```
titanforge/
├── Titan.toml
├── client.titan          # cliente de demo (corre en OTRA terminal)
└── src/
    ├── main.titan        # servidor, router, handlers, entry point
    ├── runner.titan      # ejecución del código en sandbox
    └── store.titan       # claves API y configuración
```

## Requisitos

- El binario `titan` (o `zett`) compilado y **visible en el PATH**.
- El servidor se arranca sin sandbox (para poder lanzar subprocesos);
  el **código de los clientes** sí corre con `--sandbox`.

## Cómo correr

```bash
# 1) Arrancar el servidor
cd titanforge
titan run src/main.titan          # o: titan run .

# 2) En OTRA terminal, probar con curl
curl http://127.0.0.1:8080/health

curl -X POST -H "X-Api-Key: demo-token" -H "Content-Type: application/json" \
     -d '{"code":"fn main() { print(\"hola titanforge\") }"}' \
     http://127.0.0.1:8080/run

curl http://127.0.0.1:8080/metrics

# 3) O bien con el cliente escrito en TITAN
titan run client.titan
```

## Endpoints

- `GET /` — página HTML de bienvenida
- `GET /health` — `{ "status": "ok", ... }`
- `POST /run` — body `{ "code": "...", "timeout_ms": 5000 }`, header `X-Api-Key`
  - responde `{ "ok": true, "stdout", "stderr", "exit_code", "duration_ms" }`
- `GET /metrics` — métricas Prometheus (requests, runs, histograma de duración)

## Siguientes pasos (hacia algo "grande")

1. **Mover `store.titan` a `std::sqlite` / `std::redis`** y emitir claves con
   `std::jwt::hs256` en vez de un map hardcodeado.
2. **Presupuesto de recursos por ejecución**: combinar con `std::runtime`
   (cuotas de memoria, GC) para limitar RAM/CPU de cada run.
3. **Endpoints de gestión**: `POST /functions` (registrar), `GET /runs/<id>`
   (historial), borrado y límites de tasa por clave.
4. **WebSocket** (`std::server::ws_*`) para ejecución en streaming (ver la
   salida en vivo en el navegador).
5. **Clientes en WASM**: compilar el runner a WebAssembly para que corra
   en el navegador.
