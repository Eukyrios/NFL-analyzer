# iOS build

An IPA can only be compiled on macOS with Xcode (Apple's SDK is not available on Linux).
This folder is a thin WKWebView shell around the same web app as the Android APK.

## Option A – GitHub Actions (no Mac needed)
Run the **iOS (unsigned IPA)** workflow. It needs `web/data/` present (run `cargo run -p webexport --release`
and un-ignore/commit `web/data`, or generate it in the workflow). Download the artifact `NFLPlayViewer-unsigned.ipa`.
Install it with **AltStore / Sideloadly / SideStore**, which re-sign it with your own Apple ID (free account = 7 days).

## Option B – on a Mac
```
python3 build_web.py && rm -rf ios/www && cp -R web/dist ios/www
cd ios && brew install xcodegen && xcodegen && open NFLPlayViewer.xcodeproj
```
Pick your team under Signing & Capabilities and press Run (or Product > Archive for TestFlight / App Store).

## Option C – build the IPA on Linux (no Mac, no Xcode)
`ios/build_ipa.sh` compiles `ios/native/main.m` with clang, links with lld (`ld64.lld`) against the
iPhoneOS SDK from https://github.com/theos/sdks, bundles `web/dist` and zips `NFLPlayViewer-unsigned.ipa`.
```
git clone --depth 1 --filter=blob:none --sparse https://github.com/theos/sdks && (cd sdks && git sparse-checkout set iPhoneOS14.5.sdk)
SDK=$PWD/sdks/iPhoneOS14.5.sdk ios/build_ipa.sh
```
The result is unsigned; AltStore / Sideloadly / SideStore sign it with your Apple ID when installing.
