#!/usr/bin/env python3
"""
Generate sprites for spherical fusion fuel tanks.

Three sizes of cryogenic D+He3 storage:
  Fusion Sphere S  (20x20 grid, 10m diameter)  ->  4080x4080 px (capped)
  Fusion Sphere M  (40x40 grid, 20m diameter)  ->  4080x4080 px (capped)
  Fusion Sphere L  (60x60 grid, 30m diameter)  ->  4080x4080 px (capped)

Flat white/grey color (no directional lighting) with geodesic panel lines,
equatorial weld seam, single 2.5-grid-wide structural truss on one side,
and fill port detail.

Uses PIL + numpy: numpy for per-pixel sphere fill (processed in strips
to limit memory), PIL for structural detail lines.
"""

import numpy as np
from PIL import Image, ImageDraw
import math
import os

PX = 360  # pixels per grid square
# Spheres need high detail because the player zooms in close. Cap at 4096
# (the per-sprite GPU/cap_sprite_size limit). Sphere PNGs end up 4080×4080.
# Spheres are exempt from the atlas halve loop (see HIGH_RES_SPRITES in
# src/render/sprites.rs) so they remain at full source resolution in the
# atlas; engines/other parts halve normally to keep atlas height bounded.
MAX_SPRITE_PX = 4096

# ================================================================
# Palette — white/grey for cryogenic D+He3
# ================================================================

# Flat sphere color (numpy float32 array)
SPHERE_BASE = np.array([225, 222, 218], dtype=np.float32)

# Subtle edge darkening color
SPHERE_EDGE = np.array([170, 165, 158], dtype=np.float32)

# Panel seam lines (PIL tuples)
SEAM_COLOR = (180, 175, 168)
WELD_COLOR = (155, 150, 142)

# Truss structure
TRUSS_DARK = (65, 60, 50, 255)
TRUSS_MID = (90, 82, 68, 255)
TRUSS_LIGHT = (115, 108, 95, 255)

# Fill port
PORT_DARK = (60, 58, 52, 255)
PORT_MID = (90, 85, 75, 255)
PORT_LIGHT = (120, 115, 105, 255)
PORT_HIGHLIGHT = (150, 145, 135, 255)


# ================================================================
# Tank definitions
# ================================================================

TANKS = {
    "tank_sphere_s": {"grid": 20, "name": "Fusion Sphere S (20x20)"},
    "tank_sphere_m": {"grid": 40, "name": "Fusion Sphere M (40x40)"},
    "tank_sphere_l": {"grid": 60, "name": "Fusion Sphere L (60x60)"},
}


# ================================================================
# Sphere generator
# ================================================================

