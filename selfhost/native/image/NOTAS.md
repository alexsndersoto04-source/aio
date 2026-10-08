# Imágenes: integradas en el runtime nativo de selfhost

`selfhost/native/runtime.titan` ya importa estos módulos. Los ejecutables nativos
creados con ese runtime incluyen las 21 funciones `std::image` y el contador de
`selfhost/native/cobertura.sh` las reconoce. La VM de Rust conserva su lector
independiente y se usa como referencia para comparar las imágenes válidas.

## Implementado y probado

Las pruebas están en `selfhost/native/image/verificar.sh`. Cada caso compila un
ejecutable nativo con el `zett` precompilado y compara su salida con la VM. Las
pruebas de entradas malformadas comprueban explícitamente los errores esperados.
No se usa Cargo.

- `image_base`: registro de imágenes, metadatos, cierre repetido y rechazo de
  identificadores cerrados.
- `image_pixels`: giros, reflejos, grises, brillo y recorte.
- `image_resample`: los cinco filtros, ajuste de tamaño, miniatura y desenfoque.
  El desenfoque conserva el cálculo separable y el redondeo Q15 de `image`
  0.25.10.
- `image_png`: lectura PNG con paletas, transparencias, 16 bits e intercalado
  Adam7; además revisa entradas dañadas y entradas que la VM sí acepta.
- `image_bmp`: lectura BMP con cabeceras core/info/V2/V3/V4/V5, paletas de
  1/2/4/8 bits, RGB de 16/24/32 bits, filas invertidas, máscaras y RLE4/RLE8.
  Las pruebas comparan carga por ruta y por bytes, giros, grises y BMP mal
  formados; incluyen deltas RLE horizontales y verticales.
- `image_jpeg`: lectura por ruta y por bytes de JPEG baseline, secuencial
  extendido de 8 bits y progresivo; incluye gris, RGB, CMYK/YCCK, JPEG
  secuencial de dos componentes (sin y con submuestreo horizontal 2:1), tablas
  de cuantización de 16 bits, marcadores de reinicio y muestreo vertical 1:3.
  También comprueba que Titan rechace los casos que la VM no admite: muestreo
  horizontal 3, una razón vertical fraccionaria y un proceso SOF no admitido.
  El resultado de los píxeles se compara con la VM.
- `image_gif`: lectura GIF87a/89a del primer cuadro, con tablas globales y
  locales, extensiones, transparencia, entrelazado, cuadros con desplazamiento y
  animaciones (se lee el primer cuadro, igual que `std::image::load` en la VM).
  El lector LZW iguala los casos que la VM acepta: tamaño mínimo 1, sin código
  inicial ni código final, píxeles sobrantes e índices fuera de la tabla (negro
  transparente); también rechaza GIF sin tabla o sin datos. El escritor admite
  RGB8/RGBA8, mantiene exactos hasta 256 colores y usa NeuQuant por encima de
  ese límite. Se compara el color y la VM vuelve a abrir los GIF creados por Titan.
- WebP: escritura VP8L real y lectura de archivos RIFF/VP8L reales. El lector
  admite árboles Huffman con varios grupos, referencias atrás, caché de colores
  y transformaciones VP8L (predictor, color, resta de verde e índices de color).
  Las tablas Huffman se dimensionan según la longitud máxima de sus códigos;
  se elimina el tope arbitrario de 32 grupos y se descartan las tablas de grupos
  que el mapa no usa. Las tablas persistentes tienen un límite de 64 MiB.
  Cuatro archivos permanentes —normal, con alfa, meta de un grupo y meta de
  varios grupos— se comparan con la VM por ruta y por bytes; también se verifica
  con ambos lectores la salida escrita por Titan.
  La lectura VP8 con pérdida admite cuadros clave estáticos, segmentación,
  filtro simple y normal, varias particiones de datos y alfa sin comprimir o
  comprimida. Once archivos cubren estos casos, bloques y bordes impares; los
  resultados y las cargas por ruta y por bytes se comparan con la VM. La prueba
  de WebP animado decodifica el primer cuadro por ruta y por bytes; además
  construye cuadros VP8L/VP8 con alfa y desplazamiento para contrastar la
  composición con la VM. También cubre que VP8X determine el tipo de color y
  el presupuesto de bytes aunque discrepe el bit alfa de VP8L, además de un
  árbol Huffman simple con símbolos duplicados (permitido por RFC 9649). Ambas
  regresiones nuevas siguen pendientes de ejecución con el seed precompilado.
  Las pruebas de rechazo cubren fragmentos
  RIFF dañados, cuadros VP8 y modos que no se admiten, árboles Huffman
  incompletos y límites de dimensiones y memoria; comprueban los errores
  observados en la VM.

