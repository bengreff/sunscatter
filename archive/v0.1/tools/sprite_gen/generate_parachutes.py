#!/usr/bin/env python3
"""
Generate sprites for parachute parts.

Packed sprites (stowed parachute domes) — both 1x1 grid (720x720 px):
  parachute_small.png — 0.25m (0.5 grid) wide dome, bottom-aligned
  parachute_large.png — 0.5m  (1.0 grid) wide dome, bottom-aligned

Deployed canopy is rendered procedurally (no sprite needed).

PX=720 px/grid, PAD=0.
"""

from PIL import Image, ImageDraw
import os

PX = 720

# Colors
WHITE = (230, 232, 235)
ORANGE = (210, 120, 40)
STEEL_DARK = (48, 50, 55)

OUTPUT_DIR = os.path.join(os.path.dirname(__file__), '..', '..', 'data', 'sprites', 'parts')


def draw_packed_parachute(dome_grid_width, filename, label):
    """Draw a stowed parachute: just a dome (semicircle), bottom-aligned in a 1x1 hitbox.

    dome_grid_width: width of the dome in grid squares (0.5 or 1.0).
    The dome is a semicircle whose flat edge sits at the very bottom of the sprite.
    """
    w = PX  # 1 grid = 180px
    h = PX
    img = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    dome_px = int(dome_grid_width * PX)
    dome_radius = dome_px // 2
    cx = w // 2

    # Flat edge of dome at the very bottom of the sprite
    dome_cy = h - 1  # center of the full circle; flat edge sits here

    # Draw dome as alternating white/orange pie slices (top semicircle only)
    num_wedges = 8
    bbox = [cx - dome_radius, dome_cy - dome_radius, cx + dome_radius, dome_cy + dome_radius]
    for i in range(num_wedges):
        angle_start = 180 + i * (180 / num_wedges)
        angle_end = 180 + (i + 1) * (180 / num_wedges)
        color = WHITE if i % 2 == 0 else ORANGE
        draw.pieslice(bbox, angle_start, angle_end, fill=color)

    # Outline
    draw.arc(bbox, 180, 360, fill=STEEL_DARK, width=16)
    draw.line([cx - dome_radius, dome_cy, cx + dome_radius, dome_cy], fill=STEEL_DARK, width=16)

    img.save(os.path.join(OUTPUT_DIR, filename))
    print(f"  Saved {filename} ({w}x{h}) — {label}, dome {dome_px}px wide")


def main():
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    print("Generating parachute sprites...")

    # Packed dome sprites — both in 1x1 grid, different dome sizes
    draw_packed_parachute(0.5, "parachute_small.png", "Small Parachute (0.25m)")
    draw_packed_parachute(1.0, "parachute_large.png", "Large Parachute (0.5m)")

    # Deployed canopy is rendered procedurally — no sprite needed

    print("Done!")


if __name__ == '__main__':
    main()
