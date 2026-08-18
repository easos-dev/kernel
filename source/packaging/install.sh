#!/bin/sh
set -eu

ROOT=/var/lib/easos
SOURCE_BIN=/opt/easos/bin
PLUGIN_TEMPLATE_DIR=/opt/easos/plugin
SERVICE_TEMPLATE=/opt/easos/easos-kernel.service
BIN_DIR=/usr/local/bin
ENABLE_SYSTEMD=0

usage() {
  printf '%s\n' 'Usage: install.sh [--root PATH] [--source-bin PATH] [--plugin-template-dir PATH] [--service-template PATH] [--bin-dir PATH] [--enable-systemd]'
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
    --plugin-template-dir)
      PLUGIN_TEMPLATE_DIR=$2
      shift 2
      ;;
    --service-template)
      SERVICE_TEMPLATE=$2
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

for required in manifest/main.json config/main.json config/state.json; do
  if [ ! -f "$PLUGIN_TEMPLATE_DIR/$required" ]; then
    printf 'Kernel template was not found: %s\n' "$PLUGIN_TEMPLATE_DIR/$required" >&2
    exit 1
  fi
done

KERNEL_DIR=$ROOT/kernel
install -d -m 0755 "$ROOT" "$KERNEL_DIR/manifest" "$KERNEL_DIR/bin" "$KERNEL_DIR/config" "$BIN_DIR"
install -m 0644 "$PLUGIN_TEMPLATE_DIR/manifest/main.json" "$KERNEL_DIR/manifest/main.json"
if [ ! -f "$KERNEL_DIR/config/main.json" ]; then
  install -m 0644 "$PLUGIN_TEMPLATE_DIR/config/main.json" "$KERNEL_DIR/config/main.json"
fi
if [ ! -f "$KERNEL_DIR/config/state.json" ]; then
  install -m 0644 "$PLUGIN_TEMPLATE_DIR/config/state.json" "$KERNEL_DIR/config/state.json"
fi

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
  if [ ! -f "$SERVICE_TEMPLATE" ]; then
    printf 'Systemd template was not found: %s\n' "$SERVICE_TEMPLATE" >&2
    exit 1
  fi
  SERVICE_FILE=/etc/systemd/system/easos-kernel.service
  sed \
    -e "s|@EASOS_HOME@|$ROOT|g" \
    "$SERVICE_TEMPLATE" > "$SERVICE_FILE"
  chmod 0644 "$SERVICE_FILE"
  systemctl daemon-reload
  systemctl enable --now easos-kernel.service
fi
