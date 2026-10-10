# Registro público de Zett

Esta carpeta **es** el registro de paquetes de Zett. GitHub la sirve como ficheros, así que
no hace falta ningún servidor.

```
v1/packages/<nombre>                índice JSON del paquete (versiones, SHA-256, firma)
archivos/<nombre>-<versión>.tpkg    el paquete firmado (tar.gz)
```

Dirección por defecto de `zett fetch`:
`https://raw.githubusercontent.com/alexsndersoto04-source/aio/main/registro`

**No edites estos ficheros a mano**: el índice lleva el hash y la firma de cada archivo.
Para añadir una versión usa `titan run selfhost/registro_agregar.titan PROYECTO CLAVE registro`
y abre un pull request. Guía completa: [`docs/ZETT.md`](../docs/ZETT.md).

Paquetes publicados: `hola-zett` (ejemplo, ver [`examples/paquetes/`](../examples/paquetes/)).
