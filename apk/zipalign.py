#!/usr/bin/env python3
"""Pure-Python zipalign: stored entries on 4-byte (resources.arsc / .so on 4096) boundaries. Usage: zipalign.py in.apk out.apk"""
import struct, sys, zipfile

src, dst = sys.argv[1:3]
zin = zipfile.ZipFile(src)
with open(dst, "wb") as out:
    zout = zipfile.ZipFile(out, "w")
    for info in zin.infolist():
        data = zin.read(info.filename)
        ni = zipfile.ZipInfo(info.filename, info.date_time)
        ni.compress_type = info.compress_type
        ni.external_attr = info.external_attr
        if info.compress_type == zipfile.ZIP_STORED:
            align = 4096 if info.filename.endswith(".so") else 4
            hdr = 30 + len(info.filename.encode("utf-8"))
            pad = (-(out.tell() + hdr)) % align
            if pad >= 1 and pad < 4: pad += 4  # extra field needs at least 4 bytes
            if pad:
                ni.extra = struct.pack("<HH", 0xD935, pad - 4) + b"\0" * (pad - 4)
        zout.writestr(ni, data)
    zout.close()
