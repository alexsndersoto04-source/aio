# Paquetes de ejemplo

`hola-zett/` es el paquete que está publicado en el registro público (`registro/`) para
poder comprobar que `zett fetch` funciona de punta a punta. No hace nada útil: solo
devuelve un saludo.

La clave con la que se firmó no está en el repositorio (una clave privada nunca se sube).
Por eso solo quien la tenga puede publicar nuevas versiones de `hola-zett`; para tu propio
paquete usa tu propia clave (`zett keygen`). Cómo publicar: [`docs/ZETT.md`](../../docs/ZETT.md).
