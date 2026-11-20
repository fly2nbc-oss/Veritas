"""Cuts the Lucide icons that Veritas draws out of the Lucide font.

Lucide (https://lucide.dev, ISC) ships every icon as a glyph of a TrueType font in the npm
package `lucide-static`. Veritas embeds only its own icons (`theme::icon`), as the subset
`assets/fonts/lucide-subset.ttf`. Version and package are the same as in Cerno.

Get the package (the SHA-1 is the `shasum` the npm registry lists for the version):

    curl -LO https://registry.npmjs.org/lucide-static/-/lucide-static-0.460.0.tgz
    tar -xzf lucide-static-0.460.0.tgz package/font/lucide.ttf package/font/info.json package/LICENSE

then, with fontTools (`pip install fonttools`):

    python tools/make_lucide_subset.py <package folder> assets/fonts

Writes `lucide-subset.ttf` and prints the name and code point of every icon. New icon: add
its name to ICONS, run the script, copy the code point into `theme::icon`
(`theme::tests::every_icon_is_in_the_subset` checks this).
"""

import json
import sys
from pathlib import Path

from fontTools import subset

VERSION = "0.460.0"
SHASUM = "3d6b70c74cc5e180ac773f684494eef770d95731"

# Lucide names as in `info.json`.
ICONS = [
    "check",
    "chevron-down",
    "chevron-right",
    "circle-alert",
    "circle-check",
    "cpu",
    "file-check",
    "file-json",
    "file-plus",
    "folder",
    "folder-tree",
    "hard-drive",
    "history",
    "info",
    "languages",
    "monitor",
    "moon",
    "search",
    "shield-check",
    "sun",
    "triangle-alert",
    "x",
]


def main(package: Path, out: Path) -> None:
    info = json.loads((package / "font" / "info.json").read_text(encoding="utf-8"))
    codes = {}
    for name in ICONS:
        entry = info.get(name)
        if entry is None:
            sys.exit(f"{name}: not in info.json of lucide-static {VERSION}")
        codes[name] = int(entry["encodedCode"].lstrip("\\"), 16)

    out.mkdir(parents=True, exist_ok=True)
    options = subset.Options()
    options.hinting = False
    options.layout_features = []
    options.name_IDs = ["*"]
    font = subset.load_font(str(package / "font" / "lucide.ttf"), options)
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=codes.values())
    subsetter.subset(font)
    subset.save_font(font, str(out / "lucide-subset.ttf"), options)

    for name, code in codes.items():
        print(f"{name:16} U+{code:04X}")
    print(f"{len(codes)} icons -> {out / 'lucide-subset.ttf'}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(Path(sys.argv[1]), Path(sys.argv[2]))
