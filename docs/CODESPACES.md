# TITAN completo desde el navegador con GitHub Codespaces

Esta es la opción para usar TITAN desde un teléfono **sin instalar VS Code ni Termux**. El navegador muestra VS Code y GitHub presta una máquina Linux remota donde se compilan y ejecutan las herramientas.

## Una sola vez

1. Entra a este repositorio en GitHub desde Chrome e inicia sesión.
2. Pulsa el botón verde **Code**.
3. Abre la pestaña **Codespaces**.
4. Pulsa **Create codespace on** y selecciona la rama que contiene estos cambios (`arena/019fcf81-aio`, o `main` cuando se integren).
5. Espera a que termine el mensaje **Setting up your dev container**. La primera vez puede tardar varios minutos: Codespaces compila TITAN completo en la máquina remota.
<<<<<<< HEAD
6. Cuando aparezca el editor, espera unos segundos. Codespaces instala automáticamente la extensión TITAN; **no abras el archivo `.vsix`**. Es un paquete binario, no un archivo de código.
7. Si aparece un aviso para recargar la ventana, acepta.

No tendrás que instalar nada manualmente en el mismo Codespace.

### Si abriste un Codespace antes de esta actualización

En la terminal integrada copia exactamente este comando (no pulses el archivo `.vsix`):

```sh
code --install-extension editors/vscode-titan/titan-language-tools-0.1.0.vsix --force
```

Después ejecuta **Developer: Reload Window** desde la paleta de comandos.

## Qué queda funcionando

El contenedor instala y deja en el `PATH`:

- `titan`: crear proyectos, comprobar, ejecutar, tests, bytecode y WebAssembly;
- `titan-lsp`: errores en el editor, autocompletado, definición, referencias y rename;
- `titan-dap`: breakpoints, pasos y variables al depurar.

La extensión configura automáticamente esos tres nombres. Puedes comprobarlos desde el terminal integrado:

```sh
titan version
titan-lsp --help
titan-dap </dev/null
```

## Crear y ejecutar tu primer programa

Desde la paleta de comandos usa **TITAN: Create project**. Abre el proyecto creado y `src/main.titan`.

Después puedes usar los botones/comandos:

- **TITAN: Run active file/project** para ejecutar;
- **TITAN: Check active file/project** para encontrar errores sin ejecutar;
- **TITAN: Build bytecode** para producir `target/<proyecto>.tbc`;
- **TITAN: Build WebAssembly** para producir `.wasm` y source maps;
- **TITAN: Debug active file/project** para colocar breakpoints y depurar;
- **TITAN: Run tests** para los archivos de `tests/`.

El resultado de ejecución aparece abajo, en el panel **Terminal**.

## Límites reales

Codespaces prepara todo el ecosistema que está en este repositorio: compilador, VM, stdlib, WASM, LSP y DAP. Algunas funciones TITAN siguen necesitando recursos externos por su propia naturaleza: una base de datos PostgreSQL/MySQL, una API web, un modelo ONNX o una interfaz gráfica. El compilador sí estará disponible, pero esos servicios se deben configurar cuando tu programa los use.

TITAN genera bytecode `.tbc` y WebAssembly `.wasm`. Aún no genera por sí mismo `.apk`, `.exe` ni binarios Linux autónomos.

## Coste y suspensión

GitHub puede dar una cuota gratuita limitada de Codespaces según tu cuenta. Cuando termines, abre el menú de Codespaces en GitHub y pulsa **Stop** para no consumir tiempo. Tus archivos permanecen en el Codespace hasta que lo elimines; haz `git commit` y `git push` para guardarlos también en el repositorio.
