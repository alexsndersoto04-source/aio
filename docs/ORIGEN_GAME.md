# 🌍 ORIGEN — Un planeta vivo hecho de tus datos

**Concepto de juego:** un mundo masivo, procedural y "vivo" cuyo universo nace
de **datos reales del jugador** — ubicación GPS, hora local, sensores del
celular (vía Termux) y hasta tus propias notas. Ningún jugador tiene el mismo
planeta, porque cada planeta se siembra con la vida real de quien lo juega.

> Esto es lo que hace que no sea "un clon": no es un shooter ni un plataformas
> más. Es un **ecosistema/exploración** donde TÚ y tus datos son la semilla del
> mundo. Es masivo (generación procedural), personal (tu vida lo define) y
> corre **offline** dentro de un APK que instala en tu celular.

---

## La arquitectura (cómo GitHub hace el trabajo pesado)

```
TÚ (Termux)           GITHUB ACTIONS (pesado)          RESULTADO
────────────          ──────────────────────           ─────────
escribes .titan ──push──▶ compila TITAN ──▶ WASM        juego jugable
   + HTML/JS   ────────▶ monta app Android WebView ─▶ APK descargable
   git push            firma el APK                   en el celular
                       publica GitHub Pages           probar en navegador
```

- **Tú en Termux:** solo editas `game/src/*.titan` y haces `git push`.
- **GitHub Actions:** compila el binario `titan`, compila el juego a WASM,
  arma el APK WebView, lo firma y lo sube como *artifact* (y al Release si
  haces `git tag v1.0`).
- **GitHub Pages:** publica el juego para probarlo al instante desde el
  navegador sin instalar nada.

---

## Lo que ya está construido (funcional)

| Archivo | Qué es |
|---|---|
| `.github/workflows/build-apk.yml` | Pipeline: compila titan → WASM → APK → Pages |
| `android/` | App Android WebView mínima que carga el juego |
| `game/src/main.titan` | Demo del planeta vivo (procedural + toque) para WASM/APK |
| `game/public/` | Página + host (index.html, host.js, genera origen.wasm) |
| `game/native_origen.titan` | Prototipo con estado real, corre en Termux |

## Cómo probar AHORA

```bash
# 1) En Termux, la versión con estado:
titan run game/native_origen.titan

# 2) La demo web (requiere servidor HTTP, no file://):
titan wasm game/src/main.titan --output game/public/origen.wasm
cd game/public && python3 -m http.server 8080
# abre http://127.0.0.1:8080 en un navegador

# 3) El APK: haz git push y descárgalo del artifact de GitHub Actions
#    (Acciones → build-apk → el .apk de la corrida).
```

---

## El paso grande que sigue: variables globales mutables

Para pasar de "demo procedural" a **juego masivo con estado** (guardar
semillas, inventario, progresión, mundo persistente), el backend WASM necesita
una pieza que hoy no tiene: **variables globales mutables** (el parser solo
acepta `Fn/Struct/Enum/Trait/Impl/Import/Const/Type` en top-level; `let mut` es
solo local).

Ese es el siguiente gran movimiento y es **doblemente brutal**: haces el juego
**y** le agregas un feature nuevo a tu propio compilador (lexer → parser → AST
→ typechecker → codegen → VM/WASM). Es justo lo que "marcar la diferencia"
significa: crecer el lenguaje tú mismo.

### Roadmap sugerido del juego completo

1. **Mundo semilla:** generar biomas/tiles desde la hora + ubicación real.
2. **Criaturas vivas:** un ecosistema que evoluciona solo (cada criatura
   recuerda al jugador).
3. **Progresión e inventario:** semillas → mejoras → nuevos biomas.
4. **Historia procedural:** los eventos del mundo se escriben mientras juegas.
5. **Persistencia:** el mundo guardado en el celular (SQLite) entre sesiones.
