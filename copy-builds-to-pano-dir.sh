#!/bin/bash
set -e
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

if [ "$(uname -m)" = "aarch64" ]; then
    resourcesDirName="linux-arm64"
else
    resourcesDirName="linux-x64"
fi
python3 scripts/build_native.py --output-dir "target/pano-artifacts/$resourcesDirName" --release
cp "target/pano-artifacts/$resourcesDirName/libpano_native_components.so" "../pano-scrobbler/desktopApp/resources/$resourcesDirName/"
cp "target/pano-artifacts/$resourcesDirName/libnative_webview.so" "../pano-scrobbler/desktopApp/resources/$resourcesDirName/"
