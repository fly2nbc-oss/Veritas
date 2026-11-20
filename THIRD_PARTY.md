# Third-party components

Veritas's own code is licensed under Apache-2.0 (see [LICENSE](./LICENSE)); the program icon (`assets/icon.ico`, `assets/icon.png`) is original work and also under Apache-2.0. Veritas uses or embeds the following third-party work.

## Embedded in the binary

| Component | Used for | Version | License | Copyright | Source |
|---|---|---|---|---|---|
| Noto Sans Regular and SemiBold (`assets/fonts/NotoSans-*.ttf`) | Interface font in the Linux build when the system provides none (the Windows build does not embed it; Segoe UI is used there) | 2.015 | SIL Open Font License 1.1, [text](assets/fonts/OFL-NotoSans.txt) | © 2022 The Noto Project Authors | [notofonts/latin-greek-cyrillic](https://github.com/notofonts/latin-greek-cyrillic) |
| DejaVu Sans Mono (`assets/fonts/DejaVuSansMono.ttf`) | Monospace font for hashes in the Linux build when the system provides none (the Windows build does not embed it; Consolas is used there) | 2.37 | Bitstream Vera Fonts License; DejaVu changes are in the public domain, [text](assets/fonts/DejaVu-license.txt) | © 2003 Bitstream, Inc. | [dejavu-fonts.github.io](https://dejavu-fonts.github.io/) |
| Lucide icons: a subset of the icon font of `lucide-static` (`assets/fonts/lucide-subset.ttf`, cut out by `tools/make_lucide_subset.py`) | The interface's icons | 0.460.0 | ISC, portions MIT (Feather), [text](assets/fonts/Lucide-license.txt) | © Lucide Icons and Contributors; portions © Cole Bemis | [lucide.dev](https://lucide.dev) |
| Ubuntu-Light, egui-icons (a subset of emoji-icon-font) and Hack, contained in `epaint_default_fonts` | Fallback for characters the interface font lacks; included in the Windows build only | with `epaint_default_fonts` 0.37.0 | Ubuntu Font Licence 1.0 (Ubuntu-Light), MIT (egui-icons, Hack), Bitstream Vera License (Hack), [texts](assets/fonts/egui-fonts-license.txt) | © Canonical Ltd. (Ubuntu-Light); © 2014 John Slegers (emoji-icon-font); © 2018 Source Foundry Authors, © 2003 Bitstream, Inc. (Hack) | [emilk/egui](https://github.com/emilk/egui) |
| Rust crates: egui/eframe, glow, winit, md-5, sha1, sha2, rayon, walkdir, serde, serde_json, image (PNG only), jiff, open, trash, sys-locale, … | Window, hashes, folder walk, history, trash, system language | Names and versions in `Cargo.lock` | Mostly MIT or Apache-2.0; a few BSD-2/3, ISC, Zlib, BSL-1.0, Unicode-3.0, Unlicense, 0BSD (only the licenses in `deny.toml` are allowed) | Those of the respective authors, see the crates | [crates.io](https://crates.io) |

`jiff` provides the timestamps (it replaced `chrono`); `chrono` is now in the program only through `trash` (the trash on Linux).

## Shipped next to the binary

LICENSE, NOTICE, this file and the font license texts (`*.txt`). The Visual C++ C runtime is statically linked into the binary on Windows (Microsoft Visual Studio license terms, “Distributable Code”); no DLLs are included.

## Used from the system

- **Segoe UI**, **Segoe UI Semibold** and **Consolas** (Windows), or Noto Sans and DejaVu Sans (Linux) – read from the system at start-up, not distributed. For Chinese, Japanese and Korean file names, Veritas reads a CJK font of the system only at the first such character (Microsoft YaHei, Yu Gothic, Malgun Gothic or Noto Sans CJK).
- **OpenGL** – the system's graphics stack draws the window.
- The program the system provides for a file – opens it after a double-click (on Linux through `xdg-open`).

## Trademarks

Microsoft, Windows, Segoe UI and Consolas are trademarks of the Microsoft group of companies. Ubuntu is a trademark of Canonical Ltd., Debian a registered trademark of Software in the Public Interest, Inc. Bitstream Vera is a trademark of Bitstream, Inc. The names appear here and in the release notes only to say what Veritas is built and tested for; their owners have neither reviewed nor endorsed Veritas.
