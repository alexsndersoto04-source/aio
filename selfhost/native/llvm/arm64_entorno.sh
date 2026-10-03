#!/bin/bash
# Variables de entorno iguales en el runner x86-64 que crea las referencias y
# en el runner ARM64 que ejecuta los binarios. Se usa `env -i` para no heredar
# HOME, PATH, locale ni variables de Actions diferentes entre máquinas.
arm64_prepare_environment() {
  local root="$1"
  local base=/tmp/aio-arm64-ci
  mkdir -p "$base/home" "$base/config" "$base/cache" "$base/data" \
    "$base/state" "$base/runtime" "$base/tmp" /tmp/aio-arm64-case
  chmod 700 "$base/runtime"
  ARM64_TEST_ENV=(
    "HOME=$base/home"
    "USER=runner"
    "LOGNAME=runner"
    "SHELL=/bin/bash"
    "PATH=/usr/bin:/bin:/usr/sbin:/sbin"
    "PWD=$root"
    "GITHUB_WORKSPACE=$root"
    "TMPDIR=$base/tmp"
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
