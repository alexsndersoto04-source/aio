# Programar TITAN desde Android con VS Code en el navegador

> Esta guía usa **Termux + code-server**. Es la forma de tener la interfaz de VS Code en Chrome y, al mismo tiempo, poder ejecutar el compilador en el teléfono.

## 1. Instala las herramientas en Termux

Instala Termux desde F-Droid o GitHub Releases (no la versión antigua de Play Store). Después ejecuta:

```sh
pkg update && pkg upgrade
pkg install git nodejs rust code-server
pkg install zett
```

El repositorio APT oficial de Zett puede añadirse si `pkg install zett` todavía no lo encuentra:

```sh
echo 'deb [trusted=yes] https://raw.githubusercontent.com/alexsndersoto04-source/aio/zett-repo ./ ' > "$PREFIX/etc/apt/sources.list.d/zett.list"
pkg update
pkg install zett
```

Comprueba el compilador:

```sh
zett version
```

## 2. Instala LSP y DAP

La CLI distribuida se llama `zett`; la integración del editor necesita además los binarios `titan-lsp` y `titan-dap`. Desde una copia del repositorio:

```sh
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
cargo install --path crates/titan_lsp
cargo install --path crates/titan_dap
# Opcional: la CLI compilada desde fuentes se instala como titan
cargo install --path crates/titan_cli
```

Si usas `zett` en vez de `titan`, configura `titan.compiler.path` en VS Code como `zett`. Verifica:

```sh
titan-lsp --help 2>/dev/null || true
titan-dap </dev/null
```

> Compilar Rust en un teléfono requiere espacio libre y puede tardar. No cierres Termux mientras termina. Si ya tienes los tres binarios en otra carpeta, usa las rutas absolutas en los ajustes de la extensión.

## 3. Empaqueta e instala la extensión

Desde la copia del repositorio:

```sh
cd ~/aio/editors/vscode-titan
npm install
npm run package
code-server --install-extension titan-language-tools-0.1.0.vsix
```

## 4. Abre VS Code en Chrome

Inicia code-server ligado solamente a tu teléfono:

```sh
code-server --bind-addr 127.0.0.1:8080 ~/proyectos
```

Abre `http://127.0.0.1:8080` en Chrome. Establece una contraseña cuando code-server la pida. Mantén esa dirección local: no uses `0.0.0.0` salvo que sepas proteger el servidor.

En **Settings**, busca `TITAN` y configura:

- **Titan: Compiler Path:** `zett` o `titan`
- **Titan: Lsp Path:** `titan-lsp`
- **Titan: Dap Path:** `titan-dap`

## 5. Crea, ejecuta y depura un proyecto

En la paleta de comandos (`Ctrl+Shift+P` desde teclado físico o el menú):

1. Ejecuta **TITAN: Create project**.
2. Abre `src/main.titan`.
3. Usa **TITAN: Run active file/project**. La salida aparece en el panel **Terminal**.
4. Coloca un breakpoint al lado de una línea y selecciona **TITAN: Debug active file/project**.
5. Usa **TITAN: Build bytecode** para crear `target/<nombre>.tbc` o **TITAN: Build WebAssembly** para crear `.wasm` y source maps.

## Sobre “ejecutables reales”

TITAN hoy crea dos artefactos reales: bytecode `.tbc` portable (se ejecuta con `titan exec`) y WebAssembly `.wasm` (para navegador o un host WASM). No afirma generar aún binarios nativos autónomos como `.apk`, `.exe` o ELF. Crear ese backend nativo sería una función nueva del compilador, no algo que VS Code pueda resolver por sí mismo.
