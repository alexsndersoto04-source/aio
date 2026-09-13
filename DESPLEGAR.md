# Cómo dejar Moon con un enlace fijo, gratis y sin tarjeta

Hoy la aplicación vive dentro de la máquina temporal de esta sesión: cuando la
sesión se apaga, su enlace deja de existir. Estos pasos la sacan de ahí y le dan
una **dirección fija** que puedes mandar a cualquiera.

Son dos cuentas gratuitas (Supabase y Render) y **ninguna pide tarjeta**.

---

## Paso 1 · Base de datos (Supabase)

Un proyecto de Supabase es una base de datos PostgreSQL de verdad.

1. Entra en **https://supabase.com/dashboard** con tu cuenta.
2. Pulsa el proyecto que ya tienes creado.
   - Si dice **«El proyecto está en pausa»**, pulsa el botón para restaurarlo
     (*Restore project* / *Resume*) y espera 1 o 2 minutos a que diga *Active*.
3. Pulsa el botón **Connect** (arriba del panel del proyecto).
4. En la ventana, elige la pestaña **Session pooler** (o **Direct connection**)
   y copia la cadena larga que empieza por `postgresql://`.
5. Esa cadena trae escrito `[YOUR-PASSWORD]` en el medio: hay que cambiarlo por
   la contraseña de la base de datos que elegiste al crear el proyecto.
   - Si no la recuerdas: **Settings → Database → Reset database password**,
     elige una nueva (apúntala) y usa esa.
   - Si eliges la opción *Session pooler*, la cadena ya trae el usuario
     `postgres.<algo>`; no hay que tocar nada más.

Esa cadena completa (con la contraseña dentro) **no se comparte con nadie**: se
pega directamente en el panel de Render, en el paso siguiente.

## Paso 2 · Servidor (Render)

1. Entra en **https://render.com** y pulsa *Get Started* → **GitHub** (tampoco
   pide tarjeta).
2. Arriba a la derecha: **New +** → **Blueprint**.
3. Elige el repositorio `alexsndersoto04-source/aio` y pulsa *Connect*.
   Render leerá solo el archivo `render.yaml` que ya está preparado.
4. Cuando pida el valor de **DATABASE_URL**, pega la cadena de Supabase del
   paso 1.
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

## Cosas que conviene saber de los planes gratuitos

| Tema | Qué pasa | Solución |
|---|---|---|
| El servidor «duerme» tras 15 min sin visitas | La primera visita del día tarda ~30-50 s; después va normal | Se puede despertar cada 10 min con un servicio gratuito de avisos (`cron-job.org`) |
| Supabase pausa el proyecto tras ~1 semana sin uso | No se pierde nada: se restaura con un botón | Un aviso automático gratuito cada pocos días lo mantiene despierto |
| Las fotos subidas se guardan en disco temporal de Render | Se pierden si el servidor se reinicia | Mejora planificada: guardarlas en la base de datos, que sí es permanente |

## Si prefieres no crear cuentas todavía

Se puede seguir usando la máquina de esta sesión mientras esté encendida, pero
su enlace **no es fijo** y además exige un token que solo tiene la plataforma:
no sirve para compartir. Para algo que puedas mandar a cualquiera hacen falta
los dos pasos de arriba.
