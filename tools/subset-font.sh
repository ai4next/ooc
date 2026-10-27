#!/usr/bin/env bash
# 从完整 Noto Sans SC 生成只含"源码里出现过"的字符的子集。
#
# 源字体不提交仓库。从 Google Fonts 或 Noto CJK 获取 Noto Sans SC Regular
# 的 TTF / OTF 均可。依赖：
#   python3 -m venv .venv && .venv/bin/pip install fonttools
#
# 用法：
#   PATH="$PWD/.venv/bin:$PATH" tools/subset-font.sh /path/to/NotoSansSC-Regular.ttf
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="${1:?usage: subset-font.sh <source.otf>}"
out="$root/assets/fonts/NotoSansSC-Subset.otf"
charset="$root/assets/fonts/charset.txt"

python3 "$root/tools/charset.py" "$root" > "$charset"

count=$(python3 -c "print(len(open('$charset', encoding='utf-8').read()))")
echo "charset: $count unique chars"

python3 -m fontTools.subset "$src" \
  --text-file="$charset" \
  --output-file="$out" \
  --layout-features='*' \
  --name-IDs='*' \
  --no-hinting \
  --drop-tables+=DSIG

ls -lh "$out"
