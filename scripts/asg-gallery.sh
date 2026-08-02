#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
asg_bin=${ASG_BIN:-${1:-}}
output=${2:-"$root/.aicode/state/asg-gallery"}
render_width=${RENDER_WIDTH:-95}
terminal_width=${TERMINAL_WIDTH:-100}

if [ -z "$asg_bin" ] || [ ! -x "$asg_bin" ]; then
    echo "usage: ASG_BIN=/absolute/path/to/asg scripts/asg-gallery.sh [ASG_BIN] [OUTPUT_DIR]" >&2
    exit 2
fi

cd "$root"
cargo build --quiet --bin mermansi --example asg_gallery
cargo run --quiet --example asg_gallery -- \
    --mermansi-bin "$root/target/debug/mermansi" \
    --output "$output" \
    --render-width "$render_width" \
    --terminal-width "$terminal_width"

mkdir -p "$output/svg" "$output/png"
for cast in "$output"/casts/*.cast; do
    id=$(basename "$cast" .cast)
    "$asg_bin" "$cast" "$output/svg/$id.svg" \
        --window --at 0.01 --no-cursor --theme github-dark --cols "$terminal_width"
done

if command -v rsvg-convert >/dev/null 2>&1; then
    for svg in "$output"/svg/*.svg; do
        id=$(basename "$svg" .svg)
        rsvg-convert "$svg" -o "$output/png/$id.png"
    done
elif command -v qlmanage >/dev/null 2>&1; then
    for svg in "$output"/svg/*.svg; do
        qlmanage -t -s 1800 -o "$output/png" "$svg" >/dev/null 2>&1
    done
else
    echo "warning: install rsvg-convert or use macOS qlmanage to rasterize the SVG gallery" >&2
fi

svg_count=$(find "$output/svg" -type f -name '*.svg' | wc -l | tr -d ' ')
if [ "$svg_count" -ne 33 ]; then
    echo "error: expected 33 SVG files (29 families + 4 scenarios), found $svg_count" >&2
    exit 1
fi

echo "ASG gallery: $output/index.html"
