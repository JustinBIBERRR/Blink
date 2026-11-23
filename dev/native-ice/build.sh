#!/bin/bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
app=${1:-/Volumes/代码/projects/Blink-artifacts/2026-11-23/Blink Ice Lab.app}
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" /private/tmp/blink-swift-cache
xcrun swiftc -O -swift-version 5 -module-cache-path /private/tmp/blink-swift-cache \
  -target arm64-apple-macosx13.0 "$root/dev/native-ice/IceLab.swift" \
  -framework AppKit -framework Metal -framework MetalKit -framework ScreenCaptureKit \
  -framework MetalPerformanceShaders -framework CoreMedia -framework CoreVideo -o "$app/Contents/MacOS/BlinkIceLab"
cp "$root/dev/native-ice/Ice.metal" "$app/Contents/Resources/Ice.metal"
cp "$root/src/assets/ice/deep-frost.png" "$app/Contents/Resources/deep-frost.png"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>BlinkIceLab</string>
<key>CFBundleIdentifier</key><string>com.blink.ice-lab</string>
<key>CFBundleName</key><string>Blink Ice Lab</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>13.0</string>
</dict></plist>
PLIST
codesign --force --sign - "$app"
echo "$app"
