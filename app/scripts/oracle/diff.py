"""Mean absolute difference between two PNGs (8-bit RGB/RGBA), and the share of pixels off by > 24."""
import struct, sys, zlib


def load(p):
    d = open(p, "rb").read()
    i, idat = 8, b""
    while i < len(d):
        n = struct.unpack(">I", d[i : i + 4])[0]
        t, c = d[i + 4 : i + 8], d[i + 8 : i + 8 + n]
        if t == b"IHDR":
            W, H, _, ct = struct.unpack(">IIBB", c[:10])
        if t == b"IDAT":
            idat += c
        i += 12 + n
    raw = zlib.decompress(idat)
    bpp = 4 if ct == 6 else 3
    st, out, prev, p = W * bpp, bytearray(), bytearray(W * bpp), 0
    for _ in range(H):
        f, line = raw[p], bytearray(raw[p + 1 : p + 1 + st])
        p += 1 + st
        for x in range(st):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pp = a + b - c
                pa, pb, pc = abs(pp - a), abs(pp - b), abs(pp - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        out += line
        prev = line
    return W, H, bpp, out


W, H, b1, a = load(sys.argv[1])
W2, H2, b2, b = load(sys.argv[2])
if (W, H) != (W2, H2):
    print(f"SIZE MISMATCH {W}x{H} vs {W2}x{H2}")
    sys.exit(1)
diffs = [abs(a[i * b1 + c] - b[i * b2 + c]) for i in range(0, W * H, 5) for c in range(3)]
big = sum(1 for d in diffs if d > 24)
mean = sum(diffs) / len(diffs)
pct = 100 * big / len(diffs)
print(f"{W}x{H} mean {mean:.2f} pct>24 {pct:.2f}%")
sys.exit(0 if mean < 2 and pct < 1.5 else 1)
