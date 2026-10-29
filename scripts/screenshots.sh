#!/usr/bin/env bash
# Genera las capturas del README a partir de la ruta /demo y les añade un
# marco de ventana (barra de título, esquinas redondeadas y sombra).
#
# Requisitos: `pnpm dev` corriendo, Google Chrome/Chromium e ImageMagick 7.
# Uso: scripts/screenshots.sh
set -euo pipefail

URL="${URL:-http://127.0.0.1:1420/demo}"
OUT="$(cd "$(dirname "$0")/.." && pwd)/docs/screenshots"
SCALE=1.5
TITLE="Claude · tienda-api"
CHROME="$(command -v google-chrome || command -v chromium || command -v chromium-browser)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$OUT"

# escena:ancho,alto (px CSS del contenido de la ventana)
SCENES=(conversation:1280,1110 permission:1280,960 start:1280,640)

frame() {
  local shot="$1" out="$2"
  local w h bar=54 r=18
  w=$(magick identify -format %w "$shot")
  h=$(magick identify -format %h "$shot")

  # Barra de título con los tres botones y el título centrado.
  magick -size "${w}x${bar}" xc:'#2b2b2b' \
    -fill '#ff5f57' -draw "circle 33,27 42,27" \
    -fill '#febc2e' -draw "circle 63,27 72,27" \
    -fill '#28c840' -draw "circle 93,27 102,27" \
    -fill '#3a3a3a' -draw "line 0,$((bar - 1)) $w,$((bar - 1))" \
    -font Noto-Sans-Medium -pointsize 20 -fill '#d4d4d4' -gravity center -annotate +0+0 "$TITLE" \
    "$tmp/bar.png"
  magick "$tmp/bar.png" "$shot" -append "$tmp/win.png"

  # Esquinas redondeadas, borde fino y sombra sobre fondo transparente.
  local H=$((h + bar))
  magick -size "${w}x${H}" xc:black -fill white -draw "roundrectangle 0,0 $((w - 1)),$((H - 1)) $r,$r" "$tmp/mask.png"
  magick "$tmp/win.png" "$tmp/mask.png" -alpha off -compose CopyOpacity -composite \
    -fill none -stroke '#4a4a4a' -strokewidth 2 -draw "roundrectangle 1,1 $((w - 2)),$((H - 2)) $r,$r" \
    "$tmp/rounded.png"
  magick "$tmp/rounded.png" \( +clone -background black -shadow 45x22+0+14 \) +swap \
    -background none -layers merge +repage \
    -bordercolor none -border 10 \
    -define png:compression-level=9 "$out"
}

for sc in "${SCENES[@]}"; do
  name="${sc%%:*}"
  size="${sc#*:}"
  rm -rf "$tmp/profile"
  "$CHROME" --headless=new --user-data-dir="$tmp/profile" --hide-scrollbars \
    --window-size="$size" --force-device-scale-factor="$SCALE" --virtual-time-budget=15000 \
    --screenshot="$tmp/$name.png" "$URL?scene=$name" >/dev/null 2>&1
  frame "$tmp/$name.png" "$OUT/$name.png"
  echo "✓ $OUT/$name.png"
done
