#!/bin/sh
# 构建当前源码并安装 weft CLI 与桌面端到 /usr/bin。
set -eu
cd "$(dirname "$0")/.."

# 必须走 tauri build：裸 cargo build 会沿用 devUrl，打出来的二进制去连 localhost:1420。
# beforeBuildCommand 里已包含 yarn prepare:cli 与 yarn build（staged sidecar + 前端 dist）。
yarn tauri build --no-bundle

sudo install -m755 target/release/weft target/release/weft-desktop /usr/bin/
sudo install -Dm644 src-tauri/icons/icon.png /usr/share/icons/hicolor/512x512/apps/weft.png
sudo install -Dm644 /dev/stdin /usr/share/applications/weft.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=weft
Exec=weft-desktop
Icon=weft
Terminal=false
Categories=Utility;
EOF

# distrobox 里容器的 /usr 宿主机看不到：把二进制路径外的入口与图标同步到宿主机用户目录。
# distrobox-export 只在图标缺失时才拷贝，所以直接覆盖，否则改了图标永远不生效。
if [ -d /run/host ]; then
  host_share="/run/host$HOME/.local/share"
  rm -f "$host_share"/icons/hicolor/*/apps/weft*.png
  for name in weft weft-desktop; do
    install -Dm644 src-tauri/icons/icon.png "$host_share/icons/hicolor/512x512/apps/$name.png"
  done
  distrobox-host-exec kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
  distrobox-host-exec systemctl --user restart plasma-plasmashell.service >/dev/null 2>&1 || true
fi
