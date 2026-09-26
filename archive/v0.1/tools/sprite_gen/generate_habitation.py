#!/usr/bin/env python3
"""
Generate sprites for habitation, control, greenhouse, and cargo parts.

Command Pod:
  pod_large (Large 9x5) — capsule with viewport

Inline Control Centers:
  control_small (Small 3x2), control_medium (Medium 5x2),
  control_large (Large 9x3), control_xl (XL 13x4)
  Rectangular pass-through modules with control displays and indicators.

Crew Quarters:
  quarters_small (Small 3x4), quarters_medium (Medium 5x5),
  quarters_large (Large 9x6)
  Habitation modules with porthole windows and warm interior lighting.

Greenhouses:
  greenhouse_small (Small 3x4), greenhouse_medium (Medium 5x5),
  greenhouse_large (Large 9x6)
  Growth modules with translucent green panels and UV grow strips.

Cargo Containers:
  cargo_tiny (Tiny 1x2), cargo_small (Small 3x3),
  cargo_medium (Medium 5x4), cargo_large (Large 9x5)
  Utilitarian storage boxes with reinforcement ribs and cargo markings.

All parts use PX=360 px/grid, PAD=0.
"""

from PIL import Image, ImageDraw
import math
import os

# ================================================================
# Constants
# ================================================================
PX = 360
MAX_SPRITE_PX = 4096
PAD = 0

def _capped_px(w, h):
    """Return effective PX capped so the output image stays within MAX_SPRITE_PX."""
    return min(PX, MAX_SPRITE_PX // max(w, h))

# ================================================================
# Palette
# ================================================================
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)

# Pod hull
POD_HULL = (55, 58, 65)
POD_HULL_OUTLINE = (38, 40, 45)

# Control module (dark blue-grey electronics)
CTRL_BODY = (42, 48, 58)
CTRL_DARK = (30, 34, 42)
CTRL_LIGHT = (62, 68, 80)
CTRL_SCREEN = (30, 80, 60)
CTRL_SCREEN_BRIGHT = (50, 140, 90)
CTRL_LED_GREEN = (40, 180, 40)
CTRL_LED_AMBER = (200, 160, 40)

# Quarters (warm grey, livable)
QUARTERS_BODY = (70, 68, 72)
QUARTERS_DARK = (50, 48, 52)
QUARTERS_LIGHT = (95, 92, 96)
WINDOW_FRAME = (60, 62, 68)
WINDOW_WARM = (180, 160, 100)
WINDOW_WARM_BRIGHT = (220, 200, 140)

# Greenhouse (white-grey with green accents)
GREEN_BODY = (75, 78, 72)
GREEN_DARK = (52, 55, 48)
GREEN_LIGHT = (100, 104, 95)
GREEN_PANEL = (35, 90, 45)
GREEN_PANEL_BRIGHT = (55, 140, 65)
GREEN_GLOW = (80, 180, 80)
UV_STRIP = (140, 100, 220)

# Cargo (industrial tan/olive)
CARGO_BODY = (88, 85, 75)
CARGO_DARK = (62, 60, 52)
CARGO_LIGHT = (112, 108, 98)
CARGO_HIGHLIGHT = (135, 130, 120)
CARGO_RIB = (78, 75, 65)
CARGO_MARKING = (180, 140, 60)


# ================================================================
# Drawing primitives
# ================================================================

def trap(d, cx, y, tw, bw, h, fill, outline=None):
    d.polygon([(cx - tw / 2, y), (cx + tw / 2, y),
               (cx + bw / 2, y + h), (cx - bw / 2, y + h)],
              fill=fill, outline=outline)


def circ(d, cx, cy, r, **kw):
    d.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)


def rect(d, cx, y, w, h, fill, outline=None):
    d.rectangle([cx - w / 2, y, cx + w / 2, y + h], fill=fill, outline=outline)


def lerp_color(c1, c2, t):
    t = max(0.0, min(1.0, t))
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(len(c1)))


