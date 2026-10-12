# Instalación de Titan en ARM de 32 bits (Termux)

Este documento describe cómo instalar Titan en dispositivos con Termux de 32 bits (armv7l/armv8l).

## Requisitos

- Termux instalado en Android
- Arquitectura: `armv7l` o `armv8l` (verificar con `uname -m`)
- Conexión a internet

## Instalación desde paquete .deb

### Paso 1: Descargar el paquete

```bash
wget https://github.com/alexsndersoto04-source/aio/releases/latest/download/zett_arm.termux.deb
```

### Paso 2: Instalar dependencias

```bash
pkg update
pkg install clang lld
```

### Paso 3: Instalar el paquete

```bash
pkg install ./zett_arm.termux.deb
```

### Paso 4: Verificar instalación

```bash
titan version
zett version
```

Deberías ver algo como:
```
TITAN Language Compiler v1.3.0
```

## Primer programa

Crea un archivo `hola.titan`:

```titan
fn main() {
    print("¡Hola desde Titan en ARM32!")
    
    let x = 42
    let y = 58
    print("Suma: {x} + {y} = {x + y}")
}
```

Ejecútalo:

```bash
titan run hola.titan
```

## Compilar a ejecutable nativo

```bash
titan compile hola.titan -o hola
./hola
```

## Limitaciones conocidas

- El bootstrap desde fuentes no funciona en ARM32 (la semilla es x86-64)
- Solo instalación desde paquetes .deb precompilados
- No probado en hardware real (solo en emulador hasta octubre 2026)

## Solución de problemas

### Error: "cannot locate symbol"

Actualiza todos los paquetes:
```bash
apt update && apt full-upgrade
```

### Error: "No command titan found"

Verifica que el paquete se instaló:
```bash
pkg list-installed | grep zett
```

Si no aparece, reinstala:
```bash
pkg install ./zett_arm.termux.deb
```

## Soporte

Si encuentras problemas en hardware ARM32 real, por favor reporta en:
https://github.com/alexsndersoto04-source/aio/issues

Incluye:
- Salida de `uname -m`
- Salida de `titan version`
- Descripción del problema
- Pasos para reproducir
