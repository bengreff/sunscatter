#!/usr/bin/env python3
"""
Generate sprites for the specialty (fuel-locked) tank families:
  - AM-1 / AM-3 / AM-5 / AM-9     Penning-trap antimatter storage
  - PU-1 / PU-3 / PU-5 / PU-9     Orion-style nuclear pulse magazines

Both lineups share the xenon-tank size lineup (Tiny 1x1, Small 3x1,
Medium 5x1, Large 9x1) but have distinctive visual styles:

  AM tanks    — deep violet body, evenly-spaced solenoid coil bands,
                bright cryogenic glow point at center (suspended antimatter),
                cryocooler housings on end caps.
  Pulse mags  — olive/khaki industrial body, yellow-black radiation hazard
                stripes, visible pulse-unit cells along feed track,
                heavy bolted hatches.

Output: data/sprites/parts/tank_am_{tiny,small,medium,large}.png
        data/sprites/parts/tank_pulse_{tiny,small,medium,large}.png
"""

import math
import os

import numpy as np
from PIL import Image, ImageDraw

PX = 360
MAX_SPRITE_PX = 4096
PAD = 0

SIZES = {
    "tiny":   (1, 1),
    "small":  (3, 1),
    "medium": (5, 1),
    "large":  (9, 1),
}


def _capped_px(w, h):
    return min(PX, MAX_SPRITE_PX // max(w, h))


def circ(d, cx, cy, r, **kw):
    d.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)


# ================================================================
# Antimatter tank — Penning trap palette
# ================================================================

AM_VOID = (18, 12, 30)         # deepest cavity color
AM_DARK = (38, 22, 60)         # outer shell
AM_MID = (62, 36, 92)          # body
AM_LIGHT = (96, 60, 140)       # rim highlights
AM_HIGHLIGHT = (160, 110, 210) # specular
AM_GLOW = (220, 170, 255)      # antimatter containment glow
AM_GLOW_HOT = (255, 220, 255)  # core bright dot
COIL_DARK = (55, 60, 80)       # superconducting coil casing
COIL_MID = (95, 100, 125)
COIL_LIGHT = (140, 145, 175)


