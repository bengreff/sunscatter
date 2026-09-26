#!/usr/bin/env python3
"""
Generate sprites for 8 interstellar engines.

Three technology tiers:
  Fission:    Orion Pulse (nuclear pulse propulsion)
  Fusion:     Daedalus S1/S2, Z-Pinch Probe/Advanced
  Antimatter: AM-Cat Fusion, Antimatter Torch, Gamma Conversion

Each engine has a distinctive visual design reflecting its propulsion physics.
Interstellar engines are dramatically larger than chemical engines (9-27 grid
wide vs 1-8 for chemical), with accent colors for magnetic coils (copper),
reactor housings (blue-steel), and antimatter containment (purple).

Dimensions are based on reaction area requirements, not mass:
  - Daedalus: Wide (40m-class dome), moderate length
  - Z-Pinch: Narrow (tube confinement), tall
  - AM-Cat: Moderate compact (ICAN-II style)
  - AM Torch: Very compact (magnetic bottle)
  - Gamma Conv: Wide base (parabolic reflector)
  - Orion: Very wide (massive pusher plate)
"""

from PIL import Image, ImageDraw
import math
import os
import random

# ================================================================
# Constants
# ================================================================
PX = 360   # pixels per grid square (4x base resolution)
MAX_SPRITE_PX = 4096  # cap output image size to avoid multi-GB RGBA decode
PAD = 0  # No padding — atlas packer adds spacing

def _capped_px(w, h):
    """Return effective PX capped so the output image stays within MAX_SPRITE_PX."""
    return min(PX, MAX_SPRITE_PX // max(w, h))

# ================================================================
# Palette — steel base + interstellar accents
# ================================================================

# Base steel (same as chemical engines)
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)
ABLATIVE_TIP = (70, 62, 50)
PIPE_COLOR = (65, 68, 75)
PIPE_HIGHLIGHT = (100, 105, 115)

# Magnetic coil windings (copper/bronze)
COIL_DARK = (90, 60, 35)
COIL_MID = (140, 95, 55)
COIL_LIGHT = (180, 130, 75)
COIL_HIGHLIGHT = (210, 165, 100)

# Reactor housing (blue-tinted steel)
REACTOR_DARK = (40, 50, 65)
REACTOR_MID = (55, 70, 92)
REACTOR_LIGHT = (80, 100, 130)
REACTOR_HIGHLIGHT = (105, 130, 160)

# Antimatter containment (deep purple)
AM_DARK = (55, 38, 75)
AM_MID = (80, 58, 110)
AM_LIGHT = (110, 85, 145)
AM_HIGHLIGHT = (140, 115, 175)

# Energy channel (cyan/teal)
ENERGY_DARK = (35, 65, 85)
ENERGY_MID = (50, 90, 120)
ENERGY_LIGHT = (70, 120, 155)

# Orion pusher plate (heat-darkened steel)
PLATE_DARK = (42, 40, 36)
PLATE_MID = (58, 55, 48)
PLATE_LIGHT = (78, 72, 62)
PLATE_HIGHLIGHT = (95, 88, 75)
PLATE_ABLATION = (48, 42, 34)

# Gamma reflector (warm silver)
GAMMA_DARK = (60, 58, 65)
GAMMA_MID = (90, 86, 95)
GAMMA_LIGHT = (125, 120, 132)
GAMMA_HIGHLIGHT = (160, 155, 168)


# ================================================================
# Drawing primitives
# ================================================================

def trap(d, cx, y, tw, bw, h, fill, outline=None):
    """Trapezoid: tw=top width, bw=bottom width, h=height."""
    d.polygon([(cx - tw / 2, y), (cx + tw / 2, y),
               (cx + bw / 2, y + h), (cx - bw / 2, y + h)],
              fill=fill, outline=outline)

def rect(d, cx, y, w, h, fill, outline=None):
    """Centered rectangle."""
    d.rectangle([cx - w / 2, y, cx + w / 2, y + h], fill=fill, outline=outline)

def circ(d, cx, cy, r, **kw):
    d.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)

def bell_w(frac, throat, exit_w, exp=1.8):
    """Bell/nozzle expansion curve."""
    return throat + (exit_w - throat) * (1 - (1 - frac) ** exp)

def bezier2(p0, p1, p2, n=20):
    pts = []
    for i in range(n + 1):
        t = i / n
        x = (1-t)**2*p0[0] + 2*(1-t)*t*p1[0] + t**2*p2[0]
        y = (1-t)**2*p0[1] + 2*(1-t)*t*p1[1] + t**2*p2[1]
        pts.append((int(x), int(y)))
    return pts

def lerp_color(c1, c2, t):
    """Linearly interpolate between two RGB colors."""
    t = max(0.0, min(1.0, t))
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3))

def clamp_color(c):
    return tuple(max(0, min(255, v)) for v in c)

def nearest_odd(x):
    """Round a float to the nearest odd integer."""
    n = round(x)
    if n % 2 == 0:
        return n + 1 if x >= n else n - 1
    return n


