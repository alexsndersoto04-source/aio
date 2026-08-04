# 🗺️ TIERRAS DE TITAN

Un **juego de aventura y exploración de mundo** escrito en TITAN. Cada partida
genera un mundo **distinto** (bosques, montañas, lagos, desiertos, un pueblo)
y tú eres un explorador que camina, descubre el mapa y busca los 3 Tesoros
Legendarios escondidos.

## Cómo jugar (en Termux)

```bash
titan run game/aventura.titan
```

**Controles:**
- `w / a / s / d` → moverte
- `q` → salir
- `r` → ayuda

**Objetivo:** encuentra los 3 Tesoros Legendarios (marcados con `*`) para
ganar. Una brújula te indica hacia dónde queda el tesoro más cercano.

## Qué hace "bien hecho"

- **Mundo procedural:** cada partida tiene una semilla aleatoria, así que
  nunca exploras el mismo mundo dos veces.
- **Biomas con color:** llanuras (verde), bosques (`&`), montañas (`^`, no
  transitables), lagos (`~`, no transitables) y desiertos (`:`).
- **Niebla de exploración:** lo que aún no has visto aparece oscurecido;
  el mundo se "descubre" a medida que caminas.
- **Tesoros escondidos:** brújula que te guía por distancia y dirección.
- **Teclado en tiempo real:** `std::term::read_key` lee las teclas sin
  necesidad de pulsar Enter (se juega fluido en Termux).

## Cómo está hecho (sin humo)

Usa funciones de TITAN ya confirmadas en el repo:
`std::term::read_key`, `std::term::print_colored`, `std::term::print_styled`,
`std::term::clear_screen`, `std::random::int`, `std::map::get/insert/new`,
`std::try::catch`, `std::time::sleep_ms`.

> **Nota:** no hay variable global mutable en TITAN (todo estado vive dentro
> de `main()` con `let mut`), igual que en `examples/game_engine.titan`. Por
> eso el juego es un solo archivo con el bucle dentro de `main()`.