def generate_am_tank(size):
    """Cylindrical Penning-trap array.

    Heavy violet pressure vessel banded by superconducting coil rings;
    bright magnetic-bottle glow visible through observation windows
    between coils.
    """
    GW, GH = SIZES[size]
    PXc = _capped_px(GW, GH)

    img_w = int(GW * PXc) + PAD * 2
    img_h = int(GH * PXc) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    w, h = int(GW * PXc), int(GH * PXc)
    x1, y1 = x0 + w, y0 + h
    cy = img_h // 2
    scale = max(0.35, w / 270.0)

    # Main body — dark violet
    d.rectangle([x0, y0, x1, y1], fill=AM_MID)

    # Reinforced top/bottom rims (cryocooler heat-sink fins)
    rim_h = max(3, h // 5)
    d.rectangle([x0, y0, x1, y0 + rim_h], fill=AM_DARK)
    d.rectangle([x0, y1 - rim_h, x1, y1], fill=AM_DARK)
    # Fine-pitch radiator fin lines on rims
    fin_pitch = max(2, int(3 * scale))
    for fy in range(y0 + 1, y0 + rim_h, fin_pitch):
        d.line([(x0 + 1, fy), (x1 - 1, fy)], fill=AM_VOID, width=1)
    for fy in range(y1 - rim_h + 1, y1, fin_pitch):
        d.line([(x0 + 1, fy), (x1 - 1, fy)], fill=AM_VOID, width=1)
    # Top/bottom rim highlight
    d.line([(x0, y0 + rim_h), (x1, y0 + rim_h)], fill=AM_LIGHT, width=1)
    d.line([(x0, y1 - rim_h), (x1, y1 - rim_h)], fill=AM_LIGHT, width=1)

    # Solenoid coil bands across the centerline strip — these are vertical
    # rectangles that span the visible body between the two rims, evenly
    # spaced along the tank length. Between coils, the glow shows through.
    coil_w_px = max(4, int(8 * scale))
    coil_pitch = max(coil_w_px * 2 + 2, int(12 * scale))
    inset = max(2, int(4 * scale))
    body_top = y0 + rim_h + 1
    body_bot = y1 - rim_h - 1

    # First lay down a horizontal slab of "glow void" between coils — start
    # with a dark interior, then add glow points where the magnetic bottles sit
    d.rectangle([x0 + inset, body_top, x1 - inset, body_bot], fill=AM_VOID)

    # Coil bands
    n_coils = max(2, (w - inset * 2) // coil_pitch)
    if n_coils <= 1:
        # Tiny tank: a single thick coil centered
        cx_coil = (x0 + x1) // 2
        cl, cr = cx_coil - coil_w_px // 2, cx_coil + coil_w_px // 2
        d.rectangle([cl, body_top, cr, body_bot], fill=COIL_MID, outline=COIL_DARK)
        d.line([(cl + 1, body_top + 1), (cl + 1, body_bot - 1)],
               fill=COIL_LIGHT, width=1)
        coil_xs = [cx_coil]
    else:
        # Distribute n_coils evenly between (x0+inset) and (x1-inset)
        span_left = x0 + inset + coil_w_px // 2
        span_right = x1 - inset - coil_w_px // 2
        coil_xs = []
        for i in range(n_coils):
            t = i / (n_coils - 1) if n_coils > 1 else 0.5
            cxc = int(span_left + (span_right - span_left) * t)
            cl, cr = cxc - coil_w_px // 2, cxc + coil_w_px // 2
            d.rectangle([cl, body_top, cr, body_bot],
                        fill=COIL_MID, outline=COIL_DARK)
            d.line([(cl + 1, body_top + 1), (cl + 1, body_bot - 1)],
                   fill=COIL_LIGHT, width=1)
            coil_xs.append(cxc)

    # Glow dots — one between each pair of adjacent coils, with a hot core
    glow_r = max(2, int(min(coil_pitch, body_bot - body_top) * 0.30))
    if len(coil_xs) >= 2:
        for a, b in zip(coil_xs[:-1], coil_xs[1:]):
            gx = (a + b) // 2
            circ(d, gx, cy, glow_r + 1, fill=AM_GLOW)
            circ(d, gx, cy, max(1, glow_r // 2), fill=AM_GLOW_HOT)
    else:
        # No gap — show glow above/below the single coil's vertical extent
        cx_glow = coil_xs[0]
        gy_top = body_top + (body_bot - body_top) // 3
        gy_bot = body_bot - (body_bot - body_top) // 3
        for gy in (gy_top, gy_bot):
            circ(d, cx_glow, gy, glow_r, fill=AM_GLOW)
            circ(d, cx_glow, gy, max(1, glow_r // 2), fill=AM_GLOW_HOT)

    # Bolts along top and bottom rims
    bolt_r = max(1, min(2, h // 10))
    n_bolts = max(2, int(GW * 1.2))
    for i in range(n_bolts):
        bx = x0 + bolt_r * 2 + i * int(max(1, (w - bolt_r * 4) /
                                            max(1, n_bolts - 1)))
        circ(d, bx, y0 + rim_h // 2, bolt_r, fill=AM_HIGHLIGHT)
        circ(d, bx, y0 + rim_h // 2, max(1, bolt_r - 1), fill=AM_VOID)
        circ(d, bx, y1 - rim_h // 2, bolt_r, fill=AM_HIGHLIGHT)
        circ(d, bx, y1 - rim_h // 2, max(1, bolt_r - 1), fill=AM_VOID)

    # Outer outline
    d.rectangle([x0, y0, x1, y1], outline=AM_VOID)

    return img


# ================================================================
# Pulse magazine — Orion fission ordnance palette
# ================================================================

PULSE_VOID = (18, 18, 14)
PULSE_DARK = (45, 50, 38)        # dark olive steel
PULSE_MID = (78, 82, 60)         # body
PULSE_LIGHT = (115, 120, 90)
PULSE_HIGHLIGHT = (160, 165, 130)
PULSE_HAZARD = (220, 195, 40)    # radiation yellow
PULSE_HAZARD_DARK = (180, 155, 25)
PULSE_CELL = (60, 55, 45)        # interior of pulse-unit cell
PULSE_CELL_NOSE = (200, 180, 110) # exposed casing of stored unit


def generate_pulse_tank(size):
    """Heavy radiation-shielded magazine.

    Olive-steel body with yellow-black radiation hazard stripes,
    a visible bottom feed slot, and pulse-unit cells peeking through
    inspection ports along the side.
    """
    GW, GH = SIZES[size]
    PXc = _capped_px(GW, GH)

    img_w = int(GW * PXc) + PAD * 2
    img_h = int(GH * PXc) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    w, h = int(GW * PXc), int(GH * PXc)
    x1, y1 = x0 + w, y0 + h
    cx = img_w // 2
    cy = img_h // 2
    scale = max(0.35, w / 270.0)

    # Body
    d.rectangle([x0, y0, x1, y1], fill=PULSE_MID)

    # Top rim (handling cranes / suspension lugs)
    rim_h = max(3, h // 5)
    d.rectangle([x0, y0, x1, y0 + rim_h], fill=PULSE_DARK)
    d.line([(x0, y0 + rim_h), (x1, y0 + rim_h)], fill=PULSE_VOID, width=1)
    # Suspension lug bolts on top rim
    bolt_r = max(1, min(2, h // 10))
    n_bolts = max(2, int(GW * 1.2))
    for i in range(n_bolts):
        bx = x0 + bolt_r * 2 + i * int(max(1, (w - bolt_r * 4) /
                                            max(1, n_bolts - 1)))
        circ(d, bx, y0 + rim_h // 2, bolt_r, fill=PULSE_HIGHLIGHT)
        circ(d, bx, y0 + rim_h // 2, max(1, bolt_r - 1), fill=PULSE_VOID)

    # Bottom rim — heavy explosive-ejection hatch frame
    bottom_rim = max(4, int(h * 0.28))
    d.rectangle([x0, y1 - bottom_rim, x1, y1], fill=PULSE_DARK)
    d.line([(x0, y1 - bottom_rim), (x1, y1 - bottom_rim)],
           fill=PULSE_VOID, width=1)

    # Feed slot — the pulse units drop through here
    slot_h = max(2, int(bottom_rim * 0.40))
    slot_inset = max(3, int(w * 0.06))
    slot_y = y1 - bottom_rim + (bottom_rim - slot_h) // 2
    d.rectangle([x0 + slot_inset, slot_y,
                 x1 - slot_inset, slot_y + slot_h],
                fill=PULSE_VOID)
    # A single pulse unit visible in the slot (nose protruding) — only if there's room
    unit_w = max(4, int(min(slot_h * 1.4, (x1 - slot_inset - x0 - slot_inset) * 0.25)))
    if unit_w * 2 < (x1 - x0 - slot_inset * 2):
        ux1 = cx - unit_w // 2
        ux2 = cx + unit_w // 2
        d.rectangle([ux1, slot_y + 1, ux2, slot_y + slot_h - 1],
                    fill=PULSE_CELL_NOSE)
        # Nose tip ring
        d.line([(ux1, slot_y + slot_h // 2), (ux2, slot_y + slot_h // 2)],
               fill=PULSE_HAZARD_DARK, width=1)

    # Side body — radiation hazard stripe band running along the middle
    band_y = cy
    band_h = max(3, int(h * 0.18))
    stripe_w = max(4, int(8 * scale))
    body_x0 = x0 + 2
    body_x1 = x1 - 2
    # Background of band
    d.rectangle([body_x0, band_y - band_h // 2,
                 body_x1, band_y + band_h // 2],
                fill=PULSE_VOID)
    # Diagonal yellow-black hazard stripes
    x_iter = body_x0
    color_toggle = True
    while x_iter < body_x1:
        next_x = min(x_iter + stripe_w, body_x1)
        color = PULSE_HAZARD if color_toggle else PULSE_VOID
        d.polygon([
            (x_iter, band_y - band_h // 2),
            (next_x, band_y - band_h // 2),
            (next_x + band_h // 2, band_y + band_h // 2),
            (x_iter + band_h // 2, band_y + band_h // 2),
        ], fill=color)
        x_iter = next_x
        color_toggle = not color_toggle
    d.line([(body_x0, band_y - band_h // 2),
            (body_x1, band_y - band_h // 2)], fill=PULSE_HAZARD_DARK, width=1)
    d.line([(body_x0, band_y + band_h // 2),
            (body_x1, band_y + band_h // 2)], fill=PULSE_HAZARD_DARK, width=1)

    # Inspection-port pulse-unit cells along the top of the body (between
    # the top rim and the hazard band). Each port shows the round nose of a
    # stored pulse unit.
    port_band_top = y0 + rim_h + max(1, int(2 * scale))
    port_band_bot = band_y - band_h // 2 - max(1, int(2 * scale))
    if port_band_bot - port_band_top >= 4:
        port_y = (port_band_top + port_band_bot) // 2
        port_r = max(1, min((port_band_bot - port_band_top) // 2, int(4 * scale)))
        n_ports = max(1, int(GW))
        if n_ports == 1:
            port_xs = [cx]
        else:
            port_left = x0 + int(w * 0.10)
            port_right = x1 - int(w * 0.10)
            port_xs = [int(port_left + (port_right - port_left) *
                          (i / (n_ports - 1))) for i in range(n_ports)]
        for px in port_xs:
            circ(d, px, port_y, port_r + 1, fill=PULSE_DARK)
            circ(d, px, port_y, port_r, fill=PULSE_CELL)
            # Nose visible in the cell (only if port is big enough)
            if port_r >= 2:
                circ(d, px, port_y, max(1, port_r - 2), fill=PULSE_CELL_NOSE)

    # Body outline
    d.rectangle([x0, y0, x1, y1], outline=PULSE_VOID)

    return img


# ================================================================
# AM Sphere — endgame bulk-containment tank
# ================================================================
#
# Based directly on the fusion-sphere sprite (tools/sprite_gen/
# generate_fusion_tanks.py) but with a deep violet palette and
# additional reinforcement detail:
#   - twin structural trusses (left and right) instead of one
#   - three reinforced equatorial rings instead of a single weld seam
#   - extra bolts along seams
#   - antimatter-warning fill port at top
#
# Sizes mirror the fusion spheres: 20x20, 40x40, 60x60 grid squares.

# Numpy float arrays for per-pixel sphere fill
AM_SPHERE_BASE = np.array([62, 36, 92], dtype=np.float32)
AM_SPHERE_EDGE = np.array([28, 16, 50], dtype=np.float32)

# PIL tuples for detail drawing — on a dark violet body, raised reinforcement
# panel seams read as LIGHTER lines (highlights), and the deepest recessed weld
# joints read as the darkest violet possible.
AM_SEAM_COLOR = (110, 78, 165)        # raised panel seams (highlight)
AM_WELD_COLOR = (24, 12, 44)          # recessed equatorial weld
AM_RING_COLOR = (175, 130, 230)       # bright reinforcement ring highlight

# Truss / reinforcement
AM_TRUSS_DARK = (50, 30, 78, 255)
AM_TRUSS_MID = (78, 50, 118, 255)
AM_TRUSS_LIGHT = (115, 85, 165, 255)

# Fill port (with AM warning marker)
AM_PORT_DARK = (40, 20, 60, 255)
AM_PORT_MID = (75, 50, 115, 255)
AM_PORT_LIGHT = (115, 85, 165, 255)
AM_PORT_WARNING = (245, 215, 60, 255)  # AM warning iris

AM_SPHERES = {
    "tank_am_sphere_s": {"grid": 20, "name": "AM Sphere S (20x20)"},
    "tank_am_sphere_m": {"grid": 40, "name": "AM Sphere M (40x40)"},
    "tank_am_sphere_l": {"grid": 60, "name": "AM Sphere L (60x60)"},
}


def generate_am_sphere(grid_size):
    """Endgame bulk-antimatter spherical tank — fusion-sphere-derived
    silhouette with violet palette and extra reinforcement."""
    # Spheres need high detail because the player zooms in close. Cap at 4096
    # (the per-sprite GPU/cap_sprite_size limit). All six AM/fusion spheres
    # are exempt from the atlas halve loop (see HIGH_RES_SPRITES in
    # src/render/sprites.rs) so they remain at full source resolution in the
    # atlas while engines/other parts halve normally.
    sphere_max_px = 4096
    effective_px = min(PX, sphere_max_px // grid_size)
    size = grid_size * effective_px
    radius = size / 2.0
    cx_f = cy_f = size / 2.0
    cx = cy = size // 2
    r = int(radius)

    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    edge_t = max(0.002, 2.0 / radius)

    # ---- Sphere fill (numpy, strip-processed for memory) ----
    chunk = 256
    total_chunks = (size + chunk - 1) // chunk
    report_interval = max(1, total_chunks // 5)
    for ci, y_start in enumerate(range(0, size, chunk)):
        if ci % report_interval == 0:
            print(f"    filling... {int(100 * y_start / size)}%")
        y_end = min(y_start + chunk, size)
        rows = y_end - y_start

        yy = np.arange(y_start, y_end, dtype=np.float32)
        xx = np.arange(0, size, dtype=np.float32)
        Y, X = np.meshgrid(yy, xx, indexing='ij')
        nx = (X - cx_f) / radius
        ny = (Y - cy_f) / radius
        dist = np.sqrt(nx * nx + ny * ny)

        limb = np.clip(dist * 1.2, 0.0, 1.0)[..., np.newaxis]
        color = AM_SPHERE_BASE + (AM_SPHERE_EDGE - AM_SPHERE_BASE) * (limb ** 2)
        color = np.clip(color, 0, 255).astype(np.uint8)
        alpha_f = np.clip((1.0 - dist) / edge_t, 0.0, 1.0)
        alpha = (alpha_f * 255).astype(np.uint8)

        strip = np.zeros((rows, size, 4), dtype=np.uint8)
        strip[:, :, 0] = color[:, :, 0]
        strip[:, :, 1] = color[:, :, 1]
        strip[:, :, 2] = color[:, :, 2]
        strip[:, :, 3] = alpha
        img.paste(Image.fromarray(strip), (0, y_start))
    print("    filling... 100%")

    # ---- Surface detail (clipped to sphere via alpha mask later) ----
    alpha_mask = img.split()[3]
    d = ImageDraw.Draw(img)

    scale = grid_size / 20.0
    line_w = max(2, int(4 * scale))
    weld_w = max(3, int(7 * scale))
    ring_w = max(3, int(6 * scale))

    # Latitude panel seams
    for lat_deg in [-55, -28, 28, 55]:
        lat = math.radians(lat_deg)
        y_pos = cy - int(r * math.sin(lat))
        half_w = int(r * math.cos(lat) * 0.93)
        d.line([(cx - half_w, y_pos), (cx + half_w, y_pos)],
               fill=AM_SEAM_COLOR, width=line_w)

    # Meridian seams (great-circle arcs, front-face only)
    for lon_deg in [0, 35, -35, 65, -65]:
        lon = math.radians(lon_deg)
        points = []
        for t in range(-85, 86):
            theta = math.radians(t)
            x_pt = cx + int(r * math.sin(lon) * math.cos(theta))
            y_pt = cy - int(r * math.sin(theta))
            z_check = math.cos(lon) * math.cos(theta)
            if z_check > 0.1:
                points.append((x_pt, y_pt))
            else:
                if len(points) > 1:
                    d.line(points, fill=AM_SEAM_COLOR, width=line_w)
                points = []
        if len(points) > 1:
            d.line(points, fill=AM_SEAM_COLOR, width=line_w)

    # Triple equatorial reinforcement rings instead of a single weld seam
    eq_offsets = [-int(r * 0.05), 0, int(r * 0.05)]
    for off in eq_offsets:
        d.line([(cx - int(r * 0.93), cy + off), (cx + int(r * 0.93), cy + off)],
               fill=AM_WELD_COLOR, width=weld_w)
    # Highlighted center band on the middle ring
    d.line([(cx - int(r * 0.92), cy), (cx + int(r * 0.92), cy)],
           fill=AM_RING_COLOR, width=ring_w)
    # Bolts along the equatorial bands
    bolt_r = max(2, int(3 * scale))
    n_bolts = max(6, int(grid_size * 0.6))
    for i in range(n_bolts):
        bx = cx - int(r * 0.85) + i * int(r * 1.70 / max(1, n_bolts - 1))
        for off in (eq_offsets[0], eq_offsets[2]):
            d.ellipse([bx - bolt_r, cy + off - bolt_r,
                       bx + bolt_r, cy + off + bolt_r],
                      fill=AM_TRUSS_LIGHT, outline=AM_WELD_COLOR)

    # Fill port at top with AM warning iris
    port_r = max(8, int(18 * scale))
    port_y = cy - r + int(r * 0.07)
    flange = max(4, int(6 * scale))
    d.ellipse([cx - port_r - flange, port_y - port_r - flange,
               cx + port_r + flange, port_y + port_r + flange],
              fill=AM_PORT_DARK)
    d.ellipse([cx - port_r, port_y - port_r,
               cx + port_r, port_y + port_r],
              fill=AM_PORT_MID)
    inner_r = max(4, port_r - int(4 * scale))
    d.ellipse([cx - inner_r, port_y - inner_r,
               cx + inner_r, port_y + inner_r],
              fill=AM_PORT_LIGHT)
    # Warning iris (yellow ring around the port)
    iris_r = max(2, port_r // 2)
    d.ellipse([cx - iris_r, port_y - iris_r,
               cx + iris_r, port_y + iris_r],
              fill=AM_PORT_WARNING)
    # Central dark dot (the actual port aperture)
    dot_r = max(2, port_r // 4)
    d.ellipse([cx - dot_r, port_y - dot_r,
               cx + dot_r, port_y + dot_r],
              fill=AM_PORT_DARK)

    # Restore sphere alpha (clips all surface details to sphere boundary)
    img.putalpha(alpha_mask)

    # ---- Twin structural trusses (left + right) ----
    # Mirrored about the vertical centerline; each is the same size as the
    # fusion sphere's single truss, scaled by sphere size.
    d = ImageDraw.Draw(img)

    truss_grid_w = 2.5
    truss_px_w = int(truss_grid_w * effective_px)
    truss_height = int(r * 0.30)
    member_w = max(4, int(8 * scale))

    for side in (-1, 1):
        if side == 1:
            t_x1 = cx + r - int(r * 0.03)
            t_x2 = t_x1 + truss_px_w
        else:
            t_x2 = cx - r + int(r * 0.03)
            t_x1 = t_x2 - truss_px_w
        if side == -1:
            t_x1, t_x2 = min(t_x1, t_x2), max(t_x1, t_x2)
        t_y1 = cy - truss_height // 2
        t_y2 = cy + truss_height // 2

        # Frame interior fill
        d.rectangle([t_x1 + member_w, t_y1 + member_w,
                     t_x2 - member_w, t_y2 - member_w], fill=AM_TRUSS_MID)
        # Outer frame
        d.rectangle([t_x1, t_y1, t_x2, t_y2],
                    fill=None, outline=AM_TRUSS_DARK, width=member_w)
        # Cross-bracing
        d.line([(t_x1, t_y1), (t_x2, t_y2)], fill=AM_TRUSS_DARK, width=member_w)
        d.line([(t_x1, t_y2), (t_x2, t_y1)], fill=AM_TRUSS_DARK, width=member_w)
        # Equatorial horizontal member
        d.line([(t_x1, cy), (t_x2, cy)], fill=AM_TRUSS_DARK, width=member_w)
        # Vertical center strut
        mid_x = (t_x1 + t_x2) // 2
        d.line([(mid_x, t_y1), (mid_x, t_y2)],
               fill=AM_TRUSS_DARK, width=member_w)
        # Top-edge highlight
        d.rectangle([t_x1 + member_w, t_y1 + member_w,
                     t_x2 - member_w, t_y1 + member_w + max(2, int(3 * scale))],
                    fill=AM_TRUSS_LIGHT)
        # Outer-edge reinforcement
        reinforce_w = max(3, int(6 * scale))
        if side == 1:
            d.rectangle([t_x2 - reinforce_w, t_y1, t_x2, t_y2],
                        fill=AM_TRUSS_DARK)
            d.rectangle([t_x2 - reinforce_w + 1, t_y1 + 2,
                         t_x2 - 1, t_y1 + max(3, int(5 * scale))],
                        fill=AM_TRUSS_LIGHT)
        else:
            d.rectangle([t_x1, t_y1, t_x1 + reinforce_w, t_y2],
                        fill=AM_TRUSS_DARK)
            d.rectangle([t_x1 + 1, t_y1 + 2,
                         t_x1 + reinforce_w - 1, t_y1 + max(3, int(5 * scale))],
                        fill=AM_TRUSS_LIGHT)

    return img


# ================================================================
# Main
# ================================================================

if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    parts_dir = os.path.join(project_root, "data", "sprites", "parts")
    os.makedirs(parts_dir, exist_ok=True)

    PARTS = []
    for sz in ("tiny", "small", "medium", "large"):
        PARTS.append((f"tank_am_{sz}", lambda s=sz: generate_am_tank(s)))
        PARTS.append((f"tank_pulse_{sz}", lambda s=sz: generate_pulse_tank(s)))
    for part_id, info in AM_SPHERES.items():
        PARTS.append((part_id, lambda g=info["grid"], n=info["name"]:
                      (print(f"Generating {n}...") or generate_am_sphere(g))))

    for name, fn in PARTS:
        img = fn()
        out = os.path.join(parts_dir, f"{name}.png")
        img.save(out)
        print(f"  {name:22s}  {img.size[0]:4d}x{img.size[1]:4d}  -> {out}")

    print(f"\nGenerated {len(PARTS)} specialty-tank sprites in {parts_dir}")
