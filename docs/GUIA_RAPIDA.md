# Guía rápida de Titan

Una vuelta completa por el lenguaje, en unos 20 minutos. Cada programa de esta
guía está en [`examples/guia/`](../examples/guia) y se puede ejecutar tal cual.
Las salidas que se muestran son las reales (se ejecutaron con el compilador
construido desde este repositorio).

**Antes de empezar:** necesitas la herramienta `titan`. Se construye con
`bash selfhost/bootstrap.sh` (ver [Instalación](../README.md#instalación)).

## 1. Variables y tipos

```titan
fn main() {
    let nombre = "Titan"
    let mut contador = 0
    contador += 1
    let pi = 3.14159
    let activo = true
    print("{nombre} {contador} {pi} {activo}")
}
```

```bash
titan run examples/guia/01_variables.titan
```

```text
Titan 1 3.14159 true
```

- `let` crea una variable que **no** cambia; `let mut` crea una que sí.
- Los tipos básicos son `int`, `float`, `string` y `bool`. El compilador los
  deduce, pero se pueden escribir: `let n: int = 3`.
- Dentro de un texto, `{...}` inserta el valor de una variable o de cualquier
  expresión (`"{nombre}"`, `"{contador + 1}"`, `"{p.x}"`, `"{factorial(5)}"`).
- Un `int` no se convierte solo en `float`: `1 + 0.5` es un error de tipos.

## 2. Funciones, condiciones y bucles

```titan
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    n * factorial(n - 1)
}

fn main() {
    let mut total = 0
    for i in 1..=5 {
        total += factorial(i)
    }
    print("suma de factoriales = {total}")

    let mut n = 10
    while n > 0 {
        n -= 4
    }
    print("n = {n}")
}
```

```text
suma de factoriales = 153
n = -2
```

- La última expresión de una función es su resultado (no hace falta `return`).
- `1..=5` incluye el 5; `1..5` no lo incluye.
- Los parámetros llevan su tipo y el resultado se declara con `->`.

## 3. Arrays, mapas y closures

```titan
fn main() {
    let numeros = [3, 1, 4, 1, 5]
    let dobles = numeros.map(|x| x * 2)
    print("dobles: {dobles}")
    let pares = numeros.filter(|x| x % 2 == 0)
    print("pares: {pares}")

    let edades = std::map::new()
    let edades = std::map::insert(edades, "ana", 31)
    let edades = std::map::insert(edades, "luis", 28)
    let ana = std::map::get(edades, "ana")
    print("ana tiene {ana} años")
}
```

```text
dobles: [6, 2, 8, 2, 10]
pares: [4]
ana tiene 31 años
```

- `|x| x * 2` es una *closure*: una función pequeña sin nombre.
- Los mapas se manejan con `std::map::*`; cada `insert` devuelve el mapa nuevo.

## 4. Structs, métodos y traits

```titan
trait Saludo {
    fn nombre(self) -> string;
    fn saludar(self) -> string {
        "Hola, " + self.nombre() + "!"
    }
}

struct Persona { nombre: string, edad: int }

impl Persona {
    fn nueva(nombre: string, edad: int) -> Persona {
        Persona { nombre: nombre, edad: edad }
    }
    fn es_mayor(self) -> bool {
        self.edad >= 18
    }
}

impl Saludo for Persona {
    fn nombre(self) -> string {
        self.nombre
    }
}

fn main() {
    let p = Persona::nueva("Ana", 31)
    print(p.saludar())
    let mayor = p.es_mayor()
    print("mayor de edad: {mayor}")
}
```

```text
Hola, Ana!
mayor de edad: true
```

- `struct` agrupa campos; `impl` le añade métodos. Los que no llevan `self` se
  llaman con `Tipo::nombre(...)`.
- Un `trait` es un contrato: lista métodos que un tipo debe tener. Puede traer
  métodos con cuerpo por defecto (`saludar`).

## 5. Enums y `match`

```titan
enum Forma {
    Circulo(float),
    Cuadrado(float),
    Vacio,
}

fn area(f: Forma) -> float {
    match f {
        Forma::Circulo(r) => 3.14159 * r * r,
        Forma::Cuadrado(l) => l * l,
        Forma::Vacio => 0.0,
    }
}
```

`match` revisa que cubras todos los casos: si falta uno, el compilador lo avisa
antes de ejecutar. El programa completo está en
[`05_enums_match.titan`](../examples/guia/05_enums_match.titan).

## 6. Errores: `Result` y el operador `?`

```titan
fn dividir(a: int, b: int) -> Result {
    if b == 0 {
        return Result::Err("división por cero")
    }
    Result::Ok(a / b)
}

fn calcular() -> Result {
    let x = dividir(100, 5)?
    let y = dividir(x, 0)?
    Result::Ok(y)
}

fn main() {
    match calcular() {
        Result::Ok(v) => print("resultado {v}"),
        Result::Err(e) => print("error: {e}"),
    }
    let r = std::try::catch(|| std::fs::read_text("/no/existe.txt"))
    match r {
        Result::Ok(t) => print("leído"),
        Result::Err(m) => print("no se pudo leer el archivo"),
    }
}
```

```text
error: división por cero
no se pudo leer el archivo
```

- `?` devuelve el error al que llamó a la función en cuanto aparece uno.
- `std::try::catch(|| ...)` convierte en `Result` un fallo de la biblioteca (por
  ejemplo, un archivo que no existe) para que el programa no termine.

## 7. Tareas y canales

```titan
fn productor(tx: Sender, n: int) -> int {
    let mut i = 1
    while i <= n {
        send(tx, i * i)
        i += 1
    }
    n
}

fn main() {
    let (tx, rx) = channel(2)
    let tarea = spawn || { productor(tx, 5) }
    let mut suma = 0
    let mut k = 0
    while k < 5 {
        suma += recv(rx)
        k += 1
    }
    print("suma de cuadrados = {suma}")
    print("la tarea envió {join(tarea)} valores")
}
```

```text
suma de cuadrados = 55
la tarea envió 5 valores
```

`spawn` lanza una tarea, `channel(n)` crea un canal con capacidad `n`, y `join`
espera el resultado. Las tareas son fibras cooperativas: se turnan en un solo
núcleo. Detalle: [`CONCURRENCY.md`](CONCURRENCY.md).

## 8. Usar la biblioteca estándar

```titan
fn main() {
    let doc = std::json::parse("{\"nombre\": \"Titan\", \"version\": 1}")
    print(doc.nombre)
    let datos = std::encoding::utf8_encode("hola")
    print("base64 = {std::encoding::base64_encode(datos)}")
    print("sha256 = {std::hash::sha256(datos)}")
}
```

```text
Titan
base64 = aG9sYQ==
sha256 = b221d9dbb083a7f33428d7c2a3c3198ae925614d70210e28716ccaa7cd4ddb79
```

Las funciones se llaman con su ruta completa (`std::json::parse`). La lista
completa está en [`STDLIB.md`](STDLIB.md).

## 9. Del programa al ejecutable

```bash
titan check examples/guia/02_control.titan                  # solo comprueba tipos
titan run examples/guia/02_control.titan                    # compila y ejecuta
titan compile examples/guia/02_control.titan -o factoriales # deja un ejecutable
./factoriales
titan wasm examples/guia/02_control.titan                   # módulo WebAssembly
```

`titan run` y `titan exec` capturan la salida del programa para imprimirla al
terminar (y le dan `/dev/null` como entrada), así que un programa que use la
terminal —prompts de `std::readline`, teclas de `std::term`, barras de
`std::progress`— necesita `--tty`:

```bash
titan run --tty ejemplos/con_prompt.titan
titan exec --tty programa.tbc
```

Con `--tty` el programa conserva la entrada, la salida y el error del proceso: no
hay captura, ni tope de 4 MiB, y las teclas y el tamaño de la ventana llegan
bien. `titan test` sigue capturando, para poder informar el detalle de cada
prueba.

El ejecutable de este ejemplo pesa unos 1,1 MB, no necesita nada instalado
para correr y arranca sin máquina virtual.

## 10. Tu primer proyecto

```bash
titan new mi_app
cd mi_app
titan run
```

`titan new` crea `Titan.toml` (nombre y dependencias) y `src/main.titan`. Para
dividir el código en varios archivos y escribir tests (`tests/*.titan`, que
ejecuta `titan test`), sigue [`PROJECTS.md`](PROJECTS.md).

## 11. Paquetes con Zett

`zett` es el gestor de paquetes de Titan (viene con `titan`; ver [`ZETT.md`](ZETT.md)):

```bash
cd mi_app
zett add demo "^1"     # añade [dependencies.demo] a Titan.toml
zett fetch             # descarga, verifica (SHA-256 y firma) e instala
```

`zett add` escribe esto en `Titan.toml`:

```toml
[dependencies.demo]
version = "^1"
```

`zett fetch` necesita un registro de paquetes por HTTPS; este repositorio trae el cliente, no el servidor.

## Qué leer después

| Si quieres… | Lee |
|---|---|
| Todas las construcciones del lenguaje | [`TITAN_SYNTAX.md`](TITAN_SYNTAX.md) |
| Las reglas exactas del lenguaje | [`SPEC.md`](SPEC.md) (en inglés) |
| Qué trae la biblioteca | [`STDLIB.md`](STDLIB.md) |
| Hacer un servidor web | [`NETWORKING.md`](NETWORKING.md) y `examples/webserver.titan` |
| Guardar datos | [`SQLITE.md`](SQLITE.md), [`POSTGRESQL.md`](POSTGRESQL.md) |
| Depurar | [`DEBUGGER.md`](DEBUGGER.md) |
| Usar y publicar paquetes | [`ZETT.md`](ZETT.md) |
| Ver una aplicación completa | [`projects/moon`](../projects/moon) |

## Consideraciones importantes

- **Tipado numérico estricto:** No hay conversión implícita entre `int` y `float` (`1 + 0.5` se rechaza en compilación).
- **Exhaustividad en `match`:** Un `match` sobre `Result` u `Option` debe cubrir todas las variantes (`Ok`/`Err`, `Some`/`None`) o incluir un comodín (`_`).
- **Interpolación de textos:** Dentro de `"{...}"` se puede evaluar cualquier expresión de Titan (`"{a + 1}"`, `"{p.x}"`, `"{f(x)}"`), cuidando que las llaves vayan pegadas a la expresión.
- **Genéricos `<T>`:** Puedes definir funciones, `struct<T>`, `enum<T>`, `type<T>` e `impl<T>` parametrizados por tipo (ver `examples/genericos.titan`).
- **Palabras reservadas:** `go` es palabra reservada (alias de `spawn`), por lo que no debe usarse como identificador de variable.
- El estado detallado de cada módulo está documentado en [`selfhost/ESTADO.md`](../selfhost/ESTADO.md).
