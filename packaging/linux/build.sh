#!/usr/bin/env bash
# Packs a Linux release build into dist/linux:
#
#   veritas_<version>_amd64.deb
#   veritas_<version>_x86_64.AppImage
#
# Run after `cargo build --release` on Ubuntu 24.04 (glibc 2.39).
# Needs: cargo-deb, curl.
set -euo pipefail
cd "$(dirname "$0")/../.."

bin=target/release/veritas
if [[ ! -x $bin ]]; then
    echo "$bin is missing - run: cargo build --release" >&2
    exit 1
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
dist=dist/linux
rm -rf "$dist"
mkdir -p "$dist"

cargo deb --no-build --output "$dist/veritas_${version}_amd64.deb"

tools=target/appimage-tools
mkdir -p "$tools"
fetch() { # name url sha256
    local file="$tools/$1"
    if [[ ! -f $file ]] || ! echo "$3  $file" | sha256sum --check --status; then
        curl -fsSL --retry 3 -o "$file" "$2"
        echo "$3  $file" | sha256sum --check --quiet
    fi
    chmod +x "$file"
}
fetch linuxdeploy \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
    c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
fetch appimagetool \
    https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage \
    ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
fetch runtime \
    https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64 \
    2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

appdir=target/appimage/Veritas.AppDir
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/doc/veritas"
install -m755 "$bin" "$appdir/usr/bin/veritas"
cp LICENSE NOTICE THIRD_PARTY.md assets/fonts/*.txt "$appdir/usr/share/doc/veritas/"

export APPIMAGE_EXTRACT_AND_RUN=1
export NO_STRIP=1
"$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$appdir/usr/bin/veritas" \
    --desktop-file packaging/linux/veritas.desktop \
    --icon-file assets/icon.png --icon-filename veritas \
    --custom-apprun packaging/linux/AppRun

ARCH=x86_64 "$tools/appimagetool" --no-appstream --runtime-file "$tools/runtime" \
    "$appdir" "$dist/veritas_${version}_x86_64.AppImage"

(cd "$dist" && ls -l && sha256sum -- *)
