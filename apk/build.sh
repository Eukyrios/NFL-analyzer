#!/usr/bin/env bash
# Builds a signed APK of the web app without the Android SDK.
# Needs: java, python3 (+ cryptography not required), and apktool/apksigner jars from the npm package
# @postar/apktool-node (npm i @postar/apktool-node). Usage: apk/build.sh <version_code> <version_name>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; root="$here/.."
# Find (or install) apktool + apksigner: $APKTOOL_LIB, then ./apk/tools, then ~/node_modules; else npm-install into apk/tools.
LIB="${APKTOOL_LIB:-}"
for c in "$here/tools/node_modules/@postar/apktool-node/lib" "$HOME/node_modules/@postar/apktool-node/lib"; do
  [ -n "$LIB" ] && [ -f "$LIB/apktool.jar" ] && break; LIB="$c"
done
if [ ! -f "$LIB/apktool.jar" ]; then
  echo "Installing apktool into apk/tools (one time)..."
  npm install --prefix "$here/tools" @postar/apktool-node >/dev/null
  LIB="$here/tools/node_modules/@postar/apktool-node/lib"
fi
command -v java >/dev/null || { echo "Java is missing: sudo apt install default-jdk"; exit 1; }
KS="${KEYSTORE:-$here/nfl.keystore}"
work="$here/build"; rm -rf "$work"; mkdir -p "$work"
python3 "$root/build_web.py"
cp -r "$root/web/dist" "$work/proj-assets"
cp -r "$here/proj" "$work/proj"
mkdir -p "$work/proj/assets"; cp -r "$work/proj-assets/." "$work/proj/assets/"
sed -i "s/versionCode: '.*'/versionCode: '${1:-1}'/; s/versionName: '.*'/versionName: '${2:-1.0}'/" "$work/proj/apktool.yml"
java -jar "$LIB/apktool.jar" b "$work/proj" -o "$work/unsigned.apk"
python3 "$here/zipalign.py" "$work/unsigned.apk" "$work/aligned.apk"
if [ ! -f "$KS" ]; then keytool -genkeypair -keystore "$KS" -storepass nflplay -keypass nflplay -alias nfl -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=NFL Play Viewer"; fi
java -jar "$LIB/apksigner.jar" sign --ks "$KS" --ks-pass pass:nflplay --key-pass pass:nflplay --ks-key-alias nfl --out "$here/nfl-play-viewer.apk" "$work/aligned.apk"
java -jar "$LIB/apksigner.jar" verify -v "$here/nfl-play-viewer.apk" | head -5
ls -la "$here/nfl-play-viewer.apk"
