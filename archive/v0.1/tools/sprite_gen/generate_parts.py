#!/usr/bin/env python3
"""
Generate sprites for all non-engine Sunscatter parts.

Covers: pods, fuel tanks, decouplers, fairings, nose cones, heat shields, RCS.
Canvas: grid_width * PX x grid_height * PX + padding on each side.
Matches palette and PX/PAD from generate_all.py (engine sprites).
"""

from PIL import Image, ImageDraw
import math
import os

# ================================================================
# Palette (shared with engine generator)
# ================================================================
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)

PX = 360   # Pixels per grid square (4x base resolution)
MAX_SPRITE_PX = 4096  # cap output image size to avoid multi-GB RGBA decode
PAD = 0   # No padding — atlas packer adds 1px spacing between sprites

def _capped_px(w, h):
    """Return effective PX capped so the output image stays within MAX_SPRITE_PX."""
    return min(PX, MAX_SPRITE_PX // max(w, h))


# ================================================================
# Drawing primitives
# ================================================================

def trap(d, cx, y, tw, bw, h, fill, outline=None):
    d.polygon([(cx - tw/2, y), (cx + tw/2, y),
               (cx + bw/2, y + h), (cx - bw/2, y + h)],
              fill=fill, outline=outline)

def circ(d, cx, cy, r, **kw):
    d.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)

def lerp_color(c1, c2, t):
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(len(c1)))


# ================================================================
# Part generators
# ================================================================

