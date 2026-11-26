#!/bin/sh
# 构建当前源码并安装 weft CLI 与桌面端到 /usr/bin。
set -eu
cd "$(dirname "$0")/.."

yarn build
cargo build --release -p weft-cli -p weft-desktop

sudo install -m755 target/release/weft target/release/weft-desktop /usr/bin/
