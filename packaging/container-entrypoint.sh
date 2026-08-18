#!/bin/sh
set -eu

: "${EASOS_HOME:=/var/lib/easos}"

/opt/easos/install.sh \
  --root "$EASOS_HOME" \
  --source-bin /opt/easos/bin \
  --bin-dir /usr/local/bin

exec "$EASOS_HOME/plugins/kernel/bin/easos-kerneld" --root "$EASOS_HOME"
