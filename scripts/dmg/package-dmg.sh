#!/bin/bash
# ==============================================================
#  将 .app 与「修复启动损坏」解锁脚本、损坏修复说明一起打成 dmg。
#
#  Tauri 自带的 dmg 打包无法塞入额外文件，这里改为：
#  只让 Tauri 产出 .app，再用 hdiutil 把
#     <应用>.app + Applications 软链 + 解锁脚本 + 说明
#  一起压缩成 dmg。CI 与本地均可使用：
#
#     scripts/dmg/package-dmg.sh <版本号> <架构 arm64|x64> [.app 路径]
# ==============================================================
set -euo pipefail

version="${1:?用法: scripts/dmg/package-dmg.sh <版本号> <架构> [.app 路径]}"
arch="${2:?用法: scripts/dmg/package-dmg.sh <版本号> <架构> [.app 路径]}"
app_src="${3:-}"
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"

target="aarch64-apple-darwin"
if [ "$arch" = "x64" ]; then target="x86_64-apple-darwin"; fi

if [ -z "$app_src" ]; then
  app_src=$(ls -d "$repo_root"/src-tauri/target/"$target"/release/bundle/macos/*.app 2>/dev/null | head -n 1 || true)
fi
if [ -z "$app_src" ] || [ ! -d "$app_src" ]; then
  echo "错误：未找到 .app 包。请先执行 npm run macos:app（或 macos:app:x64）。" >&2
  exit 1
fi

stage="$repo_root/dmg-stage"
dist="$repo_root/dist"
dmg="$dist/ActaHandy-${version}-macos-${arch}.dmg"

rm -rf "$stage"
mkdir -p "$stage" "$dist"

cp -R "$app_src" "$stage/"
ln -s /Applications "$stage/Applications"
cp "$repo_root/scripts/dmg/修复启动损坏.command" "$stage/"
cp "$repo_root/scripts/dmg/损坏修复说明.txt" "$stage/"
chmod +x "$stage"/*.command

# 打好的 dmg 也清掉隔离标记：上传到 Release 后由用户下载时系统会再打一次，
# 但本地/CI 环节残留的标记会让 dmg 自己带进包内，先清干净。
rm -f "$dmg"
hdiutil create -volname "Acta Handy" -srcfolder "$stage" -format UDZO -ov "$dmg"
xattr -r -d com.apple.quarantine "$dmg" 2>/dev/null || true
rm -rf "$stage"

echo "已生成: $dmg"
echo "包含: $(basename "$app_src")、Applications 软链、修复启动损坏.command、损坏修复说明.txt"
