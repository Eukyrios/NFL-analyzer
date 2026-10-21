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
