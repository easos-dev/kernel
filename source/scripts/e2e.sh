#!/bin/sh
set -eu

EASOS=${EASOS:-easos}
PLUGIN_SOURCE=${PLUGIN_SOURCE:-/easos/kernel/source/fixtures/clock}

"$EASOS" stop clock >/dev/null 2>&1 || true
"$EASOS" uninstall clock >/dev/null 2>&1 || true

test -L /usr/local/bin/easos
test "$(readlink /usr/local/bin/easos)" = "/easos/kernel/bin/easos"
test -x /easos/kernel/bin/easos
test -x /easos/kernel/bin/easos-kerneld

"$EASOS" install "$PLUGIN_SOURCE" | grep '"state": "installed"' >/dev/null
"$EASOS" config clock set timezone '"Asia/Tokyo"' | grep 'Asia/Tokyo' >/dev/null
"$EASOS" autostart clock enable | grep '"autostart": true' >/dev/null
"$EASOS" start clock | grep '"state": "running"' >/dev/null
"$EASOS" status clock | grep '"state": "running"' >/dev/null
"$EASOS" stop clock | grep '"state": "exited"' >/dev/null
"$EASOS" uninstall clock | grep '"id": "kernel"' >/dev/null

test ! -e /easos/clock
"$EASOS" install "$PLUGIN_SOURCE" | grep '"state": "installed"' >/dev/null
"$EASOS" uninstall clock >/dev/null
test ! -e /easos/clock
test ! -e /run/easos/kernel.sock.tmp
printf '%s\n' 'container e2e: ok'
