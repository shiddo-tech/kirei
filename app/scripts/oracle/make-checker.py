import struct, sys, zlib

W, H = 1200, 800
rows = []
for y in range(H):
    r = bytearray([0])
    for x in range(W):
        if 60 <= x < W - 60 and 60 <= y < H - 60:
            if y < 120:
                c = (40, 44, 52)
            elif (x // 80 + y // 80) % 2 == 0:
                c = (230, 236, 245)
            else:
                c = (120 + (x % 120), 80, 200 - (y % 100))
        else:
            c = (255, 255, 255)
        r += bytes(c) + b"\xff"
    rows.append(bytes(r))


def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
    + chunk(b"pHYs", struct.pack(">IIB", 5669, 5669, 1))
    + chunk(b"IDAT", zlib.compress(b"".join(rows), 6))
    + chunk(b"IEND", b"")
)
open(sys.argv[1], "wb").write(png)
