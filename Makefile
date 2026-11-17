# One-command builds. Needs: cargo, python3, java (default-jdk), npm.
#   make apk       -> apk/nfl-play-viewer.apk   (builds data + web bundle first if missing)
#   make web       -> web/dist/index.html       (open it in a browser, no server needed)
#   make ipa       -> iOS IPA (needs SDK=/path/to/iPhoneOS14.5.sdk, see ios/README.md)
VERSION_CODE ?= 1
VERSION_NAME ?= 1.0

.PHONY: apk web data ipa clean
data/nfl.sqlite:
	cargo run -p etl --release
web/data/meta.js: data/nfl.sqlite
	cargo run -p webexport --release
data: web/data/meta.js
web: data
	python3 build_web.py
apk: data
	apk/build.sh $(VERSION_CODE) $(VERSION_NAME)
ipa: data
	ios/build_ipa.sh
clean:
	rm -rf web/dist apk/build ios/build
