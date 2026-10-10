// Ejecuta main() de un .wasm sin importaciones y escribe el resultado como entero.
// Para los que sí importan (navegador) solo comprueba WebAssembly.validate.
const fs = require("fs");
const bytes = fs.readFileSync(process.argv[2]);
if (!WebAssembly.validate(bytes)) { console.log("INVALID"); process.exit(1); }
const mod = new WebAssembly.Module(bytes);
if (WebAssembly.Module.imports(mod).length > 0) { console.log("VALID"); process.exit(0); }
const inst = new WebAssembly.Instance(mod, {});
console.log("=> " + inst.exports.main().toString());
