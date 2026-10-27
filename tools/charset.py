"""收集游戏运行时可能用到的字符：ASCII + 中文标点 + crates 下所有非 ASCII。"""

import pathlib
import sys


def collect(root: pathlib.Path) -> str:
    chars = {chr(c) for c in range(0x20, 0x7F)}
    chars.update("，。、；：！？（）《》「」『』【】…—·×←→“”‘’▲●")

    for path in sorted((root / "crates").rglob("*.rs")):
        for ch in path.read_text(encoding="utf-8"):
            if ord(ch) > 0x7E:
                chars.add(ch)

    return "".join(sorted(chars))


if __name__ == "__main__":
    print(collect(pathlib.Path(sys.argv[1])), end="")
