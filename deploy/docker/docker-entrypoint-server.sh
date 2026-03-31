#!/bin/sh
set -eu

DATA_DIR="${DATA_DIR:-/home/tdrive/data}"
CACHE_DIR="${CACHE_DIR:-${DATA_DIR}/cache}"

mkdir -p "$DATA_DIR" "$CACHE_DIR"
chown -R tdrive:tdrive "$DATA_DIR" "$CACHE_DIR"

exec su -s /bin/sh -c "telegram-drive-server" tdrive
