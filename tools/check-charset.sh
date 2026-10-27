#!/usr/bin/env bash
# 确保 assets/fonts/charset.txt 与源码一致。
# 不一致说明有人加了中文文案却没重新跑 subset-font.sh，字体里会缺字。
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
python3 "$root/tools/charset.py" "$root" > "$tmp"

if ! diff -q "$tmp" "$root/assets/fonts/charset.txt" >/dev/null; then
  echo "charset 已过期：源码里出现了新的字符。"
  echo "请重新生成字体子集："
  echo "  PATH=\"\$PWD/.venv/bin:\$PATH\" tools/subset-font.sh /path/to/NotoSansSC-Regular.otf"
  diff "$tmp" "$root/assets/fonts/charset.txt" | head -20 || true
  exit 1
fi
echo "charset OK"
