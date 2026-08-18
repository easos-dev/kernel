#!/bin/sh
set -eu

: "${EASOS_HOME:=/easos}"
: "${EASOS_RUNTIME_HOME:=/run/easos}"

/opt/easos/install.sh \
  --root "$EASOS_HOME" \
  --source-bin /opt/easos/bin \
  --plugin-template-dir /opt/easos/plugin \
  --bin-dir /usr/local/bin

exec "$EASOS_HOME/kernel/bin/easos-kerneld" \
  --root "$EASOS_HOME" \
  --runtime-root "$EASOS_RUNTIME_HOME"
