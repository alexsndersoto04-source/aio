# Moon — acceso directo

`moon.html` es la aplicación completa en un solo archivo (interfaz + lógica),
apuntando al **servidor real** de esta sesión:

- API y base de datos: https://3000-ies7gd9idcl0bbg9nl9tn.e2b.app

Se abre desde cualquier teléfono o computadora, sin instalar nada. Se genera con:

```bash
cd frontend
VITE_API_URL="https://3000-ies7gd9idcl0bbg9nl9tn.e2b.app" node scripts/diseno-suelto.mjs --real --destino ../ver/moon.html
```

Es un archivo generado y temporal: vive mientras el espacio de trabajo esté
encendido. Para algo permanente (Render + Neon), ver `projects/moon/server/LEEME.md`.
