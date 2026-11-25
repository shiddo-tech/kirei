"""A tall page-like image with varied rows, for the scrolling-capture test."""
import random, struct, sys, zlib

W, H = 900, 3200
rnd = random.Random(7)
rows = []
band = None
for y in range(H):
    if y % 24 == 0:
        words = []
        x = 40
        while x < W - 120 and rnd.random() < 0.9:
            w = rnd.randint(30, 140)
            words.append((x, x + w, rnd.randint(20, 90)))
            x += w + rnd.randint(8, 20)
        band = words
    r = bytearray([0])
    in_text = 6 <= y % 24 < 18
    for x in range(W):
        c = (250, 250, 252)
        if in_text:
            for a, b, g in band:
                if a <= x < b:
                    c = (g, g, g + 20)
                    break
        r += bytes(c) + b"\xff"
    rows.append(bytes(r))

def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

open(sys.argv[1], "wb").write(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(b"".join(rows), 6))
    + chunk(b"IEND", b"")
)
