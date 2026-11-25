#!/usr/bin/env bash
# Renders the same inputs and styles with the Swift app (oracle) and this app, then diffs them.
# Needs `swift build` in ../.. and `cargo build` here.
set -uo pipefail
cd "$(dirname "$0")"
SWIFT=../../../.build/arm64-apple-macosx/debug/Kirei
RUST=../../target/debug/kirei
OUT=${OUT:-$(mktemp -d)}
mkdir -p "$OUT"
python3 make-checker.py "$OUT/checker.png"
swift make-text-image.swift "$OUT/text.png"
python3 - "$OUT" <<'PY'
import json, sys
out = sys.argv[1]
bg = {"r": 0.2, "g": 0.3, "b": 0.5, "a": 1}
styles = {
  "default": {},
  "transparent": {"background": {"none": {}}, "shadow": 0.8},
  "mesh-legacy": {"background": {"mesh": {"_0": [{"r":0.1,"g":0.06,"b":0.2,"a":1},{"r":0.48,"g":0.18,"b":0.97,"a":1},{"r":0.95,"g":0.03,"b":0.64,"a":1},{"r":1,"g":0.54,"b":0.24,"a":1}]}}, "inset": 0.03},
  "image": {"background": {"image": {"path": out + "/text.png"}}, "frame": "browserDark"},
  "image-blur": {"background": {"image": {"path": out + "/text.png"}}, "backgroundBlur": 0.6, "grain": 0},
}
for name, s in styles.items():
    json.dump(s, open(f"{out}/style-{name}.json", "w"))
PY
status=0
for input in checker text; do
  for style in default transparent mesh-legacy image image-blur mac browser radial; do
    f="$OUT/style-$style.json"; [[ -f $f ]] || f="style-$style.json"
    "$SWIFT" --render-test "$OUT/$input.png" "$OUT/s-$input-$style.png" "$f" >/dev/null 2>&1
    "$RUST" --render-test "$OUT/$input.png" "$OUT/r-$input-$style.png" "$f" >/dev/null 2>&1
    printf '%-8s %-12s ' "$input" "$style"
    python3 diff.py "$OUT/s-$input-$style.png" "$OUT/r-$input-$style.png" || { echo "  ^ FALHOU"; status=1; }
  done
done
python3 make-tall.py "$OUT/tall.png"
"$SWIFT" --scroll-test "$OUT/tall.png" "$OUT/s-scroll.png" > "$OUT/s-scroll.txt"
"$RUST" --scroll-test "$OUT/tall.png" "$OUT/r-scroll.png" > "$OUT/r-scroll.txt"
printf '%-21s ' "scroll"
if diff -q "$OUT/s-scroll.txt" "$OUT/r-scroll.txt" >/dev/null; then
  python3 diff.py "$OUT/s-scroll.png" "$OUT/r-scroll.png" || { echo "  ^ FALHOU"; status=1; }
else
  echo "saída diferente"; status=1
fi
echo "saídas em $OUT"
exit $status
