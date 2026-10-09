# La semilla

Un compilador que se compila a sí mismo necesita un primer compilador que ya exista. Esa es la
semilla: `titanc-linux-x86_64.gz` es el compilador nativo de Titan (Linux x86-64, ejecutable estático, sin
bibliotecas), comprimido (650 KB; descomprimido 7,7 MB). Su SHA-256 descomprimido está en
`titanc-linux-x86_64.sha256` (`f4943da3…bc3d`).

**De dónde sale.** Se construyó con la última versión del proyecto que todavía tenía Rust (etiqueta
`ultimo-con-rust`, commit `5dbb238`): la VM de Rust ejecutó `selfhost/build.titan` con un runtime «delgado»
(`native/semilla_lean.py`: sin SQLite, tokenize, ONNX, audio, imágenes, PostgreSQL ni MySQL, para caber en 4 GB)
y el resultado compiló el compilador completo tres veces seguidas con el mismo resultado (el punto fijo).
Es, byte a byte, el mismo `titanc3` que sale del punto fijo de esas fuentes.

**Cómo se usa.** `bash selfhost/bootstrap.sh` la desempaqueta, comprueba el hash, repite el punto fijo
(`titanc0 → titanc1 → titanc2 → titanc3`) y construye la CLI `selfhost/titan`. Para que la semilla sea
digna de confianza basta rehacer ese punto fijo: si las fuentes de este repositorio producen un
`titanc1 == titanc2 == titanc3`, el compilador que hay aquí se explica solo.

**Cuándo se actualiza.** Solo hace falta cambiar la semilla si el compilador pasa a usar algo que la semilla
actual no entiende (una instrucción nueva del lenguaje usada por el propio compilador). Entonces: compilar con
la semilla vieja una versión intermedia, y reemplazar el `.gz` y su `.sha256` con el `titanc3` nuevo.

**Límite.** Es un binario Linux x86-64. Para otras máquinas (macOS, Windows, ARM64/Termux) hay que
compilar desde Linux x86-64: el backend LLVM de `selfhost/build_llvm.titan` ya genera ARM64 (verificado en
hardware real en CI), pero el empaquetado de esas plataformas no está hecho.
