# Cómo colaborar

Gracias por querer ayudar. Esta guía explica cómo preparar el entorno, qué
comprobar antes de proponer un cambio y qué reglas sigue el proyecto.

## Preparar el entorno

Necesitas Linux x86-64, `bash` y `gzip`:

```bash
git clone https://github.com/alexsndersoto04-source/aio.git
cd aio
bash selfhost/bootstrap.sh      # construye selfhost/titan (2–3 minutos)
```

Para probar el generador LLVM o ARM64 hacen falta además `clang` y `lld`.
El mapa de las carpetas está en [`selfhost/README.md`](selfhost/README.md).

## Antes de proponer un cambio

| Si tocas… | Comprueba |
|---|---|
| El compilador (`selfhost/*.titan`) | `bash selfhost/verify_fixpoint.sh`: el compilador debe seguir compilándose a sí mismo con resultado idéntico |
| La biblioteca (`selfhost/native/`) | `bash selfhost/native/cobertura.sh -v` y un programa que use lo que cambiaste |
| Ejemplos o proyectos | `selfhost/titan check <archivo>` |
| La web (`site-src/`) | `python3 site-src/generar.py claro` y revisar `site/index.html` en el navegador |
| Moon | `selfhost/titan check projects/moon/src/main.titan` |

La integración continua (GitHub Actions) repite estas comprobaciones en cada
cambio, y el flujo ARM64 las prueba en una máquina ARM64 real.

## Reglas del proyecto

1. **Nada simulado.** Una función hace lo que dice o no existe. Si algo no se
   puede implementar de verdad, se elimina; no se deja un sustituto falso.
2. **Solo verdad en la documentación.** Cada cifra o afirmación debe poder
   comprobarse con un comando del repositorio. Lo no probado se declara como no
   probado (por ejemplo, ARM64 en un teléfono real).
3. **El estado vive en [`selfhost/ESTADO.md`](selfhost/ESTADO.md).** Si cambias
   algo verificable, actualiza ese archivo.
4. **Español** en la documentación y en los mensajes para quien usa Titan.
5. **Sin binarios nuevos en git**, salvo la semilla de `selfhost/semilla/` (si
   cambia, hay que actualizar su SHA-256).

## Escribir código Titan: trampas conocidas

- `go`, `first` y `last` dan problemas como nombres de variable.
- No hay conversión automática de `int` a `float`.
- Un `match` sobre un `Result` debe cubrir `Ok` y `Err` (o `_`).
- `"{a.b}"` no se evalúa dentro de un texto: guarda el valor en una variable.
- Concatenar textos o arrays dentro de un bucle largo es cuadrático.

## La página oficial

Se genera con `python3 site-src/generar.py claro` desde `site-src/` (plantillas,
estilos y ejemplos con su salida real) y se publica desde `site/` con GitHub
Pages al llegar a `main`. No edites `site/` a mano. Los ejemplos de
`site-src/ejemplos/` deben ejecutarse de verdad y mostrar su salida real.

## Proponer el cambio

1. Trabaja en una rama.
2. Haz commits pequeños con un mensaje que explique el porqué.
3. Abre un Pull Request y describe qué cambia, cómo lo probaste y qué no pudiste probar.

Para reportar un fallo, abre un [issue](https://github.com/alexsndersoto04-source/aio/issues)
con el programa mínimo que lo reproduce, lo que esperabas y lo que ocurrió.

## Licencia

Al colaborar aceptas que tu aporte se distribuya bajo la licencia [MIT](LICENSE).
