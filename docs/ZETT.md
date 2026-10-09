# Zett — el gestor de paquetes de Titan

**Zett** es el gestor de paquetes de Titan: añade dependencias a un proyecto, las
descarga, comprueba que son auténticas y las deja listas para usarlas con `import`.
También crea y publica paquetes propios.

Zett viene escrito en Titan, igual que el resto del compilador. No es un programa
aparte: es el mismo ejecutable que `titan`, que cuando se llama `zett` solo ofrece la
gestión de paquetes. El paquete de Titan trae un enlace `zett → titan`, y el instalador de
Termux (`selfhost/instalar-termux.sh`) lo deja en el `PATH`.

> Los mismos comandos también funcionan escribiendo `titan add`, `titan fetch`, etc.
> Con `zett` es más corto de escribir y deja claro que se trata de paquetes.

## Comandos

| Comando | Qué hace |
|---|---|
| `zett add <paquete> [requisito]` | Añade la dependencia a `Titan.toml` (por ejemplo `zett add demo "^1"`) |
| `zett fetch` · `zett install` | Resuelve versiones, descarga, verifica e instala las dependencias; escribe `Titan.remote.lock` |
| `zett fetch --offline` | Instala solo desde el caché y el archivo de bloqueo, sin red |
| `zett update` | Vuelve a resolver y actualiza a las versiones más nuevas que permite el requisito |
| `zett keygen <ruta>` | Genera una clave privada Ed25519 para firmar paquetes |
| `zett pack --key K --output O` | Crea un paquete `.tpkg` determinista y firmado |
| `zett publish --key K` | Empaqueta, firma y sube el paquete al registro |
| `zett version` | Muestra la versión |

Opciones comunes: `--project <carpeta>` (por defecto, la actual) y `--registry <URL>`
(por defecto `https://registry.titan-lang.org`; debe ser HTTPS).

## Usar una dependencia

```bash
cd mi_app
zett add demo "^1"      # escribe [dependencies.demo] en Titan.toml
zett fetch              # descarga e instala en .titan/packages/
titan run
```

`Titan.toml` queda así:

```toml
[dependencies.demo]
version = "^1"
```

Los requisitos de versión siguen SemVer (`^1`, `~1.2`, `>=1.0, <2`, `*`). Si dos paquetes
piden versiones que chocan, Zett prueba otras combinaciones, empezando por las más altas.
El resultado exacto queda en `Titan.remote.lock` (versión, hash y clave de firma de cada
paquete); conviene subirlo a git para que todos instalen lo mismo.

## Qué comprueba Zett antes de instalar

- La dirección del registro debe ser **HTTPS**.
- El hash **SHA-256** del archivo descargado debe coincidir con el del registro.
- La **firma Ed25519** del hash debe ser válida.
- Al extraer, solo se aceptan archivos y carpetas normales con rutas relativas: se
  rechazan enlaces, dispositivos, duplicados y rutas que salgan de la carpeta.
- La descarga se escribe en un archivo temporal y solo se mueve a su sitio tras la
  verificación, así una descarga interrumpida nunca se toma por un paquete.

## Crear y publicar un paquete

```bash
zett keygen ~/.titan/mi-clave          # una sola vez; guarda la clave fuera del proyecto
zett pack --key ~/.titan/mi-clave --output mi_paquete.tpkg
export TITAN_REGISTRY_TOKEN=...        # la credencial de tu registro
zett publish --key ~/.titan/mi-clave
```

- Un `.tpkg` es un `tar.gz` con `Titan.toml`, `src/` y el resto del proyecto, sin
  archivos de git, cachés ni compilaciones. Es **determinista**: el mismo proyecto da el
  mismo archivo, byte a byte.
- `pack` solo muestra la clave pública, el hash y la firma; la clave privada nunca se
  imprime y no puede estar dentro del proyecto.
- `publish` lee la credencial solo de `TITAN_REGISTRY_TOKEN`.

## Qué está probado y qué no

Probado de verdad (pruebas del repositorio, `selfhost/tests/pkg/`):
- Resolución de versiones y SemVer, extracción segura, hashes y firmas.
- `add`, `fetch`, `update` y `publish` contra un registro HTTPS **de prueba** que corre en
  la propia máquina (`selfhost/tests/pkg/registro/probar.sh`).
- `keygen` y `pack` (incluido que dos empaquetados salen idénticos) con el ejecutable
  `zett` del paquete.

**Lo que no está probado ni existe en este repositorio:**
- **No hay un servidor de registro dentro de este repositorio.** El cliente está
  completo, pero necesita un registro que hable su protocolo (descrito en
  [`PACKAGE_REGISTRY.md`](PACKAGE_REGISTRY.md)). La dirección por defecto,
  `registry.titan-lang.org`, no se ha podido comprobar que exista; si no responde, indica
  otro con `--registry`.
- La rotación de claves y la política de propiedad de los paquetes dependen del
  registro, no del cliente.
- En ARM64 (Termux) el gestor usa el mismo código que en x86-64, pero no se ha probado
  allí paquete por paquete.

Detalles técnicos del protocolo, del formato `.tpkg` y del resolvedor:
[`PACKAGE_REGISTRY.md`](PACKAGE_REGISTRY.md).
