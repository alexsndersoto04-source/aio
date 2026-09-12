# Moon — demostración

Dos páginas que se abren con doble clic, **sin servidor, sin instalar nada y
sin conexión** (todo el CSS y el JavaScript va dentro del archivo):

| Archivo | Qué es |
|---|---|
| `moon.html` | La aplicación en **modo demostración**: misma interfaz y mismas interacciones, con datos de ejemplo que viven solo en el navegador. Nada se guarda ni se envía a ningún sitio. |
| `diseno.html` | Galería del sistema de diseño «Órbita»: botones, textos, publicaciones, avisos, cargas, mensajes y administración. |

## Cómo se generan

Desde la carpeta `frontend/`:

```bash
node scripts/diseno-suelto.mjs --app               # → frontend/moon-demo.html
node scripts/diseno-suelto.mjs --destino ../demo/diseno.html
```

Son archivos generados: se vuelven a crear cada vez que cambia el diseño, así
que no se editan a mano.

## Modo demostración

Se activa cuando la página pone `window.MOON_DEMO = true` (o con `?demo` en la
dirección). `frontend/src/demo.js` responde a las llamadas `/api/...` con datos
de ejemplo en memoria; la aplicación real no se entera: sin esa marca, todo
sigue llamando al servidor de verdad.

## Comprobaciones

```bash
cd frontend
node scripts/prueba-demo.mjs        # 71 llamadas de la API de ejemplo
npm i --no-save jsdom               # (una vez) navegador simulado
node scripts/prueba-render.mjs ../demo/moon.html --entrar --rutas
```

La última comprueba, sin navegador, que la página arranca, que se puede
entrar y que las nueve pantallas se pintan sin errores.