def draw_bolts(d, cx, y, w, h, n, r=3):
    """Draw a row of bolts across a horizontal span."""
    hw = int(w / 2)
    for i in range(n):
        if n > 1:
            bx = cx - hw + r * 2 + i * int((w - r * 4) / (n - 1))
        else:
            bx = cx
        circ(d, bx, y + h // 2, r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y + h // 2, max(1, r - 1), fill=STEEL_DARK)


# ================================================================
# Shared interstellar components
# ================================================================

def draw_mount_ring(d, cx, y, w, h, scale=1.0):
    """Standard mounting interface ring with bolts."""
    bolt_r = max(2, int(3 * scale))
    n_bolts = max(3, int(w / (bolt_r * 8)))
    trap(d, cx, y, w, w, h, STEEL_MID, outline=STEEL_DARK)
    hw = int(w / 2)
    d.line([(cx - hw + 2, y + 1), (cx + hw - 2, y + 1)],
           fill=STEEL_HIGHLIGHT, width=max(1, int(scale)))
    draw_bolts(d, cx, y, w, h, n_bolts, r=bolt_r)
    return y + h


def draw_magnetic_coil(d, cx, y, outer_w, thickness, scale=1.0, wall_th=None):
    """Draw a toroidal magnetic field coil wrapping around a hollow nozzle.

    Renders as two raised copper bumps on the left and right nozzle walls,
    NOT spanning across the interior void. Each bump has 3D rounded
    cross-section shading with specular highlight and cast shadow.
    """
    th = max(4, int(thickness))
    protrude = max(2, int(3 * scale))

    # Wall thickness determines how wide each coil bump is
    if wall_th is None:
        wall_th = max(12, int(14 * scale))

    o_hw = int(outer_w / 2) + protrude          # outer edge (beyond wall)
    i_hw = max(0, int(outer_w / 2) - wall_th)   # inner edge (at void)

    for side in [-1, 1]:
        if side == -1:
            left = cx - o_hw
            right = cx - i_hw
        else:
            left = cx + i_hw
            right = cx + o_hw

        section_w = right - left
        if section_w < 3:
            continue

        # Rounded cross-section shading (sinusoidal brightness)
        for row in range(th):
            t = row / max(1, th - 1)
            roundness = math.sin(t * math.pi)
            top_bias = max(0, 1.0 - t * 1.6)
            shade = 0.15 + 0.55 * roundness + 0.30 * top_bias
            shade = max(0.0, min(1.0, shade))

            r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
            g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
            b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)

            ry = y - th // 2 + row
            d.rectangle([left, ry, right, ry + 1],
                        fill=clamp_color((r, g, b)))

        # Specular highlight near top
        spec_y = y - th // 2 + max(1, th // 5)
        d.line([(left + 1, spec_y), (right - 1, spec_y)],
               fill=COIL_HIGHLIGHT, width=1)

        # Cast shadow below
        shadow_y = y + (th + 1) // 2
        d.rectangle([left + 1, shadow_y, right - 1,
                     shadow_y + max(1, int(1 * scale))],
                    fill=(30, 25, 18))

        # Winding marks (only if section is wide enough)
        if section_w > 12:
            wind_color = (min(255, COIL_LIGHT[0] + 15),
                          min(255, COIL_LIGHT[1] + 15),
                          min(255, COIL_LIGHT[2] + 10))
            n_winds = max(3, section_w // max(3, int(4 * scale)))
            spacing = max(2, (section_w - 4) // max(1, n_winds))
            for i in range(n_winds):
                wx = left + 2 + i * spacing
                if wx < right - 2:
                    d.line([(wx, y - th // 2 + 2),
                            (wx + max(1, th // 6), y + th // 2 - 2)],
                           fill=wind_color, width=1)


def draw_reactor_housing(d, cx, y, top_w, bot_w, h, tint="reactor", scale=1.0):
    """Draw a reactor section housing with panel lines and structural bands."""
    colors = {
        "reactor": (REACTOR_DARK, REACTOR_MID, REACTOR_LIGHT, REACTOR_HIGHLIGHT),
        "antimatter": (AM_DARK, AM_MID, AM_LIGHT, AM_HIGHLIGHT),
        "steel": (STEEL_DARK, STEEL_MID, STEEL_LIGHT, STEEL_HIGHLIGHT),
        "gamma": (GAMMA_DARK, GAMMA_MID, GAMMA_LIGHT, GAMMA_HIGHLIGHT),
    }
    dark, mid, light, highlight = colors.get(tint, colors["reactor"])

    # Main housing body
    trap(d, cx, y, top_w, bot_w, h, mid, outline=dark)

    # Left edge highlight
    d.line([(cx - int(top_w / 2) + 2, y + 2),
            (cx - int(bot_w / 2) + 2, y + h - 2)],
           fill=light, width=max(1, int(scale)))

    # Horizontal structural bands
    band_h = max(2, int(3 * scale))
    for frac in [0.25, 0.5, 0.75]:
        by = y + int(h * frac)
        bw_at = top_w + (bot_w - top_w) * frac
        bhw = int(bw_at / 2)
        d.rectangle([cx - bhw, by, cx + bhw, by + band_h], fill=light)
        d.rectangle([cx - bhw, by + band_h, cx + bhw, by + band_h + 1], fill=dark)

    # Vertical panel lines
    n_panels = max(2, int(bot_w / (40 * scale)))
    for i in range(n_panels):
        frac = (i + 1) / (n_panels + 1)
        px = cx - int(bot_w / 2) + int(bot_w * frac)
        d.line([(px, y + 4), (px, y + h - 4)], fill=dark, width=1)

    return y + h


def draw_containment_ring(d, cx, y, w, h, tint="antimatter", scale=1.0):
    """Draw a containment/separator ring with accent color."""
    colors = {
        "antimatter": (AM_DARK, AM_MID, AM_LIGHT, AM_HIGHLIGHT),
        "energy": (ENERGY_DARK, ENERGY_MID, ENERGY_LIGHT),
        "copper": (COIL_DARK, COIL_MID, COIL_LIGHT),
        "steel": (STEEL_DARK, STEEL_MID, STEEL_LIGHT),
    }
    c = colors.get(tint, colors["steel"])
    hw = int(w / 2)
    extra = max(4, int(6 * scale))
    # Ring wider than surrounding structure
    d.rectangle([cx - hw - extra, y, cx + hw + extra, y + h], fill=c[1])
    d.rectangle([cx - hw - extra, y, cx + hw + extra, y + max(1, h // 3)], fill=c[2])
    d.rectangle([cx - hw - extra, y + h - max(1, h // 3), cx + hw + extra, y + h],
                fill=c[0])
    return y + h


def draw_structural_truss(d, cx, y, top_w, bot_w, h, n_bays=4, scale=1.0):
    """Draw an open cross-braced structural truss section."""
    sw = max(2, int(3 * scale))
    # Main longerons (side rails)
    for s in [-1, 1]:
        tx = cx + s * int(top_w / 2)
        bx = cx + s * int(bot_w / 2)
        d.line([(tx, y), (bx, y + h)], fill=STEEL_MID, width=sw + 1)
        d.line([(tx + s, y), (bx + s, y + h)], fill=STEEL_HIGHLIGHT, width=1)

    # Cross braces
    for i in range(n_bays):
        f1 = i / n_bays
        f2 = (i + 1) / n_bays
        y1 = y + int(h * f1)
        y2 = y + int(h * f2)
        w1 = top_w + (bot_w - top_w) * f1
        w2 = top_w + (bot_w - top_w) * f2
        # X-brace
        d.line([(cx - int(w1 / 2), y1), (cx + int(w2 / 2), y2)],
               fill=STEEL_DARK, width=sw)
        d.line([(cx + int(w1 / 2), y1), (cx - int(w2 / 2), y2)],
               fill=STEEL_DARK, width=sw)
        # Horizontal at each bay boundary
        hw = int(w2 / 2)
        d.line([(cx - hw, y2), (cx + hw, y2)], fill=STEEL_MID, width=sw)

    return y + h


def draw_open_magnetic_nozzle(d, cx, y, throat_w, exit_w, height,
                               coil_fracs=None, bell_exp=2.0,
                               wall_base=8, n_longerons=3, scale=1.0,
                               tint_shift=(0, 0, 0)):
    """Draw an open magnetic cage nozzle — coil rings connected by longerons.

    Draws full-width coil ring bands at specified positions along a bell curve,
    connected by thin structural longerons. Space between rings is transparent,
    creating a clearly open cage structure.

    From side view, each superconducting coil ring appears as a full-width
    horizontal band (the torus profile). The gaps between rings are empty/
    transparent — this is the defining visual of a magnetic nozzle.
    """
    if coil_fracs is None:
        coil_fracs = [0.08, 0.22, 0.37, 0.52, 0.67, 0.82, 0.95]

    sorted_fracs = sorted(coil_fracs)
    coil_th = max(3, int(5 * scale))
    longeron_w = max(2, int(4 * scale))

    # Step 1: Draw structural longerons (vertical rails) evenly across the cage
    # Each longeron traces from a position on the throat to the same
    # proportional position on the exit, following the bell curve.
    n_pts = max(60, int(height / 3))
    # Total number of vertical bars evenly spaced across full width
    # Scale with exit width so larger nozzles get more bars
    total_longerons = max(4, n_longerons, int(exit_w / (80 * scale)))
    for li in range(total_longerons):
        # Position as fraction of full width: 0.0 = left edge, 1.0 = right edge
        pos_frac = li / max(1, total_longerons - 1)
        # x offset from center: -0.5 to +0.5 of width
        offset_frac = pos_frac - 0.5

        pts = []
        for j in range(n_pts):
            t = j / (n_pts - 1)
            w = bell_w(t, throat_w, exit_w, exp=bell_exp)
            lx = cx + int(w * offset_frac)
            ly = y + int(t * height)
            pts.append((lx, ly))

        # Draw the longeron
        for k in range(len(pts) - 1):
            d.line([pts[k], pts[k + 1]], fill=STEEL_MID, width=longeron_w)
        # Left edge highlight
        if li == 0:
            hl_c = clamp_color((
                int(STEEL_LIGHT[0] + tint_shift[0]),
                int(STEEL_LIGHT[1] + tint_shift[1]),
                int(STEEL_LIGHT[2] + tint_shift[2])))
            for k in range(len(pts) - 1):
                d.line([pts[k], pts[k + 1]], fill=hl_c, width=1)

    # Step 2: Draw thin grey coil rings (cage bars)
    for cf in sorted_fracs:
        cy_pos = int(y + cf * height)
        cw = bell_w(cf, throat_w, exit_w, exp=bell_exp)
        band_y = cy_pos - coil_th // 2
        hw = int(cw / 2)

        if hw < 4:
            continue

        # Thin steel-grey ring with subtle shading
        for row in range(coil_th):
            t = row / max(1, coil_th - 1)
            shade = 0.3 + 0.5 * math.sin(t * math.pi)
            r = int(STEEL_DARK[0] + (STEEL_LIGHT[0] - STEEL_DARK[0]) * shade)
            g = int(STEEL_DARK[1] + (STEEL_LIGHT[1] - STEEL_DARK[1]) * shade)
            b = int(STEEL_DARK[2] + (STEEL_LIGHT[2] - STEEL_DARK[2]) * shade)
            ry = band_y + row
            d.rectangle([cx - hw, ry, cx + hw, ry + 1],
                        fill=clamp_color((r, g, b)))

        # Top highlight
        d.line([(cx - hw + 2, band_y), (cx + hw - 2, band_y)],
               fill=STEEL_HIGHLIGHT, width=1)
        # Bottom shadow
        d.line([(cx - hw + 2, band_y + coil_th - 1),
                (cx + hw - 2, band_y + coil_th - 1)],
               fill=STEEL_VERY_DARK, width=1)

    return y + height


# ================================================================
# Engine generators
# ================================================================

def generate_orion():
    """Orion Pulse Drive — 70w x 52h grid, 7000t

    Nuclear pulse propulsion. The most raw, industrial interstellar engine.
    Distinctive anvil silhouette: massive pusher plate at bottom, shock
    absorber column above, pulse unit magazine, narrower mount at top.
    """
    W, H, TOP = 70, 52, 30
    PX = _capped_px(W, H)
    img_w = int(W * PX) + PAD * 2
    img_h = int(H * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    total_h = H * PX
    exit_w = W * PX
    top_w = TOP * PX
    scale = exit_w / 400.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(total_h * 0.02))
    ring_w = top_w + 30
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- PAYLOAD ADAPTER (taper from mount to magazine) ---
    adapter_h = int(total_h * 0.04)
    magazine_w = exit_w * 0.68
    trap(d, cx, y, ring_w, magazine_w, adapter_h, STEEL_MID, outline=STEEL_DARK)
    for frac in [0.3, 0.6]:
        ly = y + int(adapter_h * frac)
        lw = ring_w + (magazine_w - ring_w) * frac
        lhw = int(lw / 2)
        d.rectangle([cx - lhw, ly, cx + lhw, ly + 2], fill=STEEL_LIGHT)
    y += adapter_h

    # --- PULSE UNIT MAGAZINE ---
    mag_h = int(total_h * 0.13)
    mag_w = magazine_w
    rect(d, cx, y, mag_w, mag_h, STEEL_MID, outline=STEEL_DARK)
    d.line([(cx - int(mag_w / 2) + 3, y + 3),
            (cx - int(mag_w / 2) + 3, y + mag_h - 3)],
           fill=STEEL_LIGHT, width=2)
    for frac in [0.33, 0.66]:
        by = y + int(mag_h * frac)
        mhw = int(mag_w / 2)
        d.rectangle([cx - mhw, by, cx + mhw, by + max(3, int(3 * scale))],
                    fill=STEEL_LIGHT)
        d.rectangle([cx - mhw, by + 3, cx + mhw, by + 4], fill=STEEL_DARK)
    n_hatches = max(4, int(mag_w / (60 * scale)))
    hatch_w = max(20, int(30 * scale))
    hatch_h = max(15, int(mag_h * 0.25))
    for row in [0.2, 0.55]:
        hy = y + int(mag_h * row)
        for i in range(n_hatches):
            hx = cx - int(mag_w / 2) + int((i + 0.5) * mag_w / n_hatches)
            d.rectangle([hx - hatch_w // 2, hy, hx + hatch_w // 2, hy + hatch_h],
                        outline=STEEL_DARK)
            d.rectangle([hx - hatch_w // 2 + 1, hy + 1,
                         hx + hatch_w // 2 - 1, hy + 2], fill=STEEL_HIGHLIGHT)
    y += mag_h

    # --- STRUCTURAL TRUSS ---
    truss_h = int(total_h * 0.10)
    absorber_col_w = exit_w * 0.30
    y = draw_structural_truss(d, cx, y, mag_w, absorber_col_w + 40,
                               truss_h, n_bays=5, scale=scale)

    # --- SHOCK ABSORBER ASSEMBLY ---
    absorber_h = int(total_h * 0.27)
    abs_w = absorber_col_w
    trap(d, cx, y, abs_w + 40, abs_w + 60, absorber_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    trap(d, cx, y + 3, abs_w + 20, abs_w + 40, absorber_h - 3, STEEL_MID)

    n_pistons = 4
    piston_w = max(30, int(abs_w * 0.18))
    piston_spacing = (abs_w - piston_w) / max(1, n_pistons - 1)
    for i in range(n_pistons):
        px = cx - int(abs_w / 2) + int(piston_w / 2) + int(i * piston_spacing)
        cyl_h = int(absorber_h * 0.85)
        cyl_w = piston_w
        rect(d, px, y + int(absorber_h * 0.08), cyl_w, cyl_h,
             STEEL_VERY_DARK, outline=STEEL_DARK)
        rod_w = cyl_w * 0.45
        rod_h = cyl_h * 0.6
        rod_top = y + int(absorber_h * 0.08) + int(cyl_h * 0.35)
        rect(d, px, rod_top, rod_w, rod_h, STEEL_HIGHLIGHT)
        for rf in [0.4, 0.55, 0.7]:
            ry = y + int(absorber_h * 0.08) + int(cyl_h * rf)
            d.rectangle([px - int(rod_w / 2) - 2, ry,
                         px + int(rod_w / 2) + 2, ry + 2], fill=STEEL_DARK)
        # Compression springs wrapping around piston rod
        spring_r = max(2, int(rod_w * 0.65))
        spring_start = y + int(absorber_h * 0.08) + int(cyl_h * 0.08)
        spring_end = rod_top - 2
        if spring_end > spring_start:
            n_spring_coils = max(6, int((spring_end - spring_start) / (6 * scale)))
            coil_step = (spring_end - spring_start) / max(1, n_spring_coils)
            for j in range(n_spring_coils):
                sy = spring_start + int(j * coil_step)
                d.line([(px - int(spring_r), sy),
                        (px + int(spring_r), sy + int(coil_step * 0.5))],
                       fill=STEEL_LIGHT, width=max(1, int(2 * scale)))
                d.line([(px + int(spring_r), sy + int(coil_step * 0.5)),
                        (px - int(spring_r), sy + int(coil_step))],
                       fill=STEEL_MID, width=max(1, int(2 * scale)))

    # Spring coils between pistons
    spring_w = max(10, int(piston_spacing * 0.4))
    for i in range(n_pistons - 1):
        sx = cx - int(abs_w / 2) + int(piston_w) + int(i * piston_spacing)
        n_coils = max(8, int(absorber_h / (15 * scale)))
        for j in range(n_coils):
            sy = y + int(absorber_h * 0.1) + int(j * absorber_h * 0.78 / n_coils)
            if j % 2 == 0:
                d.line([(sx, sy), (sx + spring_w, sy + int(absorber_h * 0.78 / n_coils / 2))],
                       fill=STEEL_LIGHT, width=max(1, int(2 * scale)))
            else:
                d.line([(sx + spring_w, sy),
                        (sx, sy + int(absorber_h * 0.78 / n_coils / 2))],
                       fill=STEEL_LIGHT, width=max(1, int(2 * scale)))

    for frac in [0.15, 0.45, 0.75]:
        by = y + int(absorber_h * frac)
        ahw = int((abs_w + 50) / 2)
        d.rectangle([cx - ahw, by, cx + ahw, by + max(3, int(4 * scale))],
                    fill=STEEL_LIGHT)
    y += absorber_h

    # --- PLATE ADAPTER ---
    plate_adapter_h = int(total_h * 0.06)
    trap(d, cx, y, abs_w + 60, exit_w, plate_adapter_h, PLATE_MID, outline=PLATE_DARK)
    n_ribs = max(6, int(exit_w / (50 * scale)))
    for i in range(n_ribs):
        rx = cx - int(exit_w / 2) + int((i + 0.5) * exit_w / n_ribs)
        top_x = cx - int((abs_w + 60) / 2) + int((i + 0.5) * (abs_w + 60) / n_ribs)
        d.line([(top_x, y + 2), (rx, y + plate_adapter_h - 2)],
               fill=PLATE_DARK, width=max(2, int(2 * scale)))
    # Charge ejection channel
    channel_r = max(6, int(10 * scale))
    circ(d, cx, y + plate_adapter_h // 2, channel_r + 2, fill=PLATE_DARK)
    circ(d, cx, y + plate_adapter_h // 2, channel_r, fill=INTERIOR)
    y += plate_adapter_h

    # --- PUSHER PLATE ---
    plate_h = int(total_h * 0.38)
    plate_w = exit_w
    rect(d, cx, y, plate_w, plate_h, PLATE_MID, outline=PLATE_DARK)

    # Center boss
    boss_r = max(20, int(plate_w * 0.06))
    boss_y = y + int(plate_h * 0.5)
    circ(d, cx, boss_y, boss_r + 3, fill=PLATE_LIGHT)
    circ(d, cx, boss_y, boss_r, fill=PLATE_HIGHLIGHT)
    circ(d, cx, boss_y, boss_r - 4, fill=PLATE_LIGHT)

    n_rings = max(8, int(plate_h / (20 * scale)))
    for i in range(n_rings):
        ry = y + int((i + 0.5) * plate_h / n_rings)
        phw = int(plate_w / 2)
        ring_h = max(2, int(3 * scale))
        if i % 2 == 0:
            d.rectangle([cx - phw + 4, ry, cx + phw - 4, ry + ring_h], fill=PLATE_LIGHT)
        else:
            d.rectangle([cx - phw + 4, ry, cx + phw - 4, ry + ring_h], fill=PLATE_DARK)

    random.seed(42)
    n_spots = max(30, int(plate_w * plate_h / (200 * scale)))
    for _ in range(n_spots):
        sx = cx + random.randint(-int(plate_w / 2) + 8, int(plate_w / 2) - 8)
        sy_frac = random.random() ** 0.5
        sy = y + int(plate_h * 0.3) + int(plate_h * 0.65 * sy_frac)
        sr = max(3, int(random.randint(4, 12) * scale))
        circ(d, sx, sy, sr, fill=PLATE_ABLATION)

    n_ports = max(12, int(plate_w / (30 * scale)))
    port_r = max(4, int(6 * scale))
    port_y = y + int(plate_h * 0.85)
    for i in range(n_ports):
        px = cx - int(plate_w / 2) + int((i + 0.5) * plate_w / n_ports)
        circ(d, px, port_y, port_r + 1, fill=PLATE_DARK)
        circ(d, px, port_y, port_r - 1, fill=INTERIOR)

    lip_h = max(6, int(10 * scale))
    phw = int(plate_w / 2)
    d.rectangle([cx - phw, y + plate_h - lip_h, cx + phw, y + plate_h],
                fill=PLATE_DARK, outline=STEEL_VERY_DARK)
    chamfer = max(10, int(15 * scale))
    d.polygon([(cx - phw, y + plate_h - lip_h),
               (cx - phw + chamfer, y + plate_h),
               (cx - phw, y + plate_h)], fill=PLATE_DARK)
    d.polygon([(cx + phw, y + plate_h - lip_h),
               (cx + phw - chamfer, y + plate_h),
               (cx + phw, y + plate_h)], fill=PLATE_DARK)
    d.line([(cx - phw + 3, y + 3), (cx - phw + 3, y + plate_h - 3)],
           fill=PLATE_HIGHLIGHT, width=max(2, int(3 * scale)))

    return img


def generate_daedalus(stage=1):
    """Daedalus ICF Fusion Engine — S1: 60w x 48h, S2: 44w x 34h

    Inertial confinement fusion. Wide hemispherical reaction dome feeds
    into an open magnetic cage nozzle. Electron beam emitters on the reactor.
    Dimensions based on 40m-class reaction chamber.
    """
    if stage == 1:
        W, H, TOP = 60, 48, 20
    else:
        W, H, TOP = 44, 34, 16

    PX = _capped_px(W, H)
    img_w = int(W * PX) + PAD * 2
    img_h = int(H * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    total_h = H * PX
    exit_w = W * PX
    top_w = TOP * PX
    scale = exit_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(total_h * 0.025))
    ring_w = top_w + 20
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- REACTOR HOUSING ---
    reactor_h = int(total_h * 0.18)
    reactor_top_w = ring_w + 10
    reactor_bot_w = exit_w * 0.55
    y = draw_reactor_housing(d, cx, y, reactor_top_w, reactor_bot_w,
                              reactor_h, tint="reactor", scale=scale)

    # --- HEMISPHERICAL DOME (reaction chamber) ---
    dome_h = int(total_h * 0.14)
    dome_top_w = reactor_bot_w
    dome_max_w = reactor_bot_w * 1.25
    throat_w = exit_w * 0.16

    # Helper to get dome width at fractional height
    def dome_w_at(frac):
        if frac < 0.45:
            curve = math.sin(frac / 0.45 * math.pi / 2)
            return dome_top_w + (dome_max_w - dome_top_w) * curve
        else:
            curve = (frac - 0.45) / 0.55
            return dome_max_w - (dome_max_w - throat_w - 30) * curve

    # Draw the dome body
    n_strips = max(25, int(dome_h / 4))
    sh = dome_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        w1 = dome_w_at(t)
        w2 = dome_w_at(t1)
        sy = y + t * dome_h
        trap(d, cx, int(sy), w1, w2, int(sh) + 1, fill=REACTOR_MID,
             outline=REACTOR_DARK)

    # Structural bands on dome
    for frac in [0.25, 0.5, 0.75]:
        by = y + int(dome_h * frac)
        bw = dome_w_at(frac)
        bhw = int(bw / 2)
        d.rectangle([cx - bhw, by, cx + bhw, by + max(2, int(2 * scale))],
                    fill=REACTOR_LIGHT)

    # Left edge highlight
    d.line([(cx - int(dome_top_w / 2) + 2, y + 2),
            (cx - int(dome_max_w / 2) + 2, y + int(dome_h * 0.45))],
           fill=REACTOR_LIGHT, width=max(1, int(2 * scale)))

    # Containment ring at dome exit
    draw_containment_ring(d, cx, y + dome_h - max(4, int(5 * scale)),
                          throat_w + 30, max(6, int(8 * scale)),
                          tint="energy", scale=scale)
    y += dome_h

    # --- OPEN MAGNETIC NOZZLE CAGE ---
    nozzle_h = int(total_h * 0.68)  # slightly shorter to keep last ring in bounds
    n_coils = 8 if stage == 1 else 6
    coil_fracs = [(i + 0.5) / n_coils for i in range(n_coils)]
    nozzle_y_start = y
    y = draw_open_magnetic_nozzle(d, cx, y, throat_w, exit_w, nozzle_h,
                                   coil_fracs=coil_fracs, bell_exp=2.0,
                                   wall_base=max(6, int(10 * scale)),
                                   n_longerons=3, scale=scale)

    # --- ELECTRON BEAM EMITTERS (one between each pair of coil rings) ---
    # Each emitter sits in the gap between two adjacent rings, with its
    # housing aligned to the outer nozzle edge. Barrel angles toward the
    # exact bottom center of the nozzle (cx, nozzle_y_start + nozzle_h).
    target_x = cx
    target_y = nozzle_y_start + nozzle_h  # bottom center of nozzle
    emitter_h = max(3, int(4 * scale))
    housing_w = max(5, int(8 * scale))  # width along the nozzle edge
    barrel_len = max(8, int(exit_w * 0.06))

    for i in range(len(coil_fracs) - 1):
        # Midpoint between two adjacent coil rings
        mid_frac = (coil_fracs[i] + coil_fracs[i + 1]) / 2.0
        mid_y = nozzle_y_start + int(mid_frac * nozzle_h)
        mid_w = bell_w(mid_frac, throat_w, exit_w, exp=2.0)
        mid_hw = int(mid_w / 2)

        for s in [-1, 1]:
            # Housing box on the outer edge of the nozzle
            edge_x = cx + s * mid_hw
            hx1 = edge_x - housing_w // 2
            hx2 = edge_x + housing_w // 2
            d.rectangle([hx1, mid_y - emitter_h // 2,
                         hx2, mid_y + emitter_h // 2],
                        fill=REACTOR_LIGHT, outline=REACTOR_DARK)

            # Barrel angled toward bottom center of nozzle
            dx = target_x - edge_x
            dy = target_y - mid_y
            dist = math.sqrt(dx * dx + dy * dy)
            if dist > 0:
                ux, uy = dx / dist, dy / dist
            else:
                ux, uy = 0, 1
            tip_x = edge_x + int(ux * barrel_len)
            tip_y = mid_y + int(uy * barrel_len)
            barrel_h = max(2, int(3 * scale))
            # Draw barrel as a line from housing to tip
            d.line([(edge_x, mid_y), (tip_x, tip_y)],
                   fill=STEEL_MID, width=barrel_h)
            # Emitter tip glow
            circ(d, tip_x, tip_y, max(2, int(3 * scale)), fill=ENERGY_LIGHT)
            circ(d, tip_x, tip_y, max(1, int(2 * scale)), fill=(120, 180, 220))

    return img


def generate_zpinch(variant="probe"):
    """Z-Pinch Fusion Engine — Probe: 10w x 18h, Advanced: 16w x 26h

    Z-pinch confinement in a narrow tube with intense magnetic fields.
    Dimensions based on narrow tube confinement geometry — tall and narrow.
    Open magnetic cage nozzle with dramatic divergence from the narrow tube.
    """
    if variant == "probe":
        W, H, TOP = 10, 18, 6
    else:
        W, H, TOP = 16, 26, 10

    PX = _capped_px(W, H)
    img_w = int(W * PX) + PAD * 2
    img_h = int(H * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    total_h = H * PX
    exit_w = W * PX
    top_w = TOP * PX
    scale = exit_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(total_h * 0.025))
    ring_w = top_w + 16
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- REACTOR/DRIVER SECTION ---
    reactor_h = int(total_h * 0.16)
    reactor_w = top_w + 30
    y = draw_reactor_housing(d, cx, y, ring_w, reactor_w, reactor_h,
                              tint="reactor", scale=scale)

    # --- Z-PINCH COIL BODY ---
    body_h = int(total_h * 0.52)
    body_top_w = reactor_w
    body_bot_w = exit_w * 0.72

    n_coils = 5 if variant == "probe" else 8
    coil_spacing = body_h / (n_coils + 1)
    coil_th = max(5, int(5 * scale))

    n_strips = max(30, int(body_h / 6))
    sh = body_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        w1 = body_top_w + (body_bot_w - body_top_w) * t
        w2 = body_top_w + (body_bot_w - body_top_w) * t1
        sy = y + t * body_h
        band = math.sin(t * math.pi * n_coils * 2) * 5
        r = int(STEEL_MID[0] + band)
        g = int(STEEL_MID[1] + band)
        b = int(STEEL_MID[2] + band + 5)
        trap(d, cx, int(sy), w1, w2, int(sh) + 1, fill=clamp_color((r, g, b)))

    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        w1 = body_top_w + (body_bot_w - body_top_w) * t
        w2 = body_top_w + (body_bot_w - body_top_w) * t1
        iw1 = w1 * 0.55
        iw2 = w2 * 0.55
        sy = y + t * body_h
        trap(d, cx, int(sy), iw1, iw2, int(sh) + 1, fill=INTERIOR)

    d.line([(cx - int(body_top_w / 2) + 3, y),
            (cx - int(body_bot_w / 2) + 3, y + body_h)],
           fill=STEEL_LIGHT, width=max(2, int(2 * scale)))

    # Electrode structures
    electrode_len = max(8, int(14 * scale))
    electrode_w = max(3, int(4 * scale))
    for pos_frac, body_frac in [(0.02, 0.02), (0.96, 0.96)]:
        ey = y + int(body_h * pos_frac)
        ew = body_top_w + (body_bot_w - body_top_w) * body_frac
        for s in [-1, 1]:
            ex = cx + s * int(ew / 2)
            ex2 = ex + s * electrode_len
            d.rectangle([min(ex, ex2), ey - electrode_w,
                         max(ex, ex2), ey + electrode_w],
                        fill=COIL_MID, outline=COIL_DARK)
            tip_x1 = ex + s * (electrode_len - 3)
            d.rectangle([min(tip_x1, ex2), ey - electrode_w + 1,
                         max(tip_x1, ex2), ey + electrode_w - 1],
                        fill=COIL_LIGHT)

    for i in range(n_coils):
        cy_pos = y + int((i + 1) * coil_spacing)
        frac = (i + 1) / (n_coils + 1)
        cw = body_top_w + (body_bot_w - body_top_w) * frac
        draw_magnetic_coil(d, cx, cy_pos, cw, coil_th, scale=scale)

    rib_h = max(2, int(3 * scale))
    for i in range(n_coils + 1):
        if i == 0:
            ry = y + int(coil_spacing * 0.5)
        elif i == n_coils:
            ry = y + body_h - int(coil_spacing * 0.5)
        else:
            ry = y + int((i + 0.5) * coil_spacing)
        frac = ry / (y + body_h) if body_h > 0 else 0
        rw = body_top_w + (body_bot_w - body_top_w) * min(1, frac)
        rhw = int(rw / 2)
        d.rectangle([cx - rhw, ry - rib_h, cx + rhw, ry + rib_h], fill=STEEL_LIGHT)

    y += body_h

    # --- CONVERGING SECTION ---
    conv_h = int(total_h * 0.05)
    throat_w = body_bot_w * 0.5
    trap(d, cx, y, body_bot_w, throat_w, conv_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    y += conv_h

    # --- OPEN MAGNETIC CAGE NOZZLE ---
    nozzle_h = int(total_h * 0.22)
    n_nozzle_coils = 4 if variant == "probe" else 6
    nozzle_coil_fracs = [(i + 0.5) / n_nozzle_coils for i in range(n_nozzle_coils)]
    y = draw_open_magnetic_nozzle(d, cx, y, throat_w, exit_w, nozzle_h,
                                   coil_fracs=nozzle_coil_fracs, bell_exp=1.5,
                                   wall_base=max(5, int(8 * scale)),
                                   n_longerons=3, scale=scale)

    return img


def generate_amcat():
    """Antimatter-Catalyzed Fusion Engine — 22w x 16h, 2000t

    ICAN-II style hybrid. Compact design — Penning trap antimatter storage
    feeds into a fusion reactor with ion beam injectors. Open magnetic cage
    nozzle. More compact than Daedalus (less reaction area needed).
    """
    W, H, TOP = 22, 16, 12
    PX = _capped_px(W, H)
    img_w = int(W * PX) + PAD * 2
    img_h = int(H * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    total_h = H * PX
    exit_w = W * PX
    top_w = TOP * PX
    scale = exit_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(total_h * 0.025))
    ring_w = top_w + 20
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- PENNING TRAP MODULE ---
    trap_h = max(20, int(total_h * 0.07))
    trap_w = ring_w + 16
    rect(d, cx, y, trap_w, trap_h, AM_MID, outline=AM_DARK)
    coil_ring_h = max(3, int(4 * scale))
    for frac in [0.2, 0.5, 0.8]:
        ry = y + int(trap_h * frac)
        hw = int(trap_w / 2)
        extra = max(3, int(5 * scale))
        d.rectangle([cx - hw - extra, ry - coil_ring_h,
                     cx + hw + extra, ry + coil_ring_h], fill=AM_LIGHT)
        d.rectangle([cx - hw - extra, ry - coil_ring_h,
                     cx + hw + extra, ry - coil_ring_h + 1], fill=AM_HIGHLIGHT)
        d.rectangle([cx - hw - extra, ry + coil_ring_h - 1,
                     cx + hw + extra, ry + coil_ring_h], fill=AM_DARK)
    glow_y = y + trap_h // 2
    glow_hw = int(trap_w * 0.35)
    d.line([(cx - glow_hw, glow_y), (cx + glow_hw, glow_y)],
           fill=AM_HIGHLIGHT, width=max(2, int(3 * scale)))
    d.line([(cx - glow_hw + 4, glow_y - 1), (cx + glow_hw - 4, glow_y - 1)],
           fill=(180, 150, 210), width=1)
    d.line([(cx - int(trap_w / 2) + 2, y + 2),
            (cx - int(trap_w / 2) + 2, y + trap_h - 2)],
           fill=AM_LIGHT, width=max(1, int(2 * scale)))
    y += trap_h

    # --- FUSION REACTOR ---
    reactor_h = int(total_h * 0.20)
    reactor_top_w = trap_w + 10
    reactor_max_w = exit_w * 0.62
    reactor_bot_w = reactor_max_w - 20

    half_h = reactor_h // 2
    trap(d, cx, y, reactor_top_w, reactor_max_w, half_h,
         REACTOR_MID, outline=REACTOR_DARK)
    trap(d, cx, y + half_h, reactor_max_w, reactor_bot_w, half_h,
         REACTOR_MID, outline=REACTOR_DARK)

    d.line([(cx - int(reactor_top_w / 2) + 2, y + 2),
            (cx - int(reactor_max_w / 2) + 2, y + half_h)],
           fill=REACTOR_LIGHT, width=max(1, int(2 * scale)))

    band_h = max(3, int(3 * scale))
    for frac in [0.25, 0.5, 0.75]:
        by = y + int(reactor_h * frac)
        if frac < 0.5:
            bw = reactor_top_w + (reactor_max_w - reactor_top_w) * (frac / 0.5)
        else:
            bw = reactor_max_w + (reactor_bot_w - reactor_max_w) * ((frac - 0.5) / 0.5)
        bhw = int(bw / 2)
        d.rectangle([cx - bhw, by, cx + bhw, by + band_h], fill=REACTOR_LIGHT)
        d.rectangle([cx - bhw, by + band_h, cx + bhw, by + band_h + 1],
                    fill=REACTOR_DARK)

    # Ion beam injector nubs
    nub_len = max(6, int(10 * scale))
    nub_h = max(4, int(6 * scale))
    for s in [-1, 1]:
        for frac in [0.3, 0.5, 0.7]:
            ny = y + int(reactor_h * frac)
            if frac < 0.5:
                nw = reactor_top_w + (reactor_max_w - reactor_top_w) * (frac / 0.5)
            else:
                nw = reactor_max_w + (reactor_bot_w - reactor_max_w) * ((frac - 0.5) / 0.5)
            nx = cx + s * int(nw / 2)
            nx2 = nx + s * nub_len
            d.rectangle([min(nx, nx2), ny - nub_h // 2,
                         max(nx, nx2), ny + nub_h // 2],
                        fill=STEEL_MID, outline=STEEL_DARK)
            circ(d, nx2, ny, max(2, nub_h // 3), fill=ENERGY_MID)

    vp_r = max(5, int(8 * scale))
    for s in [-1, 1]:
        vpx = cx + s * int(reactor_max_w * 0.3)
        vpy = y + int(reactor_h * 0.5)
        circ(d, vpx, vpy, vp_r + 2, fill=REACTOR_DARK)
        circ(d, vpx, vpy, vp_r, fill=ENERGY_MID)
        circ(d, vpx, vpy, vp_r - 2, fill=ENERGY_LIGHT)
    y += reactor_h

    # --- CONTAINMENT RING ---
    cont_h = max(10, int(total_h * 0.03))
    y = draw_containment_ring(d, cx, y, reactor_bot_w, cont_h,
                               tint="copper", scale=scale)

    # --- TRANSITION ---
    trans_h = int(total_h * 0.04)
    throat_w = exit_w * 0.16
    trap(d, cx, y, reactor_bot_w, throat_w + 30, trans_h,
         STEEL_DARK, outline=STEEL_VERY_DARK)
    y += trans_h

    # --- OPEN MAGNETIC NOZZLE CAGE ---
    nozzle_h = int(total_h * 0.60)
    coil_fracs = [0.1, 0.28, 0.46, 0.65, 0.84]
    y = draw_open_magnetic_nozzle(d, cx, y, throat_w, exit_w, nozzle_h,
                                   coil_fracs=coil_fracs, bell_exp=1.8,
                                   wall_base=max(6, int(9 * scale)),
                                   n_longerons=3, scale=scale,
                                   tint_shift=(5, -2, -5))

    return img


def generate_am_torch():
    """Antimatter Torch — 8w x 12h, 800t

    Beam-core antimatter annihilation in a magnetic bottle. Very compact —
    the magnetic bottle solenoid is the entire reaction vessel. Open cage
    of coil rings with magnetic mirrors at both ends.
    """
    W, H, TOP = 8, 12, 6
    PX = _capped_px(W, H)
    img_w = int(W * PX) + PAD * 2
    img_h = int(H * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    total_h = H * PX
    exit_w = W * PX
    top_w = TOP * PX
    scale = exit_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(total_h * 0.025))
    ring_w = top_w + 16
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- ANTIMATTER FEED SYSTEM ---
    feed_h = int(total_h * 0.16)
    feed_top_w = ring_w + 4
    feed_bot_w = exit_w * 0.42
    y = draw_reactor_housing(d, cx, y, feed_top_w, feed_bot_w, feed_h,
                              tint="antimatter", scale=scale)

    for frac in [0.25, 0.55, 0.85]:
        ry = y - int(feed_h * (1 - frac))
        rw = feed_top_w + (feed_bot_w - feed_top_w) * frac
        ring_h = max(4, int(5 * scale))
        draw_containment_ring(d, cx, ry - ring_h // 2, rw, ring_h,
                               tint="antimatter", scale=scale)

    # --- UPPER MAGNETIC MIRROR ---
    # Width matches the narrow bottom of the purple feed trapezoid
    mirror_w = feed_bot_w + 10
    mirror_coil_th = max(5, int(7 * scale))

    hw = int(mirror_w / 2)
    for row in range(mirror_coil_th):
        t = row / max(1, mirror_coil_th - 1)
        shade = 0.3 + 0.5 * math.sin(t * math.pi)
        r = int(STEEL_DARK[0] + (STEEL_LIGHT[0] - STEEL_DARK[0]) * shade)
        g = int(STEEL_DARK[1] + (STEEL_LIGHT[1] - STEEL_DARK[1]) * shade)
        b = int(STEEL_DARK[2] + (STEEL_LIGHT[2] - STEEL_DARK[2]) * shade)
        d.rectangle([cx - hw, y + row, cx + hw, y + row + 1],
                    fill=clamp_color((r, g, b)))
    d.line([(cx - hw + 2, y), (cx + hw - 2, y)],
           fill=STEEL_HIGHLIGHT, width=1)
    d.line([(cx - hw + 2, y + mirror_coil_th - 1),
            (cx + hw - 2, y + mirror_coil_th - 1)],
           fill=STEEL_VERY_DARK, width=1)
    y += mirror_coil_th + 4

    # --- OPEN SOLENOID CAGE (Magnetic Bottle) ---
    # Narrow rectangular cage matching the feed exit width
    cage_h = int(total_h * 0.30)
    cage_w = feed_bot_w  # matches narrow end of purple trapezoid
    n_cage_coils = 5
    cage_coil_th = max(3, int(5 * scale))
    cage_coil_spacing = cage_h / (n_cage_coils + 1)
    longeron_w = max(3, int(4 * scale))

    cage_hw = int(cage_w / 2)
    n_cage_longerons = 4
    for li in range(n_cage_longerons):
        pos_frac = li / max(1, n_cage_longerons - 1)
        lx = cx - cage_hw + int(pos_frac * cage_w)
        d.rectangle([lx - longeron_w // 2, y,
                     lx + longeron_w // 2, y + cage_h],
                    fill=STEEL_MID)
        if li == 0:
            d.rectangle([lx - longeron_w // 2, y,
                         lx - longeron_w // 2 + 1, y + cage_h],
                        fill=STEEL_LIGHT)

    coil_positions = [(i + 1) * cage_coil_spacing for i in range(n_cage_coils)]
    for i in range(n_cage_coils):
        cy_pos = int(y + coil_positions[i])
        band_y = cy_pos - cage_coil_th // 2

        for row in range(cage_coil_th):
            t = row / max(1, cage_coil_th - 1)
            shade = 0.3 + 0.5 * math.sin(t * math.pi)
            r = int(STEEL_DARK[0] + (STEEL_LIGHT[0] - STEEL_DARK[0]) * shade)
            g = int(STEEL_DARK[1] + (STEEL_LIGHT[1] - STEEL_DARK[1]) * shade)
            b = int(STEEL_DARK[2] + (STEEL_LIGHT[2] - STEEL_DARK[2]) * shade)
            ry = band_y + row
            d.rectangle([cx - cage_hw, ry, cx + cage_hw, ry + 1],
                        fill=clamp_color((r, g, b)))

        d.line([(cx - cage_hw + 2, band_y), (cx + cage_hw - 2, band_y)],
               fill=STEEL_HIGHLIGHT, width=1)
        d.line([(cx - cage_hw + 2, band_y + cage_coil_th - 1),
                (cx + cage_hw - 2, band_y + cage_coil_th - 1)],
               fill=STEEL_VERY_DARK, width=1)

    y += cage_h

    # --- LOWER MAGNETIC MIRROR ---
    lower_mirror_w = cage_w + 10
    hw = int(lower_mirror_w / 2)
    for row in range(mirror_coil_th):
        t = row / max(1, mirror_coil_th - 1)
        shade = 0.3 + 0.5 * math.sin(t * math.pi)
        r = int(STEEL_DARK[0] + (STEEL_LIGHT[0] - STEEL_DARK[0]) * shade)
        g = int(STEEL_DARK[1] + (STEEL_LIGHT[1] - STEEL_DARK[1]) * shade)
        b = int(STEEL_DARK[2] + (STEEL_LIGHT[2] - STEEL_DARK[2]) * shade)
        d.rectangle([cx - hw, y + row, cx + hw, y + row + 1],
                    fill=clamp_color((r, g, b)))
    d.line([(cx - hw + 2, y), (cx + hw - 2, y)],
           fill=STEEL_HIGHLIGHT, width=1)
    d.line([(cx - hw + 2, y + mirror_coil_th - 1),
            (cx + hw - 2, y + mirror_coil_th - 1)],
           fill=STEEL_VERY_DARK, width=1)
    y += mirror_coil_th + 2

    # --- EXPANDING OPEN NOZZLE CAGE (trapezoid shape) ---
    nozzle_h = int(total_h * 0.22)
    nozzle_throat = cage_w
    nozzle_exit = exit_w * 0.9
    n_nozzle_coils = 4
    nozzle_coil_fracs = [(i + 0.5) / n_nozzle_coils for i in range(n_nozzle_coils)]
    y = draw_open_magnetic_nozzle(d, cx, y, nozzle_throat, nozzle_exit, nozzle_h,
                                   coil_fracs=nozzle_coil_fracs, bell_exp=1.5,
                                   wall_base=max(4, int(6 * scale)),
                                   n_longerons=2, scale=scale)

    return img


def generate_gamma_conversion():
    """Gamma Conversion Drive — 34w x 26h, 1800t

    Gamma ray laser from antimatter annihilation in pinch discharge.
    Three-section design: compact annihilation reactor at top, pair production
    chamber in the middle, massive parabolic reflector at bottom with
    focal point structure.
    """
    W, H, TOP = 34, 26, 14
    PX = _capped_px(W, H)
    img_w = int(W * PX) + PAD * 2
    img_h = int(H * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    total_h = H * PX
    exit_w = W * PX
    top_w = TOP * PX
    scale = exit_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(total_h * 0.025))
    ring_w = top_w + 20
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- ANNIHILATION REACTOR (compact purple dome) ---
    dome_h = int(total_h * 0.10)
    dome_w = ring_w + 30

    n_strips = max(15, int(dome_h / 4))
    sh = dome_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        # Dome expansion curve
        curve = math.sin(t * math.pi * 0.5)
        curve1 = math.sin(t1 * math.pi * 0.5)
        w1 = ring_w + (dome_w - ring_w) * curve
        w2 = ring_w + (dome_w - ring_w) * curve1
        sy = y + t * dome_h
        shade = 0.3 + 0.5 * curve
        c = lerp_color(AM_DARK, AM_MID, shade)
        trap(d, cx, int(sy), w1, w2, int(sh) + 1, fill=c)

    d.line([(cx - int(ring_w / 2) + 2, y + 2),
            (cx - int(dome_w / 2) + 2, y + dome_h - 2)],
           fill=AM_LIGHT, width=max(1, int(2 * scale)))
    # Containment bands
    for frac in [0.3, 0.6]:
        by = y + int(dome_h * frac)
        curve = math.sin(frac * math.pi * 0.5)
        bw = ring_w + (dome_w - ring_w) * curve
        bhw = int(bw / 2) + 3
        d.rectangle([cx - bhw, by, cx + bhw, by + max(3, int(3 * scale))],
                    fill=AM_HIGHLIGHT)
        d.rectangle([cx - bhw, by + 3, cx + bhw, by + 4], fill=AM_DARK)
    y += dome_h

    # --- PAIR PRODUCTION CHAMBER (blue cylinder) ---
    chamber_h = int(total_h * 0.12)
    chamber_w = dome_w + 10

    rect(d, cx, y, chamber_w, chamber_h, ENERGY_MID, outline=ENERGY_DARK)
    d.line([(cx - int(chamber_w / 2) + 3, y + 2),
            (cx - int(chamber_w / 2) + 3, y + chamber_h - 2)],
           fill=ENERGY_LIGHT, width=max(2, int(2 * scale)))

    # Horizontal bands
    for frac in [0.3, 0.6]:
        by = y + int(chamber_h * frac)
        chw = int(chamber_w / 2)
        d.rectangle([cx - chw, by, cx + chw, by + max(3, int(3 * scale))],
                    fill=ENERGY_LIGHT)

    # Viewport windows
    vp_r = max(5, int(7 * scale))
    for s in [-1, 1]:
        vpx = cx + s * int(chamber_w * 0.32)
        vpy = y + chamber_h // 2
        circ(d, vpx, vpy, vp_r + 2, fill=ENERGY_DARK)
        circ(d, vpx, vpy, vp_r, fill=(70, 130, 180))
        circ(d, vpx, vpy, vp_r - 2, fill=(90, 150, 200))
    y += chamber_h

    # --- TRANSITION ---
    trans_h = int(total_h * 0.04)
    reflector_top_w = chamber_w + 20
    trap(d, cx, y, chamber_w, reflector_top_w, trans_h,
         STEEL_DARK, outline=STEEL_VERY_DARK)
    y += trans_h

    # --- PARABOLIC REFLECTOR ---
    reflector_h = int(total_h * 0.65)
    reflector_bot_w = exit_w

    # Draw parabolic profile (curves inward at top, flares at bottom)
    n_strips = max(50, int(reflector_h / 4))
    sh = reflector_h / n_strips

    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        # Parabolic curve: w = top + (bot-top) * t^0.6
        w1 = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t ** 0.6)
        w2 = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t1 ** 0.6)
        sy = y + t * reflector_h

        shade = 0.3 + 0.5 * t
        c = lerp_color(GAMMA_DARK, GAMMA_MID, shade)
        trap(d, cx, int(sy), w1, w2, int(sh) + 1, fill=c)

    # Interior surface (reflective)
    wall_base = max(8, int(12 * scale))
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        w1 = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t ** 0.6)
        w2 = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t1 ** 0.6)
        wt = wall_base * (1.0 - 0.3 * t)
        iw1 = max(0, w1 - wt * 2)
        iw2 = max(0, w2 - wt * 2)
        sy = y + t * reflector_h
        if iw1 > 4 and iw2 > 4:
            # Slightly warm-tinted interior
            heat = t ** 0.5
            ir = int(GAMMA_DARK[0] - 10 + heat * 5)
            ig = int(GAMMA_DARK[1] - 10 + heat * 3)
            ib = int(GAMMA_DARK[2] - 10 + heat * 3)
            trap(d, cx, int(sy), iw1, iw2, int(sh) + 1,
                 fill=clamp_color((ir, ig, ib)))

    # Curved panel lines following the parabola
    n_panel_lines = max(6, int(reflector_bot_w / (60 * scale)))
    for pi in range(n_panel_lines):
        pfrac = (pi + 1) / (n_panel_lines + 1)
        pts = []
        for j in range(30):
            t = j / 29
            w = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t ** 0.6)
            wt = wall_base * (1.0 - 0.3 * t)
            outer_hw = w / 2
            inner_hw = max(0, outer_hw - wt)
            panel_x = inner_hw + (outer_hw - inner_hw) * pfrac
            py = y + int(t * reflector_h)
            pts.append((cx - int(panel_x), py))
        for k in range(len(pts) - 1):
            d.line([pts[k], pts[k + 1]], fill=GAMMA_DARK, width=1)
        # Mirror for right side
        pts_r = [(2 * cx - px, py) for px, py in pts]
        for k in range(len(pts_r) - 1):
            d.line([pts_r[k], pts_r[k + 1]], fill=GAMMA_DARK, width=1)

    # Horizontal structural bands (curved)
    for frac in [0.2, 0.4, 0.6, 0.8]:
        by = y + int(reflector_h * frac)
        bw = reflector_top_w + (reflector_bot_w - reflector_top_w) * (frac ** 0.6)
        bhw = int(bw / 2)
        band_h = max(2, int(3 * scale))
        d.rectangle([cx - bhw, by, cx + bhw, by + band_h], fill=GAMMA_LIGHT)
        d.rectangle([cx - bhw, by + band_h, cx + bhw, by + band_h + 1],
                    fill=GAMMA_DARK)

    # Focal point structure — a small crosshair at the parabolic focus
    focal_frac = 0.35  # focal point ~35% down the reflector
    focal_y = y + int(reflector_h * focal_frac)
    focal_w = reflector_top_w + (reflector_bot_w - reflector_top_w) * (focal_frac ** 0.6)
    wt_focal = wall_base * (1.0 - 0.3 * focal_frac)
    focal_inner_hw = max(0, focal_w / 2 - wt_focal)
    focal_arm_len = max(8, int(focal_inner_hw * 0.3))

    # Support struts to focal element
    for s in [-1, 1]:
        strut_x = cx + s * int(focal_inner_hw)
        d.line([(strut_x, focal_y), (cx + s * int(focal_arm_len * 0.5), focal_y)],
               fill=STEEL_MID, width=max(2, int(2 * scale)))
    # Focal element (small bright point)
    focal_r = max(4, int(6 * scale))
    circ(d, cx, focal_y, focal_r + 2, fill=STEEL_DARK)
    circ(d, cx, focal_y, focal_r, fill=ENERGY_MID)
    circ(d, cx, focal_y, max(2, focal_r - 3), fill=ENERGY_LIGHT)

    # Magnetic collimation coils near exit — full-width bands within the nozzle
    coil_th = max(5, int(6 * scale))
    for frac in [0.85, 0.92]:
        cy_pos = y + int(reflector_h * frac)
        cw = reflector_top_w + (reflector_bot_w - reflector_top_w) * (frac ** 0.6)
        chw = int(cw / 2)
        band_y = cy_pos - coil_th // 2
        for row in range(coil_th):
            t_r = row / max(1, coil_th - 1)
            shade = 0.15 + 0.55 * math.sin(t_r * math.pi)
            r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
            g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
            b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)
            d.rectangle([cx - chw, band_y + row, cx + chw, band_y + row + 1],
                        fill=clamp_color((r, g, b)))
        d.line([(cx - chw + 2, band_y + 1), (cx + chw - 2, band_y + 1)],
               fill=COIL_HIGHLIGHT, width=1)

    # Right edge highlight (stays within reflector bounds)
    n_hl = 30
    for j in range(n_hl):
        t = j / (n_hl - 1)
        t1 = (j + 1) / (n_hl - 1)
        if t1 > 1.0:
            break
        w1 = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t ** 0.6)
        w2 = reflector_top_w + (reflector_bot_w - reflector_top_w) * (t1 ** 0.6)
        py1 = y + int(t * reflector_h)
        py2 = y + int(t1 * reflector_h)
        # Stop before the exit lip
        if py2 >= y + reflector_h:
            break
        d.line([(cx + int(w1 / 2) - 2, py1), (cx + int(w2 / 2) - 2, py2)],
               fill=GAMMA_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Exit lip
    lip_h = max(4, int(6 * scale))
    bhw = int(reflector_bot_w / 2)
    d.rectangle([cx - bhw, y + reflector_h - lip_h,
                 cx + bhw, y + reflector_h],
                fill=GAMMA_DARK, outline=STEEL_VERY_DARK)

    return img


# ================================================================
# Engine registry and main
# ================================================================

ENGINES = {
    "engine_orion_pulse":       ("Orion Pulse Drive", generate_orion),
    "engine_daedalus_s1":       ("Daedalus S1", lambda: generate_daedalus(stage=1)),
    "engine_daedalus_s2":       ("Daedalus S2", lambda: generate_daedalus(stage=2)),
    "engine_zpinch_probe":      ("Z-Pinch Probe", lambda: generate_zpinch(variant="probe")),
    "engine_zpinch_advanced":   ("Z-Pinch Advanced", lambda: generate_zpinch(variant="advanced")),
    "engine_amcat_fusion":      ("AM-Cat Fusion", generate_amcat),
    "engine_am_torch":          ("Antimatter Torch", generate_am_torch),
    "engine_gamma_conversion":  ("Gamma Conversion", generate_gamma_conversion),
}


if __name__ == "__main__":
    out_dir = os.path.join(os.path.dirname(__file__), "..", "..", "data", "sprites", "engines")
    os.makedirs(out_dir, exist_ok=True)

    for engine_id, (name, gen_func) in ENGINES.items():
        print(f"Generating {name}...")
        img = gen_func()
        path = os.path.join(out_dir, f"{engine_id}.png")
        img.save(path)
        print(f"  -> {path}  ({img.size[0]}x{img.size[1]})")

    print(f"\nDone! Generated {len(ENGINES)} engine sprites.")