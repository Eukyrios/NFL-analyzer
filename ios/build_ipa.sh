#!/usr/bin/env bash
# Build an iOS .ipa on Linux with clang + lld (no Mac, no Xcode). Needs the iPhoneOS SDK (theos/sdks).
# usage: SDK=/path/to/iPhoneOS14.5.sdk ios/build_ipa.sh
set -euo pipefail
cd "$(dirname "$0")/.."
SDK=${SDK:?set SDK=/path/to/iPhoneOS14.5.sdk}
python3 build_web.py
B=ios/build; rm -rf $B; mkdir -p $B/Payload/NFLPlayViewer.app
APP=$B/Payload/NFLPlayViewer.app
clang -target arm64-apple-ios14.0 -isysroot "$SDK" -fobjc-arc -O2 -c ios/native/main.m -o $B/main.o
/usr/lib/llvm-18/bin/ld64.lld -arch arm64 -platform_version ios 14.0.0 14.5 -syslibroot "$SDK" -lSystem -lobjc \
  -framework UIKit -framework WebKit -framework Foundation -framework CoreGraphics -o $APP/NFLPlayViewer $B/main.o
cp ios/native/Info.plist $APP/Info.plist
cp -R web/dist $APP/www
python3 - "$APP" <<'P'
import sys
from PIL import Image
im=Image.open('ios/Assets.xcassets/AppIcon.appiconset/icon-1024.png').convert('RGB')
for n,s in {'AppIcon60x60@2x':120,'AppIcon60x60@3x':180,'AppIcon76x76@2x':152,'AppIcon83.5x83.5@2x':167}.items():
    im.resize((s,s)).save(f'{sys.argv[1]}/{n}.png')
P
(cd $B && zip -qr NFLPlayViewer-unsigned.ipa Payload)
echo "built $B/NFLPlayViewer-unsigned.ipa"