def generate_tank(spec):
    """Fuel tank: cylindrical body with end caps and weld seams."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2

    body = (195, 200, 210)
    d.rectangle([x0, y0, x1, y1], fill=body)

    # End caps
    cap_h = max(3, min(int(0.12 * PX), h // 4))
    cap_color = (180, 185, 195)
    d.rectangle([x0, y0, x1, y0 + cap_h], fill=cap_color)
    d.rectangle([x0, y1 - cap_h, x1, y1], fill=cap_color)
    d.line([(x0, y0 + cap_h), (x1, y0 + cap_h)], fill=(155, 160, 170), width=1)
    d.line([(x0, y1 - cap_h), (x1, y1 - cap_h)], fill=(155, 160, 170), width=1)

    # Horizontal weld lines every ~2 grid units
    if gh > 1.5:
        weld_spacing = int(2.0 * PX)
        weld_color = (175, 180, 190)
        wy = y0 + cap_h + weld_spacing
        while wy < y1 - cap_h - 4:
            d.line([(x0 + 2, wy), (x1 - 2, wy)], fill=weld_color, width=1)
            d.line([(x0 + 2, wy + 1), (x1 - 2, wy + 1)], fill=(185, 190, 200), width=1)
            wy += weld_spacing

    # Vertical center seam
    seam_color = (178, 183, 193)
    d.line([(cx, y0 + cap_h + 2), (cx, y1 - cap_h - 2)], fill=seam_color, width=1)

    # Outline
    d.rectangle([x0, y0, x1, y1], outline=(145, 150, 160))

    return img


def generate_pod(spec):
    """Command pod: trapezoid hull with viewport window and panel lines."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    tw_grid = spec["top_width"]
    w, h = int(gw * PX), int(gh * PX)
    tw = int(tw_grid * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    y0, y1 = PAD, PAD + h

    # Hull
    hull = (55, 58, 65)
    hull_outline = (38, 40, 45)
    trap(d, cx, y0, tw, w, h, hull, outline=hull_outline)

    # Left edge highlight for 3D effect
    for i in range(h):
        frac = i / max(1, h)
        lw = tw / 2 + (w / 2 - tw / 2) * frac
        px = cx - int(lw) + 2
        py = y0 + i
        c = lerp_color((75, 78, 88), (65, 68, 78), frac)
        d.point((px, py), fill=c)
        if w > 200:
            d.point((px + 1, py), fill=c)

    # Panel lines
    n_panels = max(1, int(gh) - 1)
    panel_color = (45, 48, 55)
    for i in range(1, n_panels + 1):
        frac = i / (n_panels + 1)
        py = y0 + int(h * frac)
        pw_at = tw + (w - tw) * frac
        hw = int(pw_at / 2)
        d.line([(cx - hw + 3, py), (cx + hw - 3, py)], fill=panel_color, width=1)

    # Viewport window
    win_frac = 0.35
    win_y = y0 + int(h * win_frac)
    w_at = tw + (w - tw) * win_frac
    win_r = max(6, int(min(w_at, h) * 0.12))

    circ(d, cx, win_y, win_r + 2, fill=(100, 130, 170))
    circ(d, cx, win_y, win_r, fill=(140, 180, 220))
    circ(d, cx - win_r // 3, win_y - win_r // 3, max(2, win_r // 3),
         fill=(180, 210, 240))
    circ(d, cx, win_y, win_r, outline=(50, 52, 58))

    return img


def generate_decoupler(spec):
    """Stack decoupler: metallic band with separation line and bolts."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2
    cy = ch // 2

    # Main band (lighter metallic)
    d.rectangle([x0, y0, x1, y1], fill=STEEL_MID, outline=STEEL_DARK)

    # Top and bottom ring edges
    ring_h = max(2, h // 6)
    d.rectangle([x0, y0, x1, y0 + ring_h], fill=STEEL_LIGHT)
    d.rectangle([x0, y1 - ring_h, x1, y1], fill=STEEL_LIGHT)
    d.line([(x0, y0 + ring_h), (x1, y0 + ring_h)], fill=STEEL_DARK, width=1)
    d.line([(x0, y1 - ring_h), (x1, y1 - ring_h)], fill=STEEL_DARK, width=1)

    # Separation line (dashed) through middle
    dash_len = max(4, w // 15)
    gap_len = max(2, dash_len // 2)
    x = x0 + 2
    while x < x1 - 2:
        end = min(x + dash_len, x1 - 2)
        d.line([(x, cy), (end, cy)], fill=STEEL_HIGHLIGHT, width=1)
        x = end + gap_len

    # Bolts
    bolt_r = max(2, min(3, h // 8))
    n_bolts = max(2, int(gw * 1.5))
    for i in range(n_bolts):
        bx = x0 + bolt_r * 2 + i * int((w - bolt_r * 4) / max(1, n_bolts - 1))
        circ(d, bx, y0 + ring_h // 2, bolt_r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y0 + ring_h // 2, max(1, bolt_r - 1), fill=STEEL_DARK)
        circ(d, bx, y1 - ring_h // 2, bolt_r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y1 - ring_h // 2, max(1, bolt_r - 1), fill=STEEL_DARK)

    return img


def generate_decoupler_radial(spec):
    """Radial decoupler: two-sided clamp with spring mechanism."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2
    cy = ch // 2

    clamp_thick = max(4, w // 3)
    gap = max(3, w // 5)

    # Left clamp jaw (attaches to core stack) — C-shaped bracket
    d.rectangle([x0, y0, x0 + clamp_thick, y1], fill=STEEL_LIGHT, outline=STEEL_DARK)
    # Top arm extending right
    d.rectangle([x0, y0, cx - gap // 2, y0 + clamp_thick], fill=STEEL_LIGHT, outline=STEEL_DARK)
    # Bottom arm extending right
    d.rectangle([x0, y1 - clamp_thick, cx - gap // 2, y1], fill=STEEL_LIGHT, outline=STEEL_DARK)

    # Right clamp jaw (attaches to booster) — C-shaped bracket mirrored
    d.rectangle([x1 - clamp_thick, y0, x1, y1], fill=STEEL_MID, outline=STEEL_DARK)
    # Top arm extending left
    d.rectangle([cx + gap // 2, y0, x1, y0 + clamp_thick], fill=STEEL_MID, outline=STEEL_DARK)
    # Bottom arm extending left
    d.rectangle([cx + gap // 2, y1 - clamp_thick, x1, y1], fill=STEEL_MID, outline=STEEL_DARK)

    # Three horizontal connection bars between jaws (evenly spaced)
    bar_h = max(3, h // 12)
    inner_h = h - clamp_thick * 2
    for i in range(3):
        bar_y = y0 + clamp_thick + int((i + 1) * inner_h / 4) - bar_h // 2
        d.rectangle([x0 + clamp_thick - 1, bar_y,
                     x1 - clamp_thick + 1, bar_y + bar_h],
                    fill=STEEL_HIGHLIGHT, outline=STEEL_DARK)

    return img


def generate_fairing(spec):
    """Fairing base: disc/ring with seam and bolt pattern."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2
    cy = ch // 2

    # Main body (lighter grey)
    d.rectangle([x0, y0, x1, y1], fill=(105, 110, 120))

    # Horizontal seam through middle
    d.line([(x0 + 2, cy), (x1 - 2, cy)], fill=(70, 74, 82), width=1)
    d.line([(x0 + 2, cy + 1), (x1 - 2, cy + 1)], fill=(130, 135, 145), width=1)

    # Bolt pattern
    bolt_r = max(2, min(3, h // 8))
    n_bolts = max(2, int(gw * 1.5))
    bolt_inset = max(3, h // 4)
    for i in range(n_bolts):
        bx = x0 + bolt_r * 2 + i * int((w - bolt_r * 4) / max(1, n_bolts - 1))
        circ(d, bx, y0 + bolt_inset, bolt_r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y0 + bolt_inset, max(1, bolt_r - 1), fill=STEEL_DARK)
        circ(d, bx, y1 - bolt_inset, bolt_r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y1 - bolt_inset, max(1, bolt_r - 1), fill=STEEL_DARK)

    # Outline
    d.rectangle([x0, y0, x1, y1], outline=STEEL_DARK)

    return img


def generate_nosecone(spec):
    """Nose cone: triangular shape with gradient, edge lines, and tip highlight."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    shape = spec["shape"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2

    tip_color = (210, 215, 225)
    base_color = (195, 200, 210)
    edge_color = (145, 150, 160)

    if shape == "Triangle":
        # Isosceles: tip at top center, base at bottom
        # Fill with row-by-row gradient
        for i in range(h):
            frac = i / max(1, h - 1)
            half_w = max(0, int(w / 2 * frac))
            if half_w < 1 and frac > 0:
                half_w = 1
            py = y0 + i
            color = lerp_color(tip_color, base_color, frac)
            if half_w > 0:
                d.line([(cx - half_w, py), (cx + half_w, py)],
                       fill=color, width=1)

        # Edge lines
        d.line([(cx, y0), (x0, y1)], fill=edge_color, width=2)
        d.line([(cx, y0), (x1, y1)], fill=edge_color, width=2)
        d.line([(x0, y1), (x1, y1)], fill=edge_color, width=2)

        # Left edge highlight
        for i in range(2, h):
            frac = i / max(1, h - 1)
            half_w = int(w / 2 * frac)
            if half_w > 3:
                px = cx - half_w + 2
                py = y0 + i
                hl = lerp_color((220, 225, 235), (200, 205, 215), frac)
                d.point((px, py), fill=hl)

        # Center seam line
        for i in range(max(4, h // 8), h - 2):
            frac = i / max(1, h - 1)
            half_w = int(w / 2 * frac)
            if half_w > 4:
                py = y0 + i
                d.point((cx, py), fill=(178, 183, 193))

        # Structural ring near base
        ring_y = y1 - max(4, h // 8)
        ring_hw = int(w / 2 * (1 - 1.0 / max(2, gh)))
        d.line([(cx - ring_hw, ring_y), (cx + ring_hw, ring_y)],
               fill=STEEL_LIGHT, width=2)
        d.line([(cx - ring_hw, ring_y + 2), (cx + ring_hw, ring_y + 2)],
               fill=STEEL_DARK, width=1)

    elif shape == "TriangleRight":
        # Vertical edge on RIGHT, hypotenuse on left
        # Tip at top-right, base at bottom
        for i in range(h):
            frac = i / max(1, h - 1)
            row_w = max(0, int(w * frac))
            if row_w < 1 and frac > 0:
                row_w = 1
            py = y0 + i
            color = lerp_color(tip_color, base_color, frac)
            if row_w > 0:
                d.line([(x1 - row_w, py), (x1, py)], fill=color, width=1)

        d.line([(x1, y0), (x0, y1)], fill=edge_color, width=2)
        d.line([(x1, y0), (x1, y1)], fill=edge_color, width=2)
        d.line([(x0, y1), (x1, y1)], fill=edge_color, width=2)

        # Right edge highlight
        for i in range(2, h - 2):
            py = y0 + i
            hl = lerp_color((220, 225, 235), (200, 205, 215), i / max(1, h))
            d.point((x1 - 2, py), fill=hl)

        # Structural ring near base
        ring_y = y1 - max(4, h // 8)
        ring_w_at = int(w * (1 - 1.0 / max(2, gh)))
        d.line([(x1 - ring_w_at, ring_y), (x1, ring_y)],
               fill=STEEL_LIGHT, width=2)

    elif shape == "TriangleLeft":
        # Vertical edge on LEFT, hypotenuse on right
        # Tip at top-left, base at bottom
        for i in range(h):
            frac = i / max(1, h - 1)
            row_w = max(0, int(w * frac))
            if row_w < 1 and frac > 0:
                row_w = 1
            py = y0 + i
            color = lerp_color(tip_color, base_color, frac)
            if row_w > 0:
                d.line([(x0, py), (x0 + row_w, py)], fill=color, width=1)

        d.line([(x0, y0), (x1, y1)], fill=edge_color, width=2)
        d.line([(x0, y0), (x0, y1)], fill=edge_color, width=2)
        d.line([(x0, y1), (x1, y1)], fill=edge_color, width=2)

        # Left edge highlight
        for i in range(2, h - 2):
            py = y0 + i
            hl = lerp_color((220, 225, 235), (200, 205, 215), i / max(1, h))
            d.point((x0 + 2, py), fill=hl)

        # Structural ring near base
        ring_y = y1 - max(4, h // 8)
        ring_w_at = int(w * (1 - 1.0 / max(2, gh)))
        d.line([(x0, ring_y), (x0 + ring_w_at, ring_y)],
               fill=STEEL_LIGHT, width=2)

    return img


def generate_heatshield(spec):
    """Heat shield: dramatic convex dome — 1/3 height at edges, 1 grid at center."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w = int(gw * PX)
    # Both edge and center height scale with width
    edge_h = max(5, int(w * 0.04))
    center_h = max(edge_h + 4, int(w * 0.096))
    # Canvas must fit the tallest column (center)
    total_max_h = center_h
    cw = w + PAD * 2
    ch = total_max_h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0 = PAD
    x1 = x0 + w
    cx = cw // 2

    # Backing structure: thin flat strip at top across full width
    backing_h = max(2, edge_h // 3)
    y_top = PAD
    d.rectangle([x0, y_top, x1, y_top + backing_h], fill=(40, 38, 35))

    # Ablative dome — column by column, parabolic from edge_h to center_h
    ablative_top = y_top + backing_h
    for col in range(w):
        t = abs((col - w / 2) / max(1, w / 2))  # 0 at center, 1 at edges
        col_h = int(edge_h + (center_h - edge_h) * (1 - t * t)) - backing_h
        col_h = max(1, col_h)
        brightness = int(5 * (1 - t))
        color = (25 + brightness, 22 + brightness, 18 + brightness)
        px = x0 + col
        d.line([(px, ablative_top), (px, ablative_top + col_h)],
               fill=color, width=1)

    # Separation line between backing and ablative
    d.line([(x0, ablative_top), (x1, ablative_top)],
           fill=STEEL_DARK, width=1)

    # Top outline
    d.line([(x0, y_top), (x1, y_top)], fill=STEEL_VERY_DARK, width=1)
    d.line([(x0, y_top), (x0, ablative_top + edge_h - backing_h)],
           fill=STEEL_VERY_DARK, width=1)
    d.line([(x1, y_top), (x1, ablative_top + edge_h - backing_h)],
           fill=STEEL_VERY_DARK, width=1)

    return img


def generate_rcs(spec):
    """RCS thruster block: compact body with prominent nozzle cones."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    mirrored = spec.get("mirrored", False)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2
    cy = ch // 2

    # Compact central body (heavily inset)
    body_inset_x = max(3, w // 3)
    body_inset_y = max(3, h // 3)
    bx0 = x0 + body_inset_x
    by0 = y0 + body_inset_y
    bx1 = x1 - body_inset_x
    by1 = y1 - body_inset_y
    d.rectangle([bx0, by0, bx1, by1], fill=(60, 63, 70), outline=STEEL_VERY_DARK)

    # Thin, long nozzle trapezoids extending to part edges
    nz_color = (45, 47, 52)
    nz_throat = max(2, min(w, h) // 6)  # narrow at body
    nz_exit = max(3, nz_throat * 3 // 2)  # flares outward at exit

    if mirrored:
        # Left-mount: side nozzle points right
        d.polygon([
            (bx1, cy - nz_throat), (x1, cy - nz_exit),
            (x1, cy + nz_exit), (bx1, cy + nz_throat),
        ], fill=nz_color, outline=STEEL_VERY_DARK)
    else:
        # Right-mount: side nozzle points left
        d.polygon([
            (bx0, cy - nz_throat), (x0, cy - nz_exit),
            (x0, cy + nz_exit), (bx0, cy + nz_throat),
        ], fill=nz_color, outline=STEEL_VERY_DARK)

    # Top nozzle
    d.polygon([
        (cx - nz_throat, by0), (cx - nz_exit, y0),
        (cx + nz_exit, y0), (cx + nz_throat, by0),
    ], fill=nz_color, outline=STEEL_VERY_DARK)

    # Bottom nozzle
    d.polygon([
        (cx - nz_throat, by1), (cx - nz_exit, y1),
        (cx + nz_exit, y1), (cx + nz_throat, by1),
    ], fill=nz_color, outline=STEEL_VERY_DARK)

    return img


def generate_battery(spec):
    """Battery bank: black rectangle with horizontal cell lines and terminals."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2

    # Body — black
    body_color = (22, 22, 26)
    d.rectangle([x0, y0, x1, y1], fill=body_color)

    # Vertical cell division lines
    n_cells = max(2, int(gw))
    cell_w = w / n_cells
    for i in range(1, n_cells):
        lx = x0 + int(i * cell_w)
        d.line([(lx, y0 + 3), (lx, y1 - 3)], fill=(35, 35, 40), width=1)

    # Terminal indicators at top
    term_h = max(3, h // 6)
    d.rectangle([x0, y0, x1, y0 + term_h], fill=(38, 38, 44))

    # Terminal dots
    n_terms = max(1, int(gw) // 2)
    term_spacing = w / (n_terms + 1)
    for i in range(1, n_terms + 1):
        tx = x0 + int(i * term_spacing)
        tr = max(2, min(5, int(PX * 0.06)))
        circ(d, tx, y0 + term_h // 2, tr, fill=(180, 200, 80))

    # Metallic outline
    d.rectangle([x0, y0, x1, y1], outline=STEEL_MID)

    return img


def generate_solar_panel(spec):
    """Solar panel: dark navy with bright blue photovoltaic cell grid."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h

    # Frame (silver)
    frame_color = (140, 145, 155)
    d.rectangle([x0, y0, x1, y1], fill=frame_color)

    # Panel hinge at bottom
    hinge_h = max(4, int(PX * 0.08))
    d.rectangle([x0, y1 - hinge_h, x1, y1], fill=STEEL_MID)
    d.line([(x0, y1 - hinge_h), (x1, y1 - hinge_h)], fill=STEEL_DARK, width=1)

    # Photovoltaic cell area (inset)
    inset = max(2, int(PX * 0.04))
    cell_color = (40, 80, 160)
    cell_x0 = x0 + inset
    cell_y0 = y0 + inset
    cell_x1 = x1 - inset
    cell_y1 = y1 - hinge_h - inset
    d.rectangle([cell_x0, cell_y0, cell_x1, cell_y1], fill=cell_color)

    # Cell grid lines
    cell_h = cell_y1 - cell_y0
    cell_w = cell_x1 - cell_x0
    grid_line_color = (30, 60, 130)

    # Horizontal grid (every ~0.5 grid units)
    n_rows = max(2, int(gh * 2) - 1)
    for i in range(1, n_rows):
        ly = cell_y0 + int(i * cell_h / n_rows)
        d.line([(cell_x0, ly), (cell_x1, ly)], fill=grid_line_color, width=1)

    # Vertical center line
    if cell_w > 20:
        d.line([(cell_x0 + cell_w // 2, cell_y0), (cell_x0 + cell_w // 2, cell_y1)],
               fill=grid_line_color, width=1)

    # Specular highlight
    highlight_color = (60, 110, 200)
    hl_y = cell_y0 + cell_h // 4
    hl_h = max(2, cell_h // 8)
    d.rectangle([cell_x0 + 2, hl_y, cell_x1 - 2, hl_y + hl_h], fill=highlight_color)

    # Outline
    d.rectangle([x0, y0, x1, y1], outline=STEEL_DARK)

    return img


def generate_rtg(spec):
    """RTG: dark grey body with red/orange heat dissipation fins."""
    gw, gh = spec["grid_width"], spec["grid_height"]
    PX = _capped_px(gw, gh)
    w, h = int(gw * PX), int(gh * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    cx = cw // 2

    # Body — dark grey
    body_color = (55, 55, 60)
    d.rectangle([x0, y0, x1, y1], fill=body_color)

    # Heat dissipation fin stripes (red/orange horizontal bands)
    n_fins = max(4, int(gh * 3))
    fin_spacing = h / (n_fins + 1)
    for i in range(1, n_fins + 1):
        fy = y0 + int(i * fin_spacing)
        t = i / (n_fins + 1)
        fin_color = lerp_color((180, 60, 30), (200, 120, 40), t)
        d.line([(x0 + 2, fy), (x1 - 2, fy)], fill=fin_color, width=2)

    # Top cap
    cap_h = max(4, int(PX * 0.1))
    d.rectangle([x0, y0, x1, y0 + cap_h], fill=STEEL_MID)

    # Bottom cap
    d.rectangle([x0, y1 - cap_h, x1, y1], fill=STEEL_MID)

    # Industrial warning mark (small yellow rectangle near top)
    mark_w = max(4, w // 4)
    mark_h = max(3, int(PX * 0.06))
    d.rectangle([cx - mark_w // 2, y0 + cap_h + 4,
                 cx + mark_w // 2, y0 + cap_h + 4 + mark_h],
                fill=(200, 180, 40))

    # Outline
    d.rectangle([x0, y0, x1, y1], outline=STEEL_DARK)

    return img


# ================================================================
# Part catalog
# ================================================================

PARTS = {
    # --- PODS ---
    "pod_small": {
        "type": "pod", "grid_width": 3.0, "grid_height": 3.0,
        "top_width": 0.5, "shape": "Trapezoid",
    },
    "pod_medium": {
        "type": "pod", "grid_width": 5.0, "grid_height": 4.0,
        "top_width": 1.0, "shape": "Trapezoid",
    },

    # --- TANKS (Tiny) ---
    "tank_tiny_1":  {"type": "tank", "grid_width": 1.0, "grid_height": 1.0},
    "tank_tiny_2":  {"type": "tank", "grid_width": 1.0, "grid_height": 2.0},
    "tank_tiny_4":  {"type": "tank", "grid_width": 1.0, "grid_height": 4.0},
    "tank_tiny_8":  {"type": "tank", "grid_width": 1.0, "grid_height": 8.0},
    # Small
    "tank_small_1": {"type": "tank", "grid_width": 3.0, "grid_height": 1.0},
    "tank_small_2": {"type": "tank", "grid_width": 3.0, "grid_height": 2.0},
    "tank_small_4": {"type": "tank", "grid_width": 3.0, "grid_height": 4.0},
    "tank_small_8": {"type": "tank", "grid_width": 3.0, "grid_height": 8.0},
    # Medium
    "tank_medium_1":  {"type": "tank", "grid_width": 5.0, "grid_height": 1.0},
    "tank_medium_2":  {"type": "tank", "grid_width": 5.0, "grid_height": 2.0},
    "tank_medium_4":  {"type": "tank", "grid_width": 5.0, "grid_height": 4.0},
    "tank_medium_8":  {"type": "tank", "grid_width": 5.0, "grid_height": 8.0},
    "tank_medium_16": {"type": "tank", "grid_width": 5.0, "grid_height": 16.0},
    # Large
    "tank_large_1":  {"type": "tank", "grid_width": 9.0, "grid_height": 1.0},
    "tank_large_2":  {"type": "tank", "grid_width": 9.0, "grid_height": 2.0},
    "tank_large_4":  {"type": "tank", "grid_width": 9.0, "grid_height": 4.0},
    "tank_large_8":  {"type": "tank", "grid_width": 9.0, "grid_height": 8.0},
    "tank_large_16": {"type": "tank", "grid_width": 9.0, "grid_height": 16.0},
    # XL
    "tank_xl_1":  {"type": "tank", "grid_width": 13.0, "grid_height": 1.0},
    "tank_xl_2":  {"type": "tank", "grid_width": 13.0, "grid_height": 2.0},
    "tank_xl_4":  {"type": "tank", "grid_width": 13.0, "grid_height": 4.0},
    "tank_xl_8":  {"type": "tank", "grid_width": 13.0, "grid_height": 8.0},
    "tank_xl_16": {"type": "tank", "grid_width": 13.0, "grid_height": 16.0},

    # --- DECOUPLERS ---
    "decoupler_tiny":   {"type": "decoupler", "grid_width": 1.0,  "grid_height": 0.5},
    "decoupler_small":  {"type": "decoupler", "grid_width": 3.0,  "grid_height": 0.5},
    "decoupler_medium": {"type": "decoupler", "grid_width": 5.0,  "grid_height": 0.5},
    "decoupler_large":  {"type": "decoupler", "grid_width": 9.0,  "grid_height": 0.5},
    "decoupler_xl":     {"type": "decoupler", "grid_width": 13.0, "grid_height": 0.5},
    "decoupler_radial": {"type": "decoupler_radial", "grid_width": 1.0, "grid_height": 2.0},

    # --- FAIRINGS ---
    "fairing_tiny":   {"type": "fairing", "grid_width": 1.0,  "grid_height": 1.0},
    "fairing_small":  {"type": "fairing", "grid_width": 3.0,  "grid_height": 1.0},
    "fairing_medium": {"type": "fairing", "grid_width": 5.0,  "grid_height": 1.0},
    "fairing_large":  {"type": "fairing", "grid_width": 9.0,  "grid_height": 1.0},
    "fairing_xl":     {"type": "fairing", "grid_width": 13.0, "grid_height": 1.0},

    # --- NOSE CONES (standard) ---
    "nosecone_tiny":   {"type": "nosecone", "grid_width": 1.0, "grid_height": 2.0,  "shape": "Triangle"},
    "nosecone_small":  {"type": "nosecone", "grid_width": 3.0, "grid_height": 4.0,  "shape": "Triangle"},
    "nosecone_medium": {"type": "nosecone", "grid_width": 5.0, "grid_height": 6.0,  "shape": "Triangle"},
    "nosecone_large":  {"type": "nosecone", "grid_width": 9.0, "grid_height": 10.0, "shape": "Triangle"},
    # Right side
    "nosecone_side_right_tiny":   {"type": "nosecone", "grid_width": 1.0, "grid_height": 2.0,  "shape": "TriangleRight"},
    "nosecone_side_right_small":  {"type": "nosecone", "grid_width": 3.0, "grid_height": 4.0,  "shape": "TriangleRight"},
    "nosecone_side_right_medium": {"type": "nosecone", "grid_width": 5.0, "grid_height": 6.0,  "shape": "TriangleRight"},
    "nosecone_side_right_large":  {"type": "nosecone", "grid_width": 9.0, "grid_height": 10.0, "shape": "TriangleRight"},
    # Left side
    "nosecone_side_left_tiny":   {"type": "nosecone", "grid_width": 1.0, "grid_height": 2.0,  "shape": "TriangleLeft"},
    "nosecone_side_left_small":  {"type": "nosecone", "grid_width": 3.0, "grid_height": 4.0,  "shape": "TriangleLeft"},
    "nosecone_side_left_medium": {"type": "nosecone", "grid_width": 5.0, "grid_height": 6.0,  "shape": "TriangleLeft"},
    "nosecone_side_left_large":  {"type": "nosecone", "grid_width": 9.0, "grid_height": 10.0, "shape": "TriangleLeft"},

    # --- HEAT SHIELDS ---
    "heatshield_tiny":   {"type": "heatshield", "grid_width": 1.0, "grid_height": 0.5},
    "heatshield_small":  {"type": "heatshield", "grid_width": 3.0, "grid_height": 0.5},
    "heatshield_medium": {"type": "heatshield", "grid_width": 5.0, "grid_height": 0.5},
    "heatshield_large":  {"type": "heatshield", "grid_width": 9.0, "grid_height": 0.5},

    # --- RCS ---
    "rcs_small":       {"type": "rcs", "grid_width": 0.5, "grid_height": 0.75, "mirrored": False},
    "rcs_small_left":  {"type": "rcs", "grid_width": 0.5, "grid_height": 0.75, "mirrored": True},
    "rcs_medium":      {"type": "rcs", "grid_width": 0.5, "grid_height": 1.0,  "mirrored": False},
    "rcs_medium_left": {"type": "rcs", "grid_width": 0.5, "grid_height": 1.0,  "mirrored": True},

    # --- BATTERIES ---
    "battery_z1":      {"type": "battery", "grid_width": 1.0,  "grid_height": 1.0},
    "battery_z3":      {"type": "battery", "grid_width": 3.0,  "grid_height": 1.0},
    "battery_z5":      {"type": "battery", "grid_width": 5.0,  "grid_height": 1.0},
    "battery_z9":      {"type": "battery", "grid_width": 9.0,  "grid_height": 1.0},
    "battery_z13":     {"type": "battery", "grid_width": 13.0, "grid_height": 1.0},

    # --- SOLAR PANELS ---
    "solar_sp3":       {"type": "solar_panel", "grid_width": 1.0, "grid_height": 3.0},
    "solar_sp6":       {"type": "solar_panel", "grid_width": 1.0, "grid_height": 6.0},
    "solar_sp12":      {"type": "solar_panel", "grid_width": 2.0, "grid_height": 6.0},
    "solar_sp24":      {"type": "solar_panel", "grid_width": 2.0, "grid_height": 12.0},

    # --- RTG ---
    "rtg_pbnuk":       {"type": "rtg", "grid_width": 1.0, "grid_height": 2.0},
}


# ================================================================
# Main
# ================================================================

def generate_part(name, spec):
    ptype = spec["type"]
    generators = {
        "tank": generate_tank,
        "pod": generate_pod,
        "decoupler": generate_decoupler,
        "decoupler_radial": generate_decoupler_radial,
        "fairing": generate_fairing,
        "nosecone": generate_nosecone,
        "heatshield": generate_heatshield,
        "rcs": generate_rcs,
        "battery": generate_battery,
        "solar_panel": generate_solar_panel,
        "rtg": generate_rtg,
    }
    gen = generators.get(ptype)
    if gen is None:
        raise ValueError(f"Unknown part type: {ptype}")
    return gen(spec)


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    output_dir = os.path.join(project_root, "data", "sprites", "parts")
    os.makedirs(output_dir, exist_ok=True)

    for name, spec in PARTS.items():
        img = generate_part(name, spec)
        out = os.path.join(output_dir, f"{name}.png")
        img.save(out)

        gw, gh = spec["grid_width"], spec["grid_height"]
        expected_w = int(gw * PX) + PAD * 2
        expected_h = int(gh * PX) + PAD * 2
        ok = "OK" if img.size == (expected_w, expected_h) else "MISMATCH"
        print(f"  {name:35s}  {img.size[0]:4d}x{img.size[1]:4d}  "
              f"({gw}x{gh} grid)  [{ok}]")

    print(f"\nGenerated {len(PARTS)} sprites in {output_dir}")
