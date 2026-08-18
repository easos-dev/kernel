#!/bin/sh
set -eu

ROOT=/var/lib/easos
SOURCE_BIN=/opt/easos/bin
BIN_DIR=/usr/local/bin
ENABLE_SYSTEMD=0

usage() {
  printf '%s\n' 'Usage: install.sh [--root PATH] [--source-bin PATH] [--bin-dir PATH] [--enable-systemd]'
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --root)
      ROOT=$2
      shift 2
      ;;
    --source-bin)
      SOURCE_BIN=$2
      shift 2
      ;;
    --bin-dir)
      BIN_DIR=$2
      shift 2
      ;;
    --enable-systemd)
      ENABLE_SYSTEMD=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ ! -x "$SOURCE_BIN/easos" ] || [ ! -x "$SOURCE_BIN/easos-kerneld" ]; then
  printf 'Kernel binaries were not found in %s\n' "$SOURCE_BIN" >&2
  exit 1
fi

KERNEL_DIR=$ROOT/plugins/kernel
install -d -m 0755 "$KERNEL_DIR/bin" "$ROOT/config" "$ROOT/run/plugins" "$ROOT/logs" "$BIN_DIR"
install -m 0755 "$SOURCE_BIN/easos" "$KERNEL_DIR/bin/.easos.new"
install -m 0755 "$SOURCE_BIN/easos-kerneld" "$KERNEL_DIR/bin/.easos-kerneld.new"
mv -f "$KERNEL_DIR/bin/.easos.new" "$KERNEL_DIR/bin/easos"
mv -f "$KERNEL_DIR/bin/.easos-kerneld.new" "$KERNEL_DIR/bin/easos-kerneld"
ln -sfn "$KERNEL_DIR/bin/easos" "$BIN_DIR/easos"

if [ "$ENABLE_SYSTEMD" -eq 1 ]; then
  if ! command -v systemctl >/dev/null 2>&1; then
    printf '%s\n' 'systemctl is required for --enable-systemd' >&2
    exit 1
  fi
  SERVICE_FILE=/etc/systemd/system/easos-kernel.service
  sed \
    -e "s|@EASOS_HOME@|$ROOT|g" \
    "$(dirname "$0")/easos-kernel.service" > "$SERVICE_FILE"
  chmod 0644 "$SERVICE_FILE"
  systemctl daemon-reload
  systemctl enable --now easos-kernel.service
fi