def generate_fusion_sphere(grid_size):
    """Generate a spherical fusion tank sprite with flat shading."""
    effective_px = min(PX, MAX_SPRITE_PX // grid_size)
    size = grid_size * effective_px
    radius = size / 2.0
    cx_f = cy_f = size / 2.0
    cx = cy = size // 2
    r = int(radius)

    # Create output image
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))

    # Anti-aliased edge width in normalized coords (~2 pixels)
    edge_t = max(0.002, 2.0 / radius)

    # Process in horizontal strips for memory efficiency
    chunk = 256
    total_chunks = (size + chunk - 1) // chunk
    report_interval = max(1, total_chunks // 5)

    for ci, y_start in enumerate(range(0, size, chunk)):
        if ci % report_interval == 0:
            pct = int(100 * y_start / size)
            print(f"    filling... {pct}%")

        y_end = min(y_start + chunk, size)
        rows = y_end - y_start

        # Coordinate grids for this strip
        yy = np.arange(y_start, y_end, dtype=np.float32)
        xx = np.arange(0, size, dtype=np.float32)
        Y, X = np.meshgrid(yy, xx, indexing='ij')

        # Normalized sphere coordinates [-1, 1]
        nx = (X - cx_f) / radius
        ny = (Y - cy_f) / radius
        dist_sq = nx * nx + ny * ny
        dist = np.sqrt(dist_sq)

        # Flat base color with subtle edge darkening (limb effect)
        # Edges get slightly darker to suggest curvature without lighting
        limb = np.clip(dist * 1.2, 0.0, 1.0)[..., np.newaxis]
        color = SPHERE_BASE + (SPHERE_EDGE - SPHERE_BASE) * (limb ** 2)

        color = np.clip(color, 0, 255).astype(np.uint8)

        # Anti-aliased sphere alpha
        alpha_f = np.clip((1.0 - dist) / edge_t, 0.0, 1.0)
        alpha = (alpha_f * 255).astype(np.uint8)

        # Build strip RGBA
        strip = np.zeros((rows, size, 4), dtype=np.uint8)
        strip[:, :, 0] = color[:, :, 0]
        strip[:, :, 1] = color[:, :, 1]
        strip[:, :, 2] = color[:, :, 2]
        strip[:, :, 3] = alpha

        # Paste strip into image
        strip_img = Image.fromarray(strip)
        img.paste(strip_img, (0, y_start))

    print("    filling... 100%")

    # ---- Detail drawing with PIL ----

    # Save sphere alpha (restored after drawing to clip lines to sphere)
    alpha_mask = img.split()[3]
    d = ImageDraw.Draw(img)

    scale = grid_size / 20.0  # relative to smallest tank
    line_w = max(2, int(4 * scale))
    weld_w = max(3, int(7 * scale))

    # --- Latitude panel lines ---
    for lat_deg in [-55, -28, 28, 55]:
        lat = math.radians(lat_deg)
        y_pos = cy - int(r * math.sin(lat))
        half_w = int(r * math.cos(lat) * 0.93)
        d.line([(cx - half_w, y_pos), (cx + half_w, y_pos)],
               fill=SEAM_COLOR, width=line_w)

    # --- Meridian panel lines (great-circle arcs) ---
    for lon_deg in [0, 35, -35, 65, -65]:
        lon = math.radians(lon_deg)
        points = []
        for t in range(-85, 86):
            theta = math.radians(t)
            x_pt = cx + int(r * math.sin(lon) * math.cos(theta))
            y_pt = cy - int(r * math.sin(theta))
            # Front-facing check (z-component of surface normal)
            z_check = math.cos(lon) * math.cos(theta)
            if z_check > 0.1:
                points.append((x_pt, y_pt))
            else:
                if len(points) > 1:
                    d.line(points, fill=SEAM_COLOR, width=line_w)
                points = []
        if len(points) > 1:
            d.line(points, fill=SEAM_COLOR, width=line_w)

    # --- Equatorial weld seam (thicker, distinct) ---
    d.line([(cx - int(r * 0.93), cy), (cx + int(r * 0.93), cy)],
           fill=WELD_COLOR, width=weld_w)

    # --- Fill port at top ---
    port_r = max(6, int(14 * scale))
    port_y = cy - r + int(r * 0.06)

    # Outer flange
    flange = max(3, int(5 * scale))
    d.ellipse([cx - port_r - flange, port_y - port_r - flange,
               cx + port_r + flange, port_y + port_r + flange],
              fill=PORT_DARK)
    # Port body
    d.ellipse([cx - port_r, port_y - port_r,
               cx + port_r, port_y + port_r],
              fill=PORT_MID)
    # Inner recess
    inner_r = max(3, port_r - int(4 * scale))
    d.ellipse([cx - inner_r, port_y - inner_r,
               cx + inner_r, port_y + inner_r],
              fill=PORT_LIGHT)
    # Specular dot
    dot_r = max(1, port_r // 4)
    d.ellipse([cx - dot_r - 1, port_y - dot_r - 2,
               cx + dot_r - 1, port_y + dot_r - 2],
              fill=PORT_HIGHLIGHT)

    # Restore sphere alpha (clips all drawn lines to sphere boundary)
    img.putalpha(alpha_mask)

    # --- Single structural truss on right side ---
    # 5 grid squares wide, flat outer edge, attached at equator
    d = ImageDraw.Draw(img)

    truss_grid_w = 2.5  # 2.5 grid squares wide
    truss_px_w = int(truss_grid_w * effective_px)  # width in pixels
    truss_height = int(r * 0.3)  # ~30% of radius tall

    # Truss starts at the sphere edge on the right side
    truss_x1 = cx + r - int(r * 0.03)  # slightly overlapping sphere edge
    truss_x2 = truss_x1 + truss_px_w
    truss_y1 = cy - truss_height // 2
    truss_y2 = cy + truss_height // 2

    member_w = max(4, int(8 * scale))  # structural member thickness

    # Outer frame rectangle
    d.rectangle([truss_x1, truss_y1, truss_x2, truss_y2],
                fill=None, outline=TRUSS_DARK, width=member_w)

    # Fill the frame interior with mid tone
    d.rectangle([truss_x1 + member_w, truss_y1 + member_w,
                 truss_x2 - member_w, truss_y2 - member_w],
                fill=TRUSS_MID)

    # Re-draw the outer frame on top
    d.rectangle([truss_x1, truss_y1, truss_x2, truss_y2],
                fill=None, outline=TRUSS_DARK, width=member_w)

    # Diagonal cross-bracing
    d.line([(truss_x1, truss_y1), (truss_x2, truss_y2)],
           fill=TRUSS_DARK, width=member_w)
    d.line([(truss_x1, truss_y2), (truss_x2, truss_y1)],
           fill=TRUSS_DARK, width=member_w)

    # Horizontal cross-member at equator
    d.line([(truss_x1, cy), (truss_x2, cy)],
           fill=TRUSS_DARK, width=member_w)

    # Vertical center strut
    mid_x = (truss_x1 + truss_x2) // 2
    d.line([(mid_x, truss_y1), (mid_x, truss_y2)],
           fill=TRUSS_DARK, width=member_w)

    # Highlight strip along top edge of truss
    d.rectangle([truss_x1 + member_w, truss_y1 + member_w,
                 truss_x2 - member_w, truss_y1 + member_w + max(2, int(3 * scale))],
                fill=TRUSS_LIGHT)

    # Flat outer edge reinforcement (right side)
    reinforce_w = max(3, int(6 * scale))
    d.rectangle([truss_x2 - reinforce_w, truss_y1, truss_x2, truss_y2],
                fill=TRUSS_DARK)
    # Slight highlight on reinforcement
    d.rectangle([truss_x2 - reinforce_w + 1, truss_y1 + 2,
                 truss_x2 - 1, truss_y1 + max(3, int(5 * scale))],
                fill=TRUSS_LIGHT)

    return img


# ================================================================
# Main
# ================================================================

if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    parts_dir = os.path.join(project_root, "data", "sprites", "parts")
    os.makedirs(parts_dir, exist_ok=True)

    for part_id, info in TANKS.items():
        print(f"Generating {info['name']}...")
        img = generate_fusion_sphere(info["grid"])
        path = os.path.join(parts_dir, f"{part_id}.png")
        img.save(path)
        print(f"  -> {path}  ({img.size[0]}x{img.size[1]})")

    print(f"\nDone! Generated {len(TANKS)} fusion tank sprites.")
