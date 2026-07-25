# Phase 1 — Extra Standard-Library Modules

These modules extend the Titan standard library with real, non-simulated
functionality backed by well-established Rust crates. Every function is
reachable from `.titan` code via the native registry (prefix `std::`).

All modules are enabled by default (Cargo feature `extras`). Disable them
individually or all at once when you need a leaner `zett` binary — see
[Building on Termux](#building-on-termux).

## Modules

| `.titan` prefix   | Backing crate              | What it gives you                                       |
|-------------------|----------------------------|---------------------------------------------------------|
| `std::regex::*`   | `regex`                    | Unicode regex: match, find, captures, replace, split    |
| `std::uuid::*`    | `uuid`                     | UUID v4 (random) and v7 (time-ordered)                  |
| `std::hash::*`    | `sha2`, `sha3`, `blake3`, `hmac` | SHA-256/384/512, SHA-3, BLAKE3, HMAC-SHA-256/512     |
| `std::random::*`  | `rand`, `rand_chacha`      | OS-seeded RNG + deterministic ChaCha20 helpers          |
| `std::datetime::*`| `chrono`                   | Now, format, RFC 3339/2822, parse, field access, offsets|
| `std::url::*`     | `url`                      | Parse/build URLs and `application/x-www-form-urlencoded`|
| `std::dirs::*`    | `dirs`                     | User HOME, config, cache, downloads, Termux-friendly    |

Full API surface: see the `NATIVES` table in
`crates/titan_stdlib/src/native.rs` (search for `Phase 1`).

## Example (`examples/extras.titan`)

```titan
fn main() {
    let emails = std::regex::find_all(
        "[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+",
        "escribe a juan@ejemplo.com o a maria@dev.io"
    )
    print("emails: {emails}")

    print("uuid v7: {std::uuid::v7()}")
    print("sha256:  {std::hash::sha256(std::encoding::utf8_encode(\"hola mundo\"))}")
    print("dado:    {std::random::seeded_int(42, 1, 6)}")
    print("now UTC: {std::datetime::now_iso()}")
    print("Caracas: {std::datetime::format_offset(std::datetime::now(), \"%H:%M\", -240)}")
    print("host:    {std::url::host(\"https://arena.ai/agents?x=1\")}")
    print("temp:    {std::dirs::temp()}")
}
```

Run it once you rebuild:

```bash
cargo run -p titan_cli -- run examples/extras.titan
```

## Building on Termux

```bash
pkg install rust git clang pkg-config make

git clone https://github.com/alexsndersoto04-source/aio
cd aio

# Full feature set (default) — this is what `pkg install zett` should ship.
cargo test --workspace --all-targets
cargo build --release -p titan_cli

# Rebuild the .deb (this is what `make-zett-package.sh` does):
./make-zett-package.sh          # produces zett_0.2.0_arm.deb
```

If a specific extra pulls in a dependency you don't want on-device,
you can strip it out:

```bash
cargo build --release -p titan_cli --no-default-features                      # nothing extra
cargo build --release -p titan_cli --no-default-features --features regex_mod # just regex
```

## Testing what you added

```bash
# Run every unit test in the workspace
cargo test --workspace

# Only the new bindings (VM ↔ stdlib integration)
cargo test -p titan_vm -- regex_native_bindings
cargo test -p titan_vm -- uuid_native_bindings
cargo test -p titan_vm -- hash_native_bindings
cargo test -p titan_vm -- random_native_bindings
cargo test -p titan_vm -- datetime_native_bindings
cargo test -p titan_vm -- url_native_bindings
cargo test -p titan_vm -- dirs_native_bindings
```

## Adding another module (recipe)

1. Add the crate to `crates/titan_stdlib/Cargo.toml` as `optional = true` and
   register a matching `feature = ["dep:..."]`.
2. Write `crates/titan_stdlib/src/<name>_mod.rs` with real functions and
   `#[cfg(test)] mod tests`.
3. Gate the module in `crates/titan_stdlib/src/lib.rs` with
   `#[cfg(feature = "<name>_mod")] pub mod <name>_mod;`.
4. Register every function in `crates/titan_stdlib/src/native.rs`'s
   `NATIVES` table with a `native!` invocation.
5. Add the dispatch arm(s) in `crates/titan_vm/src/native.rs` guarded by
   `#[cfg(feature = "<name>_mod")]`.
6. Forward the feature in `titan_vm/Cargo.toml` and `titan_cli/Cargo.toml`.
7. Add a smoke test in `crates/titan_vm/src/native.rs`'s
   `#[cfg(test)] mod tests`.
