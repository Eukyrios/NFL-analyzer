#!/usr/bin/env python3
"""Joins web/index.src.html + app1.js + app2.js into web/dist/index.html and copies web/data next to it."""
import pathlib, shutil, sys
w = pathlib.Path(__file__).parent / "web"
js = (w / "app1.js").read_text() + "\n" + (w / "app2.js").read_text()
html = (w / "index.src.html").read_text().replace("/*APP_JS*/", js)
d = w / "dist"
d.mkdir(exist_ok=True)
(d / "index.html").write_text(html)
if (d / "data").exists():
    shutil.rmtree(d / "data")
shutil.copytree(w / "data", d / "data")
for f in ("logos.js", "usmap.js", "heads.js"):
    shutil.copy(w / f, d / f)
print("built", d / "index.html", len(html) // 1024, "KB")
