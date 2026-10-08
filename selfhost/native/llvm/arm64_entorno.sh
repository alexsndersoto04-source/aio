#!/bin/bash
# Variables de entorno iguales en el runner x86-64 que crea las referencias y
# en el runner ARM64 que ejecuta los binarios. Se usa `env -i` para no heredar
# HOME, PATH, locale ni variables de Actions diferentes entre máquinas.
arm64_prepare_environment() {
  local root="$1"
  local base=/tmp/aio-arm64-ci
  mkdir -p "$base/home" "$base/config" "$base/cache" "$base/data" \
    "$base/state" "$base/runtime" "$base/tmp"
  chmod 700 "$base/runtime"
  rm -rf /tmp/aio-arm64-case
  mkdir -p /tmp/aio-arm64-case
  python3 "$root/selfhost/tests/native/prepare_audio_tag_aliases.py" "$base/audio-tags-lossy"
  ARM64_TEST_ENV=(
    "HOME=$base/home"
    "USER=runner"
    "LOGNAME=runner"
    "SHELL=/bin/bash"
    "PATH=/usr/bin:/bin:/usr/sbin:/sbin"
    "PWD=$root"
    "GITHUB_WORKSPACE=$root"
    "TMPDIR=$base/tmp"
    "TITAN_AUDIO_TAGS_LOSSY_DIR=$base/audio-tags-lossy"
    "TITAN_AUDIO_TAGS_COVER_DIR=$base/audio-tags-cover"
    "XDG_CONFIG_HOME=$base/config"
    "XDG_CACHE_HOME=$base/cache"
    "XDG_DATA_HOME=$base/data"
    "XDG_STATE_HOME=$base/state"
    "XDG_RUNTIME_DIR=$base/runtime"
    "XDG_CONFIG_DIRS=/etc/xdg"
    "XDG_DATA_DIRS=/usr/local/share:/usr/share"
    "TERM=dumb"
    "CI=true"
    "LANG=C.UTF-8"
    "LC_ALL=C.UTF-8"
    "TZ=UTC"
  )
}

# La prueba de validación consume archivos escritos por el productor; ambos
# binarios deben recibir la misma ruta dentro de su ejecución de referencia.
arm64_case_group() {
  case "$1" in
    image_gif|image_gif_validate) printf 'gif\n' ;;
    image_webp|image_webp_validate) printf 'webp\n' ;;
    image_webp_write) printf 'webp-write\n' ;;
    image_io) printf 'image-io\n' ;;
    *) printf '%s\n' "$1" ;;
  esac
}

arm64_prepare_case_dir() {
  local base="$1"
  local group
  group="$(arm64_case_group "$base")"
  local path="/tmp/aio-arm64-case/$group"
  case "$base" in
    image_gif_validate|image_webp_validate)
      mkdir -p "$path"
      ;;
    *)
      rm -rf "$path"
      mkdir -p "$path"
      ;;
  esac
  printf '%s\n' "$path"
}