def draw_bolts(d, cx, y, w, h, n, r=3):
    hw = int(w / 2)
    for i in range(n):
        if n > 1:
            bx = cx - hw + r * 2 + i * int((w - r * 4) / (n - 1))
        else:
            bx = cx
        circ(d, bx, y + h // 2, r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y + h // 2, max(1, r - 1), fill=STEEL_DARK)


def draw_mount_ring(d, cx, y, w, h, scale=1.0):
    bolt_r = max(2, int(3 * scale))
    n_bolts = max(3, int(w / (bolt_r * 8)))
    trap(d, cx, y, w, w, h, STEEL_MID, outline=STEEL_DARK)
    hw = int(w / 2)
    d.line([(cx - hw + 2, y + 1), (cx + hw - 2, y + 1)],
           fill=STEEL_HIGHLIGHT, width=max(1, int(scale)))
    draw_bolts(d, cx, y, w, h, n_bolts, r=bolt_r)
    return y + h


# ================================================================
# Pod generator (large command pod)
# ================================================================

def generate_pod_large():
    """Large command pod: 9x5 trapezoid hull with viewport windows."""
    gw, gh = 9.0, 5.0
    tw_grid = 3.0
    PX = _capped_px(int(gw), int(gh))
    w, h = int(gw * PX), int(gh * PX)
    tw = int(tw_grid * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    y0, y1 = PAD, PAD + h

    # Hull
    trap(d, cx, y0, tw, w, h, POD_HULL, outline=POD_HULL_OUTLINE)

    # Left edge highlight for 3D effect
    for i in range(h):
        frac = i / max(1, h)
        lw = tw / 2 + (w / 2 - tw / 2) * frac
        px = cx - int(lw) + 2
        py = y0 + i
        c = lerp_color((75, 78, 88), (65, 68, 78), frac)
        d.point((px, py), fill=c)
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

    # Main viewport window (center)
    win_frac = 0.30
    win_y = y0 + int(h * win_frac)
    w_at = tw + (w - tw) * win_frac
    win_r = max(6, int(min(w_at, h) * 0.10))

    circ(d, cx, win_y, win_r + 2, fill=(100, 130, 170))
    circ(d, cx, win_y, win_r, fill=(140, 180, 220))
    circ(d, cx - win_r // 3, win_y - win_r // 3, max(2, win_r // 3),
         fill=(180, 210, 240))
    circ(d, cx, win_y, win_r, outline=(50, 52, 58))

    # Side viewport windows (two flanking windows)
    side_r = max(4, int(win_r * 0.65))
    side_offset = int(w_at * 0.22)
    for sx in [-1, 1]:
        swx = cx + sx * side_offset
        swy = win_y + int(h * 0.06)
        circ(d, swx, swy, side_r + 2, fill=(100, 130, 170))
        circ(d, swx, swy, side_r, fill=(140, 180, 220))
        circ(d, swx - side_r // 3, swy - side_r // 3, max(1, side_r // 3),
             fill=(180, 210, 240))
        circ(d, swx, swy, side_r, outline=(50, 52, 58))

    return img


# ================================================================
# Inline Control Center generator
# ================================================================

def generate_control(size):
    """Inline control center: crewed command module with wide observation windows."""
    sizes = {
        "small":  (3, 2),
        "medium": (5, 2),
        "large":  (9, 3),
        "xl":     (13, 4),
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)
    w, h = int(GW * PX), int(GH * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    scale = max(0.4, w / 270.0)

    # Main body (command module hull — slightly lighter than quarters)
    body_color = (58, 62, 70)
    body_dark = (38, 42, 48)
    body_light = (78, 82, 92)
    d.rectangle([x0, y0, x1, y1], fill=body_color, outline=body_dark)

    # Top and bottom interface rings (same width — pass-through)
    ring_h = max(4, int(h * 0.12))
    d.rectangle([x0, y0, x1, y0 + ring_h], fill=STEEL_MID)
    d.rectangle([x0, y1 - ring_h, x1, y1], fill=STEEL_MID)
    d.line([(x0, y0 + ring_h), (x1, y0 + ring_h)], fill=body_dark, width=1)
    d.line([(x0, y1 - ring_h), (x1, y1 - ring_h)], fill=body_dark, width=1)
    d.line([(x0 + 2, y0 + 1), (x1 - 2, y0 + 1)], fill=STEEL_HIGHLIGHT, width=1)
    d.line([(x0 + 2, y1 - 1), (x1 - 2, y1 - 1)], fill=STEEL_LIGHT, width=1)

    # Bolts on rings
    bolt_r = max(2, int(3 * scale))
    n_bolts = max(2, int(GW * 1.2))
    draw_bolts(d, cx, y0, w, ring_h, n_bolts, r=bolt_r)
    draw_bolts(d, cx, y1 - ring_h, w, ring_h, n_bolts, r=bolt_r)

    inner_top = y0 + ring_h
    inner_bot = y1 - ring_h
    inner_h = inner_bot - inner_top

    # Left edge highlight
    d.line([(x0 + 2, inner_top + 2), (x0 + 2, inner_bot - 2)],
           fill=body_light, width=max(1, int(2 * scale)))

    # Vertical panel seams
    n_seams = max(1, int(GW * 0.4))
    seam_spacing = w / (n_seams + 1)
    for i in range(n_seams):
        sx = x0 + int((i + 1) * seam_spacing)
        d.line([(sx, inner_top + 2), (sx, inner_bot - 2)], fill=body_dark, width=1)

    # --- Wide panoramic observation windows (rectangular, command bridge style) ---
    # These are wide horizontal windows — distinct from the round portholes on quarters
    win_margin_x = max(6, int(w * 0.08))
    win_margin_y = max(4, int(inner_h * 0.15))
    win_area_w = w - win_margin_x * 2
    win_area_h = inner_h - win_margin_y * 2

    # Window colors — cooler blue-white (star-gazing, command bridge feel)
    win_glass = (130, 165, 210)
    win_glass_bright = (175, 205, 240)
    win_frame = (45, 48, 55)
    win_frame_light = (90, 95, 105)

    # Determine window layout based on size
    if GH <= 2:
        # Short modules: single row of wide horizontal windows
        n_rows = 1
    else:
        # Taller modules: two rows
        n_rows = 2

    n_cols = max(1, int(GW * 0.6))
    gap = max(3, int(6 * scale))

    row_h = (win_area_h - gap * (n_rows - 1)) // n_rows
    col_w = (win_area_w - gap * (n_cols - 1)) // n_cols

    for row in range(n_rows):
        wy = inner_top + win_margin_y + row * (row_h + gap)
        for col in range(n_cols):
            wx = x0 + win_margin_x + col * (col_w + gap)

            # Window frame (dark border)
            frame_t = max(2, int(3 * scale))
            d.rectangle([wx, wy, wx + col_w, wy + row_h],
                        fill=win_frame)

            # Glass area (inset from frame)
            gx0 = wx + frame_t
            gy0 = wy + frame_t
            gx1 = wx + col_w - frame_t
            gy1 = wy + row_h - frame_t

            if gx1 <= gx0 or gy1 <= gy0:
                continue

            # Glass gradient (brighter at top — light from stars)
            glass_h = gy1 - gy0
            for pr in range(glass_h):
                frac = pr / max(1, glass_h - 1)
                c = lerp_color(win_glass_bright, win_glass, frac * 0.7)
                d.line([(gx0, gy0 + pr), (gx1, gy0 + pr)], fill=c, width=1)

            # Specular highlight (bright strip near top)
            hl_h = max(1, glass_h // 5)
            for pr in range(hl_h):
                frac = pr / max(1, hl_h)
                c = lerp_color((200, 225, 250), win_glass_bright, frac)
                d.line([(gx0 + 2, gy0 + pr), (gx1 - 2, gy0 + pr)], fill=c, width=1)

            # Frame edge highlights
            d.line([(wx + 1, wy + 1), (wx + col_w - 1, wy + 1)],
                   fill=win_frame_light, width=1)
            d.line([(wx + 1, wy + 1), (wx + 1, wy + row_h - 1)],
                   fill=win_frame_light, width=1)

            # Frame corner rivets
            rivet_r = max(1, int(1.5 * scale))
            for rvx, rvy in [(wx + frame_t, wy + frame_t),
                              (wx + col_w - frame_t, wy + frame_t),
                              (wx + frame_t, wy + row_h - frame_t),
                              (wx + col_w - frame_t, wy + row_h - frame_t)]:
                circ(d, rvx, rvy, rivet_r, fill=STEEL_HIGHLIGHT)

    # Structural band between window rows (if multi-row)
    if n_rows > 1:
        band_y = inner_top + win_margin_y + row_h + gap // 2
        d.line([(x0 + 3, band_y), (x1 - 3, band_y)],
               fill=body_light, width=max(2, int(3 * scale)))

    return img


# ================================================================
# Crew Quarters generator
# ================================================================

def generate_quarters(size):
    """Crew quarters: habitation module with porthole windows."""
    sizes = {
        "small":  (3, 4),
        "medium": (5, 5),
        "large":  (9, 6),
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)
    w, h = int(GW * PX), int(GH * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    scale = max(0.4, w / 270.0)

    # Main body
    d.rectangle([x0, y0, x1, y1], fill=QUARTERS_BODY, outline=QUARTERS_DARK)

    # Top and bottom interface rings
    ring_h = max(5, int(h * 0.08))
    d.rectangle([x0, y0, x1, y0 + ring_h], fill=STEEL_MID)
    d.rectangle([x0, y1 - ring_h, x1, y1], fill=STEEL_MID)
    d.line([(x0, y0 + ring_h), (x1, y0 + ring_h)], fill=QUARTERS_DARK, width=1)
    d.line([(x0, y1 - ring_h), (x1, y1 - ring_h)], fill=QUARTERS_DARK, width=1)
    d.line([(x0 + 2, y0 + 1), (x1 - 2, y0 + 1)], fill=STEEL_HIGHLIGHT, width=1)

    # Bolts on rings
    bolt_r = max(2, int(3 * scale))
    n_bolts = max(2, int(GW * 1.2))
    draw_bolts(d, cx, y0, w, ring_h, n_bolts, r=bolt_r)
    draw_bolts(d, cx, y1 - ring_h, w, ring_h, n_bolts, r=bolt_r)

    inner_top = y0 + ring_h
    inner_bot = y1 - ring_h
    inner_h = inner_bot - inner_top

    # Left edge highlight
    d.line([(x0 + 2, inner_top + 2), (x0 + 2, inner_bot - 2)],
           fill=QUARTERS_LIGHT, width=max(1, int(2 * scale)))

    # Horizontal structural bands (every ~2 grid units)
    band_spacing = int(2.0 * PX)
    band_color = QUARTERS_LIGHT
    by = inner_top + band_spacing
    while by < inner_bot - 10:
        d.line([(x0 + 2, by), (x1 - 2, by)], fill=band_color, width=max(2, int(3 * scale)))
        d.line([(x0 + 2, by + max(2, int(3 * scale))), (x1 - 2, by + max(2, int(3 * scale)))],
               fill=QUARTERS_DARK, width=1)
        by += band_spacing

    # Vertical panel seam
    d.line([(cx, inner_top + 2), (cx, inner_bot - 2)],
           fill=QUARTERS_DARK, width=1)

    # Porthole windows — rows of circular windows with warm interior light
    margin_x = max(8, int(w * 0.12))
    margin_y = max(8, int(inner_h * 0.12))
    win_area_w = w - margin_x * 2
    win_area_h = inner_h - margin_y * 2

    # Determine window grid
    win_r = max(5, int(min(PX * 0.35, win_area_w / (GW * 0.8))))
    win_spacing_x = max(win_r * 3, int(win_area_w / max(1, GW - 1)))
    n_cols = max(1, int(win_area_w / win_spacing_x))
    n_rows = max(1, int(win_area_h / (win_r * 3.5)))

    for row in range(n_rows):
        wy = inner_top + margin_y + int((row + 0.5) * win_area_h / n_rows)
        for col in range(n_cols):
            wx = x0 + margin_x + int((col + 0.5) * win_area_w / n_cols)
            # Window frame (dark ring)
            circ(d, wx, wy, win_r + 2, fill=WINDOW_FRAME)
            # Window glass (warm interior light)
            circ(d, wx, wy, win_r, fill=WINDOW_WARM)
            # Interior highlight (offset for depth)
            circ(d, wx - win_r // 4, wy - win_r // 4, max(2, win_r // 3),
                 fill=WINDOW_WARM_BRIGHT)
            # Frame outline
            circ(d, wx, wy, win_r + 2, outline=QUARTERS_DARK)
            # Frame rivet detail (4 small dots at compass points)
            rivet_r = max(1, int(1.5 * scale))
            for angle in [0, 90, 180, 270]:
                rad = math.radians(angle)
                rx = wx + int((win_r + 1) * math.cos(rad))
                ry = wy + int((win_r + 1) * math.sin(rad))
                circ(d, rx, ry, rivet_r, fill=STEEL_HIGHLIGHT)

    # Life support vent (small rectangle near top)
    vent_w = max(8, int(w * 0.15))
    vent_h = max(3, int(PX * 0.06))
    vent_y = inner_top + margin_y // 2 - vent_h // 2
    for side in [-1, 1]:
        vx = cx + side * int(w * 0.32)
        d.rectangle([vx - vent_w // 2, vent_y, vx + vent_w // 2, vent_y + vent_h],
                    fill=QUARTERS_DARK)
        # Vent slats
        n_slats = max(2, int(vent_w / (4 * scale)))
        for si in range(n_slats):
            slat_x = vx - vent_w // 2 + int((si + 0.5) * vent_w / n_slats)
            d.line([(slat_x, vent_y + 1), (slat_x, vent_y + vent_h - 1)],
                   fill=STEEL_DARK, width=1)

    return img


# ================================================================
# Greenhouse generator
# ================================================================

def generate_greenhouse(size):
    """Greenhouse: growth module with translucent green panels and UV strips."""
    sizes = {
        "small":  (3, 4),
        "medium": (5, 5),
        "large":  (9, 6),
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)
    w, h = int(GW * PX), int(GH * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    scale = max(0.4, w / 270.0)

    # Main structural frame
    d.rectangle([x0, y0, x1, y1], fill=GREEN_BODY, outline=GREEN_DARK)

    # Top and bottom interface rings
    ring_h = max(5, int(h * 0.08))
    d.rectangle([x0, y0, x1, y0 + ring_h], fill=STEEL_MID)
    d.rectangle([x0, y1 - ring_h, x1, y1], fill=STEEL_MID)
    d.line([(x0, y0 + ring_h), (x1, y0 + ring_h)], fill=GREEN_DARK, width=1)
    d.line([(x0, y1 - ring_h), (x1, y1 - ring_h)], fill=GREEN_DARK, width=1)
    d.line([(x0 + 2, y0 + 1), (x1 - 2, y0 + 1)], fill=STEEL_HIGHLIGHT, width=1)

    # Bolts
    bolt_r = max(2, int(3 * scale))
    n_bolts = max(2, int(GW * 1.2))
    draw_bolts(d, cx, y0, w, ring_h, n_bolts, r=bolt_r)
    draw_bolts(d, cx, y1 - ring_h, w, ring_h, n_bolts, r=bolt_r)

    inner_top = y0 + ring_h
    inner_bot = y1 - ring_h
    inner_h = inner_bot - inner_top

    # Structural frame members (vertical beams dividing panels)
    frame_w = max(3, int(5 * scale))
    n_bays = max(1, int(GW * 0.5))
    bay_w = w / (n_bays)
    frame_positions = []
    # Left edge, panel dividers, right edge
    for i in range(n_bays + 1):
        fx = x0 + int(i * bay_w)
        frame_positions.append(fx)

    # Horizontal frame members
    h_frame_positions = []
    n_h_frames = max(1, int(GH * 0.5))
    for i in range(n_h_frames + 1):
        fy = inner_top + int(i * inner_h / n_h_frames)
        h_frame_positions.append(fy)

    # Draw green translucent panels (between frame members)
    for col in range(n_bays):
        px0 = frame_positions[col] + frame_w // 2 + 1
        px1 = frame_positions[col + 1] - frame_w // 2 - 1
        if px0 >= px1:
            continue
        for row in range(n_h_frames):
            py0 = h_frame_positions[row] + frame_w // 2 + 1
            py1 = h_frame_positions[row + 1] - frame_w // 2 - 1
            if py0 >= py1:
                continue

            # Green panel with depth gradient
            panel_w = px1 - px0
            panel_h = py1 - py0
            for pr in range(panel_h):
                frac = pr / max(1, panel_h - 1)
                # Slightly brighter at top (light coming in)
                c = lerp_color(GREEN_PANEL_BRIGHT, GREEN_PANEL, frac * 0.6 + 0.2)
                d.line([(px0, py0 + pr), (px1, py0 + pr)], fill=c, width=1)

            # Interior vegetation silhouettes (dark green shapes at bottom of panel)
            veg_h = max(3, int(panel_h * 0.35))
            for vx in range(px0 + 2, px1 - 1, max(2, int(4 * scale))):
                # Random-ish vegetation height using position-based hash
                seed = (vx * 37 + col * 13 + row * 7) % 17
                vh = max(2, int(veg_h * (0.4 + 0.6 * (seed / 16.0))))
                veg_top = py1 - vh
                veg_color = lerp_color((20, 55, 25), (30, 75, 35), seed / 16.0)
                d.line([(vx, veg_top), (vx, py1 - 1)], fill=veg_color, width=max(1, int(2 * scale)))

            # Panel edge highlights
            d.line([(px0, py0), (px1, py0)],
                   fill=GREEN_GLOW, width=1)

    # Draw structural frame members (over panels)
    for fx in frame_positions:
        d.rectangle([fx - frame_w // 2, inner_top, fx + frame_w // 2, inner_bot],
                    fill=GREEN_LIGHT, outline=GREEN_DARK)
    for fy in h_frame_positions:
        d.rectangle([x0, fy - frame_w // 2, x1, fy + frame_w // 2],
                    fill=GREEN_LIGHT, outline=GREEN_DARK)

    # UV grow light strips (purple lines at top of each row)
    uv_h = max(2, int(3 * scale))
    for row in range(n_h_frames):
        uv_y = h_frame_positions[row] + frame_w // 2 + 2
        for col in range(n_bays):
            ux0 = frame_positions[col] + frame_w // 2 + 2
            ux1 = frame_positions[col + 1] - frame_w // 2 - 2
            if ux0 < ux1:
                d.rectangle([ux0, uv_y, ux1, uv_y + uv_h], fill=UV_STRIP)
                # Subtle glow below strip
                d.line([(ux0 + 1, uv_y + uv_h + 1), (ux1 - 1, uv_y + uv_h + 1)],
                       fill=lerp_color(UV_STRIP, GREEN_PANEL, 0.5), width=1)

    return img


# ================================================================
# Cargo Container generator
# ================================================================

def generate_cargo(size):
    """Cargo container: utilitarian storage with ribs and markings."""
    sizes = {
        "tiny":   (1, 2),
        "small":  (3, 3),
        "medium": (5, 4),
        "large":  (9, 5),
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)
    w, h = int(GW * PX), int(GH * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    scale = max(0.35, w / 270.0)

    # Main body
    d.rectangle([x0, y0, x1, y1], fill=CARGO_BODY, outline=CARGO_DARK)

    # Top and bottom interface flanges
    flange_h = max(4, int(h * 0.08))
    d.rectangle([x0, y0, x1, y0 + flange_h], fill=STEEL_MID)
    d.rectangle([x0, y1 - flange_h, x1, y1], fill=STEEL_MID)
    d.line([(x0, y0 + flange_h), (x1, y0 + flange_h)], fill=CARGO_DARK, width=1)
    d.line([(x0, y1 - flange_h), (x1, y1 - flange_h)], fill=CARGO_DARK, width=1)
    d.line([(x0 + 2, y0 + 1), (x1 - 2, y0 + 1)], fill=STEEL_HIGHLIGHT, width=1)

    # Bolts
    bolt_r = max(1, int(2 * scale))
    n_bolts = max(2, int(GW * 1.0))
    draw_bolts(d, cx, y0, w, flange_h, n_bolts, r=bolt_r)
    draw_bolts(d, cx, y1 - flange_h, w, flange_h, n_bolts, r=bolt_r)

    inner_top = y0 + flange_h
    inner_bot = y1 - flange_h
    inner_h = inner_bot - inner_top

    # Reinforcement ribs (horizontal corrugation)
    rib_spacing = max(8, int(PX * 0.25))
    rib_h = max(2, int(3 * scale))
    ry = inner_top + rib_spacing
    while ry + rib_h < inner_bot - rib_spacing // 2:
        # Rib highlight (raised ridge)
        d.rectangle([x0 + 2, ry, x1 - 2, ry + rib_h], fill=CARGO_LIGHT)
        # Shadow below rib
        d.line([(x0 + 2, ry + rib_h), (x1 - 2, ry + rib_h)], fill=CARGO_DARK, width=1)
        ry += rib_spacing

    # Left edge highlight
    d.line([(x0 + 2, inner_top + 2), (x0 + 2, inner_bot - 2)],
           fill=CARGO_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Vertical center seam
    d.line([(cx, inner_top + 2), (cx, inner_bot - 2)], fill=CARGO_DARK, width=1)

    # Cargo marking band (colored stripe for contents identification)
    mark_h = max(4, int(inner_h * 0.08))
    mark_y = inner_top + int(inner_h * 0.25) - mark_h // 2
    mark_inset = max(4, int(w * 0.08))
    d.rectangle([x0 + mark_inset, mark_y, x1 - mark_inset, mark_y + mark_h],
                fill=CARGO_MARKING)
    d.rectangle([x0 + mark_inset, mark_y, x1 - mark_inset, mark_y + mark_h],
                outline=CARGO_DARK)

    # Tie-down latch points (small rectangles on sides)
    latch_w = max(3, int(6 * scale))
    latch_h = max(4, int(8 * scale))
    n_latches = max(1, int(GH * 0.5))
    latch_spacing = inner_h / (n_latches + 1)
    for i in range(n_latches):
        ly = inner_top + int((i + 1) * latch_spacing) - latch_h // 2
        for side_x in [x0, x1 - latch_w]:
            d.rectangle([side_x, ly, side_x + latch_w, ly + latch_h],
                        fill=STEEL_LIGHT, outline=STEEL_DARK)
            # Latch handle line
            d.line([(side_x + 1, ly + latch_h // 2),
                    (side_x + latch_w - 1, ly + latch_h // 2)],
                   fill=STEEL_HIGHLIGHT, width=1)

    # Bottom cargo access hatch indicator (dashed line)
    hatch_y = inner_bot - int(inner_h * 0.2)
    hatch_inset = max(6, int(w * 0.15))
    dash_len = max(3, int(6 * scale))
    gap_len = max(2, dash_len // 2)
    x = x0 + hatch_inset
    while x < x1 - hatch_inset:
        end = min(x + dash_len, x1 - hatch_inset)
        d.line([(x, hatch_y), (end, hatch_y)], fill=CARGO_DARK, width=1)
        x = end + gap_len

    return img


# ================================================================
# Probe Core generator
# ================================================================

# Probe palette (darker than control modules — minimal, electronics-heavy)
PROBE_BODY = (35, 38, 45)
PROBE_DARK = (22, 24, 30)
PROBE_LIGHT = (52, 56, 65)

# HAL eye colors
HAL_FRAME = (100, 20, 20)
HAL_RED = (200, 30, 30)
HAL_BRIGHT = (240, 80, 50)
HAL_CENTER = (255, 180, 120)


def generate_probe(size):
    """Probe core: unmanned control unit with HAL-style red eye."""
    sizes = {
        "tiny":   (1, 1),
        "small":  (3, 1),
        "medium": (5, 1),
        "large":  (9, 1),
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)
    w, h = int(GW * PX), int(GH * PX)
    cw, ch = w + PAD * 2, h + PAD * 2
    img = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx = cw // 2
    x0, y0 = PAD, PAD
    x1, y1 = x0 + w, y0 + h
    scale = max(0.3, w / 270.0)

    # Main body (dark electronics)
    d.rectangle([x0, y0, x1, y1], fill=PROBE_BODY, outline=PROBE_DARK)

    # Top and bottom mount rings
    ring_h = max(3, int(h * 0.14))
    d.rectangle([x0, y0, x1, y0 + ring_h], fill=STEEL_MID)
    d.rectangle([x0, y1 - ring_h, x1, y1], fill=STEEL_MID)
    d.line([(x0, y0 + ring_h), (x1, y0 + ring_h)], fill=PROBE_DARK, width=1)
    d.line([(x0, y1 - ring_h), (x1, y1 - ring_h)], fill=PROBE_DARK, width=1)
    d.line([(x0 + 2, y0 + 1), (x1 - 2, y0 + 1)], fill=STEEL_HIGHLIGHT, width=1)
    d.line([(x0 + 2, y1 - 1), (x1 - 2, y1 - 1)], fill=STEEL_LIGHT, width=1)

    # Bolts on rings
    bolt_r = max(1, int(2 * scale))
    n_bolts = max(2, int(GW * 1.0))
    draw_bolts(d, cx, y0, w, ring_h, n_bolts, r=bolt_r)
    draw_bolts(d, cx, y1 - ring_h, w, ring_h, n_bolts, r=bolt_r)

    inner_top = y0 + ring_h
    inner_bot = y1 - ring_h
    inner_h = inner_bot - inner_top

    # Left edge highlight
    d.line([(x0 + 2, inner_top + 1), (x0 + 2, inner_bot - 1)],
           fill=PROBE_LIGHT, width=max(1, int(1.5 * scale)))

    # Subtle panel seams
    n_seams = max(0, int(GW * 0.3))
    seam_spacing = w / (n_seams + 1)
    for i in range(n_seams):
        sx = x0 + int((i + 1) * seam_spacing)
        d.line([(sx, inner_top + 1), (sx, inner_bot - 1)], fill=PROBE_DARK, width=1)

    # --- HAL 9000 red eye (central) ---
    eye_r = max(4, int(min(inner_h * 0.35, w * 0.08)))
    eye_cx = cx
    eye_cy = (inner_top + inner_bot) // 2

    # Dark frame ring
    circ(d, eye_cx, eye_cy, eye_r + max(2, int(3 * scale)), fill=HAL_FRAME)
    # Main red
    circ(d, eye_cx, eye_cy, eye_r, fill=HAL_RED)
    # Brighter inner ring
    circ(d, eye_cx, eye_cy, max(2, int(eye_r * 0.6)), fill=HAL_BRIGHT)
    # Hot center highlight
    circ(d, eye_cx, eye_cy, max(1, int(eye_r * 0.25)), fill=HAL_CENTER)
    # Specular glint (offset up-left)
    glint_r = max(1, int(eye_r * 0.15))
    circ(d, eye_cx - int(eye_r * 0.25), eye_cy - int(eye_r * 0.25),
         glint_r, fill=(255, 220, 200))
    # Outline
    circ(d, eye_cx, eye_cy, eye_r + max(2, int(3 * scale)), outline=PROBE_DARK)

    # Small status LEDs flanking the eye (for wider probes)
    if GW >= 3:
        led_r = max(1, int(2 * scale))
        led_offset = int(eye_r * 2.5 + 4 * scale)
        for sx in [-1, 1]:
            lx = eye_cx + sx * led_offset
            circ(d, lx, eye_cy, led_r, fill=CTRL_LED_GREEN)

    # Additional circuit trace lines (for medium+ probes)
    if GW >= 5:
        trace_color = (45, 50, 60)
        trace_y = eye_cy
        # Left traces
        tx0 = x0 + int(w * 0.06)
        tx1 = eye_cx - int(eye_r * 3)
        if tx1 > tx0:
            d.line([(tx0, trace_y), (tx1, trace_y)], fill=trace_color, width=1)
            d.line([(tx0, trace_y - 3), (tx1 - int(w * 0.03), trace_y - 3)],
                   fill=trace_color, width=1)
        # Right traces
        tx0 = eye_cx + int(eye_r * 3)
        tx1 = x1 - int(w * 0.06)
        if tx1 > tx0:
            d.line([(tx0, trace_y), (tx1, trace_y)], fill=trace_color, width=1)
            d.line([(tx0 + int(w * 0.03), trace_y - 3), (tx1, trace_y - 3)],
                   fill=trace_color, width=1)

    return img


# ================================================================
# Registry and main
# ================================================================

PARTS = {
    # Large Command Pod -> parts/
    "pod_large": ("Mk3 Command Pod (Large 9x5)",
                  generate_pod_large, "parts"),

    # Inline Control Centers -> parts/
    "control_small":  ("Sentinel Control Hub (Small 3x2)",
                       lambda: generate_control("small"), "parts"),
    "control_medium": ("Aegis Command Bridge (Medium 5x2)",
                       lambda: generate_control("medium"), "parts"),
    "control_large":  ("Bastion Operations Center (Large 9x3)",
                       lambda: generate_control("large"), "parts"),
    "control_xl":     ("Citadel CIC (XL 13x4)",
                       lambda: generate_control("xl"), "parts"),

    # Crew Quarters -> parts/
    "quarters_small":  ("Bunkhouse Module (Small 3x4)",
                        lambda: generate_quarters("small"), "parts"),
    "quarters_medium": ("Dormitory Module (Medium 5x5)",
                        lambda: generate_quarters("medium"), "parts"),
    "quarters_large":  ("Habitat Ring (Large 9x6)",
                        lambda: generate_quarters("large"), "parts"),

    # Greenhouses -> parts/
    "greenhouse_small":  ("Sprout Growth Chamber (Small 3x4)",
                          lambda: generate_greenhouse("small"), "parts"),
    "greenhouse_medium": ("Garden Biome (Medium 5x5)",
                          lambda: generate_greenhouse("medium"), "parts"),
    "greenhouse_large":  ("Arboretum (Large 9x6)",
                          lambda: generate_greenhouse("large"), "parts"),

    # Probe Cores -> parts/
    "probe_tiny":   ("Tiny Probe Core (1x1)",
                     lambda: generate_probe("tiny"), "parts"),
    "probe_small":  ("Small Probe Core (3x1)",
                     lambda: generate_probe("small"), "parts"),
    "probe_medium": ("Medium Probe Core (5x1)",
                     lambda: generate_probe("medium"), "parts"),
    "probe_large":  ("Large Probe Core (9x1)",
                     lambda: generate_probe("large"), "parts"),

    # Cargo Containers -> parts/
    "cargo_tiny":   ("Utility Crate (Tiny 1x2)",
                     lambda: generate_cargo("tiny"), "parts"),
    "cargo_small":  ("Storage Module (Small 3x3)",
                     lambda: generate_cargo("small"), "parts"),
    "cargo_medium": ("Cargo Bay (Medium 5x4)",
                     lambda: generate_cargo("medium"), "parts"),
    "cargo_large":  ("Freight Hold (Large 9x5)",
                     lambda: generate_cargo("large"), "parts"),
}


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    parts_dir = os.path.join(project_root, "data", "sprites", "parts")
    os.makedirs(parts_dir, exist_ok=True)

    for part_id, (name, gen_func, subdir) in PARTS.items():
        print(f"Generating {name}...")
        img = gen_func()
        path = os.path.join(parts_dir, f"{part_id}.png")
        img.save(path)
        print(f"  -> {path}  ({img.size[0]}x{img.size[1]})")

    print(f"\nDone! Generated {len(PARTS)} sprites.")