Las fixtures WebP de límite miden 8192 × 2049 píxeles y se generaron con
ImageMagick 6.9.11-60 y libwebp 1.2.4. El RGB8 final requiere 50 356 224 bytes
(48 MiB + 24 KiB) y debe aceptarse; el RGBA8 final requiere 67 141 632 bytes,
32 768 sobre 64 MiB, y debe rechazarse. La matriz diferencial prueba esos dos
resultados para lossless VP8L, VP8 con pérdida y, para RGB8, una animación VP8L
cuyo primer cuadro se carga como imagen fija. Además compara en VM y nativo los
hashes BMP de píxeles muestreados en las esquinas y el centro.

- `testdata/webp-oversized-lossless-rgb.webp`: VP8L opaca, 706 bytes; SHA-256
  `3d2ecf80a33f36307c3753227150e1f50b4f4fba13897fe563b8165dfa6979a5`.
- `testdata/webp-oversized-lossless.webp`: VP8L con alfa real, 708 bytes; SHA-256
  `7b3ad51f73b161f912c6257cdaf1dbd7d8d5912f902c0706a408381a90d84ed3`.
- `testdata/webp-oversized-lossy-rgb.webp`: VP8 opaca, 30 144 bytes; SHA-256
  `b4370248d2261e2bf5a9e548c2de63c4072384287e9c1b9d4316ba1d73bc986b`.
- `testdata/webp-oversized-lossy-alpha.webp`: VP8 con alfa, 30 852 bytes; SHA-256
  `1a85c6c23e931807c642618d77508cf11aa31fe1fc81afedbae944c5c55427da`.
- `testdata/webp-oversized-animation-rgb.webp`: VP8X/ANIM con dos cuadros VP8L,
  opaca, 1 488 bytes; SHA-256
  `52e1b669a929ba541ec8973a94710c64a75341af41757b2647cad0eb8e601963`. Conserva
  exactamente el tamaño RIFF declarado por el encoder.

Una cabecera VP8L sobre el límite sin flujo de bits es un caso separado: la VM
reporta primero el flujo WebP corrupto.
- `image_io`: `encode` y `save` para PNG, BMP y JPEG. El escritor JPEG coincide
  con la VM en imágenes grises, RGB, con transparencia y de 16 bits convertidas
  al formato que acepta JPEG; también se vuelve a leer la salida escrita. `GIF`
  tiene pruebas específicas en `image_gif`.

La FDCT del escritor JPEG es un port Titan de `jfdctint.c`; sus condiciones y
aviso de origen están en `IJG-NOTICE.txt`. El cuantizador NeuQuant del escritor
GIF conserva la atribución y licencia de `color_quant` en
`COLOR-QUANT-NOTICE.txt`.

La suite completa se ejecuta así:

```sh
PATH="$HOME/.local/bin:$PATH" bash selfhost/native/image/verificar.sh
```

Para probar solo una parte, por ejemplo JPEG:

```sh
PATH="$HOME/.local/bin:$PATH" IMAGE_TESTS=image_jpeg bash selfhost/native/image/verificar.sh
```

Referencias: `crates/titan_stdlib/src/image_mod.rs`,
`crates/titan_vm/src/native.rs`, `selfhost/fuentes/image-0.25.10.crate`,
`selfhost/fuentes/png-0.18.1.crate`, `gif` 0.14.2, `color_quant` 1.1.0,
`image-webp` 0.2.4 y el código BMP de `image` 0.25.10.
