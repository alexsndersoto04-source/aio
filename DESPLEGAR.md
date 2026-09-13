# Cómo dejar Moon con un enlace fijo, gratis y sin tarjeta

Hoy la aplicación vive dentro de la máquina temporal de esta sesión: cuando la
sesión se apaga, su enlace deja de existir. Estos tres pasos la sacan de ahí y
le dan una **dirección fija** que puedes mandar a cualquiera.

Todo es plan gratuito y **ninguno de los dos servicios pide tarjeta**. Son unos
5 minutos y solo hay que pulsar botones: no hace falta escribir código.

---

## Paso 1 · Crear la base de datos (Neon)

1. Entra en **https://neon.tech** y pulsa *Sign up* → **Continue with GitHub**.
   (Es la cuenta de GitHub, no hay que rellenar formularios ni pagar nada.)
2. Cuando pregunte, crea un proyecto: nombre `moon`, región la que te sugiera.
3. Busca el botón **Connect** (o *Connection string*) y copia la cadena que
   empieza por `postgresql://`. Guárdala un momento: es la dirección de tu base
   de datos. Ya está creada y es permanente.

## Paso 2 · Publicar el servidor (Render)

1. Entra en **https://render.com** y pulsa *Get Started* → **GitHub** (tampoco
   pide tarjeta).
2. Arriba a la derecha: **New +** → **Blueprint**.
3. Elige el repositorio `alexsndersoto04-source/aio` y pulsa *Connect*.
   Render leerá solo el archivo `render.yaml` que ya está preparado.
4. Cuando pida el valor de **DATABASE_URL**, pega la cadena de Neon del Paso 1.
5. Pulsa **Apply** y espera unos 3 minutos (verás cómo compila y arranca).

Al terminar, arriba del panel aparece la dirección del servicio, del estilo:

```
https://moon-xxxx.onrender.com
```

Esa es la dirección de tu red social: ábrela, pulsa **Regístrate** y tu cuenta
será la administradora (el primer usuario de una instalación nueva lo es).

## Paso 3 · Contármelo

Pásame esa dirección y yo compruebo desde aquí lo que se puede comprobar, dejo
el resto afinado y te digo qué falta.

---

## Cosas que conviene saber del plan gratuito

| Tema | Qué pasa | Solución |
|---|---|---|
| El servidor «duerme» tras 15 min sin visitas | La primera visita del día tarda ~30-50 s; después va normal | Se puede despertar cada 10 min con un servicio gratuito de avisos (`cron-job.org`), o pasar al plan de pago más adelante |
| Las fotos subidas se guardan en disco temporal | Se pierden si el servidor se reinicia | Siguiente mejora ya planificada: guardarlas dentro de la base de datos, que sí es permanente |
| La base de datos de Render caduca a los 30 días | Por eso usamos **Neon**, que es gratis y no caduca | — |

## Si prefieres no crear cuentas todavía

Se puede seguir usando la máquina de esta sesión mientras esté encendida, pero
su enlace **no es fijo**: vive lo que viva la sesión. Para algo que puedas
compartir sin que se caiga, hacen falta los dos pasos de arriba.
