#!/bin/sh
# 构建当前源码并安装 weft CLI 与桌面端到 /usr/bin。
set -eu
cd "$(dirname "$0")/.."

# 必须走 tauri build：裸 cargo build 会沿用 devUrl，打出来的二进制去连 localhost:1420。
# beforeBuildCommand 里已包含 yarn prepare:cli 与 yarn build（staged sidecar + 前端 dist）。
yarn tauri build --no-bundle

sudo install -m755 target/release/weft target/release/weft-desktop /usr/bin/
