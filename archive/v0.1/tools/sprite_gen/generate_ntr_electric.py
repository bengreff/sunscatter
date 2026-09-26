#!/usr/bin/env python3
"""
Generate sprites for NTR engines, electric engines, small reactors, H2 tanks, and xenon tanks.

NTR Engines (3):
  Salamander (Small 3x5), Basilisk (Medium 5x7), Wyvern (Large 9x9)
  Nuclear thermal rockets: cylindrical reactor body + bell nozzle below.

Electric Engines (6):
  Ion:  Moth (Tiny 1x2), Cicada (Tiny 1x3) — gridded electrostatic, flat emitter face
  Hall: Tern (Tiny 1x2), Albatross (Small 3x3) — annular discharge ring
  MPD:  Mako (Small 3x4), Orca (Medium 5x5) — central cathode + anode ring + coils

Small Fission Reactors (3):
  Ember (Tiny 1x3), Hearth (Small 3x4), Crucible (Medium 5x5)
  Scaled-down versions of the interstellar fission reactor.

Hydrogen Tanks (4):
  H2 Tiny (1x2), Small (3x4), Medium (5x6), Large (9x8)
  White/cream MLI insulation blanket look.

Xenon Tanks (4):
  Xe Tiny (1x1), Small (3x1), Medium (5x1), Large (9x1)
  High-pressure dark grey/charcoal vessels, very short.

All parts use PX=360 px/grid. Engine sprites use PAD=0, part sprites use PAD=0.
"""

from PIL import Image, ImageDraw
import math
import os

# ================================================================
# Constants
# ================================================================
PX = 360
MAX_SPRITE_PX = 4096
PAD = 0  # No padding — atlas packer adds spacing

def _capped_px(w, h):
    """Return effective PX capped so the output image stays within MAX_SPRITE_PX."""
    return min(PX, MAX_SPRITE_PX // max(w, h))

# ================================================================
# Palette
# ================================================================

# Base steel
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)

# Fission reactor palette (warm industrial)
FISSION_BODY = (70, 68, 62)
FISSION_DARK = (50, 48, 44)
FISSION_LIGHT = (95, 92, 85)
FISSION_HIGHLIGHT = (115, 112, 105)
WARNING_YELLOW = (200, 180, 40)
WARNING_ORANGE = (200, 120, 40)

# Copper coil colors (for magnetic coils)
COIL_DARK = (90, 60, 35)
COIL_MID = (140, 95, 55)
COIL_LIGHT = (180, 130, 75)
COIL_HIGHLIGHT = (210, 165, 100)

# Energy/glow colors
ENERGY_DARK = (35, 65, 85)
ENERGY_MID = (50, 90, 120)
ENERGY_LIGHT = (70, 120, 155)

# Electric engine glow
ION_GLOW = (80, 180, 220)
ION_GLOW_BRIGHT = (140, 220, 255)
HALL_GLOW = (100, 160, 255)
MPD_GLOW = (160, 120, 255)

# Xenon tank
XENON_DARK = (55, 58, 65)
XENON_MID = (75, 78, 88)
XENON_LIGHT = (95, 100, 110)
XENON_HIGHLIGHT = (120, 125, 135)


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


def lerp_color(c1, c2, t):
    """Linearly interpolate between two RGB colors."""
    t = max(0.0, min(1.0, t))
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3))


def clamp_color(c):
    return tuple(max(0, min(255, v)) for v in c)


def bell_w(frac, throat, exit_w, exp=1.8):
    """Bell/nozzle expansion curve."""
    return throat + (exit_w - throat) * (1 - (1 - frac) ** exp)


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


def draw_structural_band(d, cx, y, w, h, color_light, color_dark):
    hw = int(w / 2)
    d.rectangle([cx - hw, y, cx + hw, y + h], fill=color_light)
    d.rectangle([cx - hw, y + h, cx + hw, y + h + 1], fill=color_dark)


# ================================================================
# NTR Engine generators
# ================================================================

def generate_ntr_engine(size="small"):
    """Nuclear Thermal Rocket engine.

    Top ~40% is the reactor section (cylindrical, with control drums,
    neutron reflector bands in yellow/warning stripes, cooling channel
    vertical lines). Below that is a standard bell nozzle.
    """
    sizes = {
        "small":  (3, 5),   # Salamander
        "medium": (5, 7),   # Basilisk
        "large":  (9, 9),   # Wyvern
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)

    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = body_w / 270.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(8, int(body_h * 0.025))
    ring_w = body_w * 0.6
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- REACTOR SECTION (top ~40%) ---
    reactor_h = int(body_h * 0.38)
    reactor_w = body_w * 0.78

    # Shadow shield (radiation shielding cap at top of reactor)
    shield_h = int(reactor_h * 0.18)
    shield_w = reactor_w + 10
    rect(d, cx, y, shield_w, shield_h, FISSION_DARK, outline=STEEL_VERY_DARK)
    # Dense cross-hatching pattern
    shw = int(shield_w / 2)
    hatch_spacing = max(5, int(7 * scale))
    for i in range(0, int(shield_w), hatch_spacing):
        hx = cx - shw + i
        d.line([(hx, y + 2), (hx + hatch_spacing // 2, y + shield_h - 2)],
               fill=(40, 38, 35), width=1)
    # Warning stripe
    stripe_h = max(2, int(3 * scale))
    stripe_y = y + shield_h - stripe_h - 2
    d.rectangle([cx - shw + 3, stripe_y, cx + shw - 3, stripe_y + stripe_h],
                fill=WARNING_YELLOW)
    # Small radiation trefoil hint
    tr = max(2, int(3 * scale))
    circ(d, cx, y + shield_h // 3, tr, fill=WARNING_YELLOW)
    circ(d, cx - tr - 1, y + shield_h // 3 + tr + 1, max(1, tr - 1),
         fill=WARNING_YELLOW)
    circ(d, cx + tr + 1, y + shield_h // 3 + tr + 1, max(1, tr - 1),
         fill=WARNING_YELLOW)
    y += shield_h

    # Control rod cluster
    rod_section_h = int(reactor_h * 0.15)
    rod_w = reactor_w * 0.85
    rect(d, cx, y, rod_w, rod_section_h, FISSION_BODY, outline=FISSION_DARK)
    n_rods = max(2, int(GW * 0.9))
    rod_r = max(2, int(4 * scale))
    rod_spacing = max(rod_r * 3, (rod_w - rod_r * 4) / max(1, n_rods - 1))
    rhw = int(rod_w / 2)
    for i in range(n_rods):
        rx = cx - rhw + rod_r * 2 + int(i * rod_spacing)
        if rx > cx + rhw - rod_r * 2:
            break
        d.rectangle([rx - rod_r // 2, y + 2, rx + rod_r // 2, y + rod_section_h - 2],
                    fill=STEEL_DARK)
        circ(d, rx, y + rod_r + 1, rod_r, fill=STEEL_LIGHT, outline=STEEL_DARK)
        circ(d, rx, y + rod_r + 1, max(1, rod_r - 2), fill=STEEL_MID)
    y += rod_section_h

    # Main reactor pressure vessel
    vessel_h = int(reactor_h * 0.52)
    vessel_w = reactor_w
    rect(d, cx, y, vessel_w, vessel_h, FISSION_BODY, outline=FISSION_DARK)

    # Left edge highlight
    vhw = int(vessel_w / 2)
    d.line([(cx - vhw + 2, y + 2), (cx - vhw + 2, y + vessel_h - 2)],
           fill=FISSION_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Neutron reflector bands (horizontal, with warning stripes)
    n_refl = max(2, int(vessel_h / (40 * scale)))
    for i in range(n_refl):
        frac = (i + 1) / (n_refl + 1)
        by = y + int(vessel_h * frac)
        band_h = max(3, int(5 * scale))
        # Warning stripe band
        d.rectangle([cx - vhw + 1, by - band_h // 2,
                     cx + vhw - 1, by + band_h // 2],
                    fill=WARNING_ORANGE)
        # Overlay thin yellow stripe in center
        d.rectangle([cx - vhw + 3, by - 1, cx + vhw - 3, by + 1],
                    fill=WARNING_YELLOW)

    # Cooling channel vertical lines
    n_channels = max(2, int(vessel_w / (18 * scale)))
    for i in range(n_channels):
        px = cx - vhw + int((i + 0.5) * vessel_w / n_channels)
        d.line([(px, y + 3), (px, y + vessel_h - 3)], fill=FISSION_DARK, width=1)

    # Control drum indicators (circles along sides)
    drum_r = max(2, int(4 * scale))
    n_drums = max(2, int(GW * 0.6))
    for i in range(n_drums):
        frac = (i + 0.5) / n_drums
        dy = y + int(vessel_h * frac)
        for s in [-1, 1]:
            dx = cx + s * (vhw - drum_r - 3)
            circ(d, dx, dy, drum_r, fill=STEEL_LIGHT, outline=FISSION_DARK)
            circ(d, dx, dy, max(1, drum_r - 2), fill=WARNING_ORANGE)

    # Central inspection viewport
    vp_r = max(4, int(7 * scale))
    vp_y = y + vessel_h // 2
    circ(d, cx, vp_y, vp_r + 2, fill=FISSION_DARK)
    circ(d, cx, vp_y, vp_r, fill=(60, 50, 35))
    circ(d, cx, vp_y, max(2, vp_r - 2), fill=(80, 65, 40))
    circ(d, cx - 1, vp_y - 1, max(1, vp_r // 3), fill=(120, 80, 30))
    y += vessel_h

    # Heat exchanger section (transition from reactor to nozzle)
    hex_h = int(reactor_h * 0.15)
    hex_w = vessel_w + 8
    rect(d, cx, y, hex_w, hex_h, STEEL_MID, outline=STEEL_DARK)
    hhw = int(hex_w / 2)
    n_fins = max(4, int(hex_w / (10 * scale)))
    for i in range(n_fins):
        fx = cx - hhw + int((i + 1) * hex_w / (n_fins + 1))
        t = i / max(1, n_fins - 1)
        fin_color = lerp_color(WARNING_ORANGE, (180, 60, 30), t)
        d.line([(fx, y + 2), (fx, y + hex_h - 2)], fill=fin_color, width=max(1, int(2 * scale)))
    y += hex_h

    # --- NOZZLE SECTION (bottom ~60%) ---
    nozzle_h = body_h - (y - PAD)
    throat_w = body_w * 0.25
    exit_w = body_w * 0.88

    # Converging section (from reactor width to throat)
    conv_h = int(nozzle_h * 0.12)
    trap(d, cx, y, vessel_w, throat_w, conv_h, STEEL_MID, outline=STEEL_DARK)
    y += conv_h

    # Throat ring
    throat_ring_h = max(4, int(6 * scale))
    thw = int(throat_w / 2)
    d.rectangle([cx - thw - 4, y, cx + thw + 4, y + throat_ring_h],
                fill=STEEL_LIGHT, outline=STEEL_DARK)
    d.line([(cx - thw - 3, y + 1), (cx + thw + 3, y + 1)],
           fill=STEEL_HIGHLIGHT, width=1)
    y += throat_ring_h

    # Bell nozzle (diverging section)
    bell_h = nozzle_h - conv_h - throat_ring_h
    n_strips = max(30, int(bell_h / 3))
    sh = bell_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        w1 = bell_w(t, throat_w, exit_w, exp=1.6)
        w2 = bell_w(t1, throat_w, exit_w, exp=1.6)
        sy = y + t * bell_h
        # Darken toward exit
        shade = 0.6 - 0.25 * t
        c = lerp_color(STEEL_DARK, STEEL_MID, shade)
        trap(d, cx, int(sy), w1, w2, int(sh) + 1, fill=c)

    # Interior void
    wall_th = max(6, int(10 * scale))
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips
        w1 = bell_w(t, throat_w, exit_w, exp=1.6)
        w2 = bell_w(t1, throat_w, exit_w, exp=1.6)
        iw1 = max(0, w1 - wall_th * 2)
        iw2 = max(0, w2 - wall_th * 2)
        sy = y + t * bell_h
        if iw1 > 2:
            trap(d, cx, int(sy), iw1, iw2, int(sh) + 1, fill=INTERIOR)

    # Left edge highlight on bell
    for j in range(min(40, n_strips)):
        t = j / min(40, n_strips)
        w_at = bell_w(t, throat_w, exit_w, exp=1.6)
        py = y + int(t * bell_h)
        px = cx - int(w_at / 2) + 2
        d.point((px, py), fill=STEEL_LIGHT)
        d.point((px, py + 1), fill=STEEL_LIGHT)

    # Structural ribs on the bell
    n_ribs = max(2, int(exit_w / (50 * scale)))
    rib_w = max(1, int(2 * scale))
    for i in range(n_ribs):
        pfrac = (i + 1) / (n_ribs + 1)
        for j in range(n_strips):
            t = j / n_strips
            w_at = bell_w(t, throat_w, exit_w, exp=1.6)
            px = cx - int(w_at / 2) + int(w_at * pfrac)
            py = y + int(t * bell_h)
            d.point((px, py), fill=STEEL_DARK)

    # Exit lip
    lip_h = max(3, int(4 * scale))
    ehw = int(exit_w / 2)
    d.rectangle([cx - ehw, y + bell_h - lip_h, cx + ehw, y + bell_h],
                fill=STEEL_DARK, outline=STEEL_VERY_DARK)

    return img


# ================================================================
# Electric Engine generators
# ================================================================

def generate_ion_engine(size="tiny_short"):
    """Ion engine (gridded electrostatic).

    Flat gridded emitter face at bottom with cyan/blue glow.
    Cylindrical body, NO bell nozzle. The bottom face has a grid pattern.
    Top has a simple mount ring.
    """
    sizes = {
        "tiny_short": (1, 2),  # Moth
        "tiny_tall":  (1, 3),  # Cicada
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)

    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = max(0.4, body_w / 270.0)
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(6, int(body_h * 0.04))
    ring_w = body_w * 0.65
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- POWER CONNECTOR ---
    conn_h = max(4, int(body_h * 0.04))
    conn_w = ring_w - 4
    rect(d, cx, y, conn_w, conn_h, ENERGY_MID, outline=ENERGY_DARK)
    # Terminal dots
    n_terms = max(2, int(conn_w / (15 * scale)))
    chw = int(conn_w / 2)
    for i in range(n_terms):
        tx = cx - chw + int((i + 0.5) * conn_w / n_terms)
        circ(d, tx, y + conn_h // 2, max(1, int(2 * scale)), fill=ENERGY_LIGHT)
    y += conn_h

    # --- MAIN CYLINDRICAL BODY ---
    cyl_h = int(body_h * 0.60)
    cyl_w = body_w * 0.80
    rect(d, cx, y, cyl_w, cyl_h, STEEL_MID, outline=STEEL_DARK)

    # Left edge highlight
    cyl_hw = int(cyl_w / 2)
    d.line([(cx - cyl_hw + 2, y + 2), (cx - cyl_hw + 2, y + cyl_h - 2)],
           fill=STEEL_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Horizontal structural bands
    n_bands = max(2, int(cyl_h / (30 * scale)))
    for i in range(n_bands):
        frac = (i + 1) / (n_bands + 1)
        by = y + int(cyl_h * frac)
        draw_structural_band(d, cx, by, cyl_w + 4, max(2, int(3 * scale)),
                             STEEL_LIGHT, STEEL_DARK)

    # Vertical panel lines
    n_panels = max(1, int(cyl_w / (30 * scale)))
    for i in range(n_panels):
        px = cx - cyl_hw + int((i + 1) * cyl_w / (n_panels + 1))
        d.line([(px, y + 3), (px, y + cyl_h - 3)], fill=STEEL_DARK, width=1)

    # Internal ionization chamber indicator (blue viewport)
    if cyl_h > 40:
        vp_r = max(3, int(5 * scale))
        vp_y = y + cyl_h // 2
        circ(d, cx, vp_y, vp_r + 1, fill=STEEL_DARK)
        circ(d, cx, vp_y, vp_r, fill=ION_GLOW)
        circ(d, cx - 1, vp_y - 1, max(1, vp_r // 3), fill=ION_GLOW_BRIGHT)
    y += cyl_h

    # --- ACCELERATION GRID HOUSING ---
    grid_housing_h = int(body_h * 0.10)
    grid_w = cyl_w + 6
    rect(d, cx, y, grid_w, grid_housing_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    # Left/right edge highlights
    ghw = int(grid_w / 2)
    d.line([(cx - ghw + 2, y + 1), (cx + ghw - 2, y + 1)],
           fill=STEEL_LIGHT, width=1)
    y += grid_housing_h

    # --- GRIDDED EMITTER FACE ---
    emitter_h = int(body_h * 0.22)
    emitter_w = cyl_w
    ehw = int(emitter_w / 2)

    # Background glow
    rect(d, cx, y, emitter_w, emitter_h, (25, 50, 65))

    # Grid lines (vertical parallel lines — the gridded ion optics)
    n_grid_lines = max(3, int(emitter_w / (6 * scale)))
    grid_spacing = emitter_w / (n_grid_lines + 1)
    for i in range(n_grid_lines):
        gx = cx - ehw + int((i + 1) * grid_spacing)
        # Alternating screen/accel grids
        if i % 2 == 0:
            d.line([(gx, y + 2), (gx, y + emitter_h - 2)],
                   fill=STEEL_MID, width=max(1, int(2 * scale)))
        else:
            d.line([(gx, y + 2), (gx, y + emitter_h - 2)],
                   fill=STEEL_DARK, width=1)

    # Horizontal grid lines
    n_hgrid = max(2, int(emitter_h / (8 * scale)))
    for i in range(n_hgrid):
        gy = y + int((i + 1) * emitter_h / (n_hgrid + 1))
        d.line([(cx - ehw + 2, gy), (cx + ehw - 2, gy)],
               fill=STEEL_MID, width=1)

    # Ion glow between grid lines
    for i in range(n_grid_lines - 1):
        gx1 = cx - ehw + int((i + 1) * grid_spacing)
        gx2 = cx - ehw + int((i + 2) * grid_spacing)
        mid_x = (gx1 + gx2) // 2
        glow_y = y + emitter_h - max(3, int(4 * scale))
        circ(d, mid_x, glow_y, max(1, int(2 * scale)), fill=ION_GLOW)

    # Bottom edge glow line
    d.line([(cx - ehw + 2, y + emitter_h - 1), (cx + ehw - 2, y + emitter_h - 1)],
           fill=ION_GLOW, width=max(2, int(3 * scale)))
    d.line([(cx - ehw + 4, y + emitter_h - 2), (cx + ehw - 4, y + emitter_h - 2)],
           fill=ION_GLOW_BRIGHT, width=1)

    # Outline
    d.rectangle([cx - ehw, y, cx + ehw, y + emitter_h], outline=STEEL_DARK)

    return img


def generate_hall_engine(size="tiny"):
    """Hall-effect thruster.

    Annular discharge channel — the bottom exit is a ring/donut shape
    (dark center, bright ring around it). Compact cylindrical body.
    """
    sizes = {
        "tiny":  (1, 2),  # Tern
        "small": (3, 3),  # Albatross
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)

    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = max(0.4, body_w / 270.0)
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(6, int(body_h * 0.04))
    ring_w = body_w * 0.55
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- POWER/PROPELLANT HEADER ---
    header_h = max(4, int(body_h * 0.06))
    header_w = ring_w + 6
    rect(d, cx, y, header_w, header_h, ENERGY_MID, outline=ENERGY_DARK)
    n_terms = max(2, int(header_w / (15 * scale)))
    hhw = int(header_w / 2)
    for i in range(n_terms):
        tx = cx - hhw + int((i + 0.5) * header_w / n_terms)
        circ(d, tx, y + header_h // 2, max(1, int(2 * scale)), fill=ENERGY_LIGHT)
    y += header_h

    # --- MAIN CYLINDRICAL BODY ---
    cyl_h = int(body_h * 0.50)
    cyl_w = body_w * 0.82
    rect(d, cx, y, cyl_w, cyl_h, STEEL_MID, outline=STEEL_DARK)

    cyl_hw = int(cyl_w / 2)
    # Left edge highlight
    d.line([(cx - cyl_hw + 2, y + 2), (cx - cyl_hw + 2, y + cyl_h - 2)],
           fill=STEEL_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Structural bands
    n_bands = max(2, int(cyl_h / (30 * scale)))
    for i in range(n_bands):
        frac = (i + 1) / (n_bands + 1)
        by = y + int(cyl_h * frac)
        draw_structural_band(d, cx, by, cyl_w + 4, max(2, int(3 * scale)),
                             STEEL_LIGHT, STEEL_DARK)

    # Magnetic circuit coil wrapping
    n_coils = max(1, int(cyl_h / (25 * scale)))
    coil_th = max(2, int(3 * scale))
    for i in range(n_coils):
        frac = (i + 0.5) / n_coils
        cy_pos = y + int(cyl_h * frac)
        for row in range(coil_th):
            t_r = row / max(1, coil_th - 1)
            shade = 0.3 + 0.5 * math.sin(t_r * math.pi)
            r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
            g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
            b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)
            ry = cy_pos - coil_th // 2 + row
            d.rectangle([cx - cyl_hw - 2, ry, cx + cyl_hw + 2, ry + 1],
                        fill=clamp_color((r, g, b)))
    y += cyl_h

    # --- ANNULAR DISCHARGE SECTION ---
    ann_h = int(body_h * 0.40)
    ann_w = cyl_w + 4

    # Outer ring body
    rect(d, cx, y, ann_w, ann_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    ann_hw = int(ann_w / 2)

    # The annular channel: a ring shape at the exit face
    # Inner wall (dark center)
    inner_r = max(4, int(ann_w * 0.18))
    channel_w = max(3, int(ann_w * 0.12))

    # Draw row-by-row to create the annular appearance
    for row in range(ann_h):
        py = y + row
        row_frac = row / max(1, ann_h - 1)
        # The ring becomes more visible toward the bottom (exit)
        ring_visibility = row_frac ** 0.5

        # Dark interior fill
        interior_shade = 0.1 + 0.05 * ring_visibility
        interior_c = lerp_color(INTERIOR, STEEL_VERY_DARK, interior_shade)

        # Outer wall region
        for col in range(int(ann_w)):
            px = cx - ann_hw + col
            # Distance from center (normalized)
            dist = abs(col - ann_w / 2) / (ann_w / 2)

            if dist > 0.85:
                # Outer wall
                d.point((px, py), fill=STEEL_MID)
            elif dist > 0.55:
                # Annular discharge channel
                glow_intensity = ring_visibility * (0.3 + 0.7 * (1.0 - abs(dist - 0.7) / 0.15))
                c = lerp_color(interior_c, HALL_GLOW, glow_intensity * 0.4)
                d.point((px, py), fill=c)
            elif dist > 0.35:
                # Inner wall (ceramic insulator)
                d.point((px, py), fill=lerp_color(STEEL_VERY_DARK, STEEL_DARK, ring_visibility * 0.3))
            else:
                # Dark center (cathode region)
                d.point((px, py), fill=interior_c)

    # Bottom glow ring (the bright discharge ring)
    ring_outer_r = int(ann_w * 0.35)
    ring_inner_r = int(ann_w * 0.22)
    ring_y = y + ann_h - max(4, int(6 * scale))

    # Draw the annular glow
    for angle in range(360):
        rad = math.radians(angle)
        for r in range(ring_inner_r, ring_outer_r + 1):
            gx = cx + int(r * math.cos(rad))
            gy = ring_y + int(r * math.sin(rad) * 0.3)  # Flatten for perspective
            if 0 <= gx < img_w and 0 <= gy < img_h:
                dist_from_mid = abs(r - (ring_inner_r + ring_outer_r) / 2)
                max_dist = (ring_outer_r - ring_inner_r) / 2
                brightness = 1.0 - (dist_from_mid / max(1, max_dist))
                c = lerp_color(ENERGY_DARK, HALL_GLOW, brightness * 0.8)
                d.point((gx, gy), fill=c)

    # Bottom edge glow
    d.line([(cx - ann_hw + 3, y + ann_h - 1), (cx + ann_hw - 3, y + ann_h - 1)],
           fill=HALL_GLOW, width=max(1, int(2 * scale)))

    # Outline
    d.rectangle([cx - ann_hw, y, cx + ann_hw, y + ann_h], outline=STEEL_VERY_DARK)

    return img


def generate_mpd_engine(size="small"):
    """MPD (magnetoplasmadynamic) thruster.

    Central cathode spike protruding from the bottom, surrounded by an
    anode ring. Visible magnetic coil bands around the body. Larger and
    more industrial than ion/hall engines.
    """
    sizes = {
        "small":  (3, 4),  # Mako
        "medium": (5, 5),  # Orca
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)

    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = max(0.5, body_w / 270.0)
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(8, int(body_h * 0.03))
    ring_w = body_w * 0.55
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- POWER CONDITIONING ---
    pcu_h = max(6, int(body_h * 0.06))
    pcu_w = ring_w + 10
    rect(d, cx, y, pcu_w, pcu_h, ENERGY_MID, outline=ENERGY_DARK)
    pcu_hw = int(pcu_w / 2)
    n_terms = max(2, int(pcu_w / (20 * scale)))
    for i in range(n_terms):
        tx = cx - pcu_hw + int((i + 0.5) * pcu_w / n_terms)
        circ(d, tx, y + pcu_h // 2, max(1, int(2 * scale)), fill=ENERGY_LIGHT)
    y += pcu_h

    # --- SOLENOID BODY (cylindrical with magnetic coils) ---
    sol_h = int(body_h * 0.52)
    sol_w = body_w * 0.82
    sol_hw = int(sol_w / 2)

    # Main body cylinder
    rect(d, cx, y, sol_w, sol_h, STEEL_MID, outline=STEEL_DARK)

    # Left edge highlight
    d.line([(cx - sol_hw + 2, y + 2), (cx - sol_hw + 2, y + sol_h - 2)],
           fill=STEEL_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Vertical panel lines
    n_panels = max(1, int(sol_w / (40 * scale)))
    for i in range(n_panels):
        px = cx - sol_hw + int((i + 1) * sol_w / (n_panels + 1))
        d.line([(px, y + 3), (px, y + sol_h - 3)], fill=STEEL_DARK, width=1)

    # Magnetic coil bands (copper, 3D shaded)
    n_coils = max(3, int(sol_h / (20 * scale)))
    coil_th = max(4, int(6 * scale))
    coil_spacing = sol_h / (n_coils + 1)
    protrude = max(3, int(5 * scale))
    for i in range(n_coils):
        cy_pos = y + int((i + 1) * coil_spacing)
        band_y = cy_pos - coil_th // 2
        for row in range(coil_th):
            t_r = row / max(1, coil_th - 1)
            roundness = math.sin(t_r * math.pi)
            top_bias = max(0, 1.0 - t_r * 1.6)
            shade = 0.15 + 0.55 * roundness + 0.30 * top_bias
            shade = max(0.0, min(1.0, shade))
            r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
            g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
            b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)
            ry = band_y + row
            d.rectangle([cx - sol_hw - protrude, ry,
                         cx + sol_hw + protrude, ry + 1],
                        fill=clamp_color((r, g, b)))
        # Specular highlight
        d.line([(cx - sol_hw - protrude + 2, band_y),
                (cx + sol_hw + protrude - 2, band_y)],
               fill=COIL_HIGHLIGHT, width=1)
        # Bottom shadow
        d.line([(cx - sol_hw - protrude + 2, band_y + coil_th - 1),
                (cx + sol_hw + protrude - 2, band_y + coil_th - 1)],
               fill=(30, 25, 18), width=1)

        # Winding marks
        wind_color = clamp_color((COIL_LIGHT[0] + 15,
                                  COIL_LIGHT[1] + 15,
                                  COIL_LIGHT[2] + 10))
        full_span = (sol_hw + protrude) * 2
        n_winds = max(4, int(full_span / max(3, int(5 * scale))))
        wind_spacing = max(2, full_span // max(1, n_winds))
        for wi in range(n_winds):
            wx = cx - sol_hw - protrude + 3 + wi * wind_spacing
            if wx < cx + sol_hw + protrude - 3:
                d.line([(wx, band_y + 2),
                        (wx + max(1, coil_th // 5), band_y + coil_th - 2)],
                       fill=wind_color, width=1)
    y += sol_h

    # --- ANODE RING (wide band at transition) ---
    anode_h = max(6, int(body_h * 0.06))
    anode_w = sol_w + 12
    rect(d, cx, y, anode_w, anode_h, STEEL_LIGHT, outline=STEEL_DARK)
    # Glow tint on the anode ring
    ahw = int(anode_w / 2)
    for row in range(anode_h):
        t = row / max(1, anode_h - 1)
        shade = math.sin(t * math.pi)
        glow_c = lerp_color(STEEL_LIGHT, MPD_GLOW, shade * 0.2)
        d.rectangle([cx - ahw + 1, y + row, cx + ahw - 1, y + row + 1],
                    fill=glow_c)
    d.rectangle([cx - ahw, y, cx + ahw, y + anode_h], outline=STEEL_DARK)
    y += anode_h

    # --- EXIT SECTION with central cathode ---
    exit_h = int(body_h * 0.33)
    exit_w = sol_w - 10

    # Tapered exit body
    exit_top_w = exit_w
    exit_bot_w = exit_w * 0.7
    trap(d, cx, y, exit_top_w, exit_bot_w, exit_h, STEEL_DARK, outline=STEEL_VERY_DARK)

    # Interior void (dark)
    inner_top = exit_top_w * 0.6
    inner_bot = exit_bot_w * 0.5
    trap(d, cx, y + 2, inner_top, inner_bot, exit_h - 4, INTERIOR)

    # Central cathode spike (protruding below the exit)
    cathode_w = max(4, int(body_w * 0.06))
    cathode_h = int(exit_h * 0.85)
    cathode_y = y + int(exit_h * 0.10)

    # Cathode rod
    d.rectangle([cx - cathode_w // 2, cathode_y,
                 cx + cathode_w // 2, cathode_y + cathode_h],
                fill=STEEL_HIGHLIGHT, outline=STEEL_MID)
    # Cathode tip (pointed)
    tip_h = max(6, int(cathode_h * 0.2))
    d.polygon([(cx - cathode_w // 2, cathode_y + cathode_h),
               (cx + cathode_w // 2, cathode_y + cathode_h),
               (cx, cathode_y + cathode_h + tip_h)],
              fill=STEEL_LIGHT)
    # Cathode glow
    circ(d, cx, cathode_y + cathode_h + tip_h, max(2, int(3 * scale)),
         fill=MPD_GLOW)

    # Bottom glow ring around cathode
    glow_y = y + exit_h - 2
    bot_hw = int(exit_bot_w / 2)
    d.line([(cx - bot_hw + 2, glow_y), (cx + bot_hw - 2, glow_y)],
           fill=MPD_GLOW, width=max(2, int(3 * scale)))

    return img


# ================================================================
# Small Fission Reactor generators
# ================================================================

def generate_small_reactor(size="tiny"):
    """Small fission reactor: scaled-down industrial pressure vessel.

    Same visual language as the interstellar fission reactors but much smaller.
    Control rods, pressure vessel, heat exchanger, warning stripes.
    """
    sizes = {
        "tiny":   (1, 3),  # Ember
        "small":  (3, 5),  # Hearth
        "medium": (5, 5),  # Crucible
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)

    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = max(0.35, body_w / 270.0)
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(6, int(body_h * 0.03))
    ring_w = body_w * 0.60
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- SHADOW SHIELD (top radiation cap) ---
    shield_h = max(8, int(body_h * 0.08))
    shield_w = ring_w + 8
    rect(d, cx, y, shield_w, shield_h, FISSION_DARK, outline=STEEL_VERY_DARK)
    shw = int(shield_w / 2)
    # Cross-hatching
    hatch_spacing = max(4, int(6 * scale))
    for i in range(0, int(shield_w), hatch_spacing):
        hx = cx - shw + i
        d.line([(hx, y + 1), (hx + hatch_spacing // 2, y + shield_h - 1)],
               fill=(40, 38, 35), width=1)
    # Warning stripe
    stripe_h = max(2, int(3 * scale))
    d.rectangle([cx - shw + 2, y + shield_h - stripe_h - 1,
                 cx + shw - 2, y + shield_h - 1],
                fill=WARNING_YELLOW)
    # Radiation trefoil hint (only if big enough)
    if shield_w > 40:
        tr = max(2, int(3 * scale))
        circ(d, cx, y + shield_h // 3, tr, fill=WARNING_YELLOW)
    y += shield_h

    # --- CONTROL ROD SECTION ---
    rod_h = max(8, int(body_h * 0.06))
    rod_w = body_w * 0.72
    rect(d, cx, y, rod_w, rod_h, FISSION_BODY, outline=FISSION_DARK)
    n_rods = max(2, int(GW * 0.8))
    rod_r = max(2, int(3 * scale))
    rhw = int(rod_w / 2)
    for i in range(n_rods):
        rx = cx - rhw + int((i + 0.5) * rod_w / n_rods)
        d.rectangle([rx - 1, y + 1, rx + 1, y + rod_h - 1], fill=STEEL_DARK)
        circ(d, rx, y + rod_r + 1, rod_r, fill=STEEL_LIGHT, outline=STEEL_DARK)
    y += rod_h

    # --- MAIN PRESSURE VESSEL ---
    # Compute bottom sections first so vessel fills remaining space
    hex_h = max(8, int(body_h * 0.10))
    bot_mount_h = max(6, int(body_h * 0.025))
    vessel_h = body_h - (y - PAD) - hex_h - bot_mount_h
    vessel_w = body_w * 0.82
    rect(d, cx, y, vessel_w, vessel_h, FISSION_BODY, outline=FISSION_DARK)

    vhw = int(vessel_w / 2)
    # Left edge highlight
    d.line([(cx - vhw + 2, y + 2), (cx - vhw + 2, y + vessel_h - 2)],
           fill=FISSION_HIGHLIGHT, width=max(1, int(2 * scale)))

    # Horizontal structural bands
    n_bands = max(2, int(vessel_h / (35 * scale)))
    for i in range(n_bands):
        frac = (i + 1) / (n_bands + 1)
        by = y + int(vessel_h * frac)
        draw_structural_band(d, cx, by, vessel_w + 4, max(2, int(3 * scale)),
                             FISSION_LIGHT, FISSION_DARK)

    # Vertical panel lines
    n_panels = max(1, int(vessel_w / (40 * scale)))
    for i in range(n_panels):
        px = cx - vhw + int((i + 1) * vessel_w / (n_panels + 1))
        d.line([(px, y + 3), (px, y + vessel_h - 3)], fill=FISSION_DARK, width=1)

    # Coolant pipe nubs on sides
    nub_len = max(5, int(8 * scale))
    nub_h = max(3, int(4 * scale))
    for s in [-1, 1]:
        for frac in [0.3, 0.7]:
            ny = y + int(vessel_h * frac)
            nx = cx + s * vhw
            nx2 = nx + s * nub_len
            d.rectangle([min(nx, nx2), ny - nub_h // 2,
                         max(nx, nx2), ny + nub_h // 2],
                        fill=STEEL_MID, outline=STEEL_DARK)
            circ(d, nx2, ny, max(1, nub_h // 3), fill=STEEL_LIGHT)

    # Central viewport
    if vessel_w > 40:
        vp_r = max(3, int(5 * scale))
        vp_y = y + vessel_h // 2
        circ(d, cx, vp_y, vp_r + 1, fill=FISSION_DARK)
        circ(d, cx, vp_y, vp_r, fill=(60, 50, 35))
        circ(d, cx, vp_y, max(2, vp_r - 2), fill=(80, 65, 40))
        circ(d, cx - 1, vp_y - 1, max(1, vp_r // 3), fill=(120, 80, 30))
    y += vessel_h

    # --- HEAT EXCHANGER ---
    hex_w = vessel_w + 6
    rect(d, cx, y, hex_w, hex_h, STEEL_MID, outline=STEEL_DARK)
    hhw = int(hex_w / 2)
    n_fins = max(3, int(hex_w / (8 * scale)))
    for i in range(n_fins):
        fx = cx - hhw + int((i + 1) * hex_w / (n_fins + 1))
        t = i / max(1, n_fins - 1)
        fin_color = lerp_color(WARNING_ORANGE, (180, 60, 30), t)
        d.line([(fx, y + 2), (fx, y + hex_h - 2)], fill=fin_color, width=max(1, int(2 * scale)))
    y += hex_h

    # --- BOTTOM MOUNT ---
    draw_mount_ring(d, cx, y, mount_w, bot_mount_h, scale=scale)

    return img


# ================================================================
# Xenon Tank generators
# ================================================================

def generate_xenon_tank(size="tiny"):
    """Xenon tank: high-pressure vessel.

    Dark grey/charcoal palette. Short (1 grid tall) but varying width.
    Pressure gauge indicators and fill port details.
    """
    sizes = {
        "tiny":   (1, 1),
        "small":  (3, 1),
        "medium": (5, 1),
        "large":  (9, 1),
    }
    GW, GH = sizes[size]
    PX = _capped_px(GW, GH)

    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    x0, y0 = PAD, PAD
    w, h = int(GW * PX), int(GH * PX)
    x1, y1 = x0 + w, y0 + h
    cx = img_w // 2
    cy = img_h // 2
    scale = max(0.35, w / 270.0)

    # Main body — dark charcoal
    d.rectangle([x0, y0, x1, y1], fill=XENON_MID)

    # Top and bottom edge bands (reinforced rims)
    rim_h = max(3, h // 5)
    d.rectangle([x0, y0, x1, y0 + rim_h], fill=XENON_LIGHT)
    d.rectangle([x0, y1 - rim_h, x1, y1], fill=XENON_LIGHT)
    d.line([(x0, y0 + rim_h), (x1, y0 + rim_h)], fill=XENON_DARK, width=1)
    d.line([(x0, y1 - rim_h), (x1, y1 - rim_h)], fill=XENON_DARK, width=1)

    # Rounded end caps (left/right) for pressure vessel appearance
    end_inset = max(2, int(w * 0.02))
    # Left dome effect
    for i in range(min(h, max(3, int(h * 0.3)))):
        for col in range(end_inset):
            frac = col / max(1, end_inset)
            c = lerp_color(XENON_DARK, XENON_MID, frac)
            d.point((x0 + col, y0 + rim_h + i), fill=c)
            d.point((x1 - col, y0 + rim_h + i), fill=c)

    # Bolts along top and bottom rims
    bolt_r = max(1, min(2, h // 10))
    n_bolts = max(2, int(GW * 1.2))
    for i in range(n_bolts):
        bx = x0 + bolt_r * 2 + i * int(max(1, (w - bolt_r * 4) / max(1, n_bolts - 1)))
        circ(d, bx, y0 + rim_h // 2, bolt_r, fill=XENON_HIGHLIGHT)
        circ(d, bx, y0 + rim_h // 2, max(1, bolt_r - 1), fill=XENON_DARK)
        circ(d, bx, y1 - rim_h // 2, bolt_r, fill=XENON_HIGHLIGHT)
        circ(d, bx, y1 - rim_h // 2, max(1, bolt_r - 1), fill=XENON_DARK)

    # Fill port (center top)
    port_r = max(2, int(3 * scale))
    circ(d, cx, y0 + rim_h // 2, port_r + 1, fill=STEEL_MID)
    circ(d, cx, y0 + rim_h // 2, port_r, fill=STEEL_DARK)

    # Pressure gauge indicator (small circle with green indicator)
    if w > 60:
        gauge_r = max(3, int(4 * scale))
        gauge_x = cx + int(w * 0.2)
        circ(d, gauge_x, cy, gauge_r + 1, fill=XENON_DARK)
        circ(d, gauge_x, cy, gauge_r, fill=(30, 35, 40))
        # Gauge needle (tiny line)
        d.line([(gauge_x, cy), (gauge_x + gauge_r - 1, cy - 1)],
               fill=(40, 180, 40), width=1)
        # Gauge highlight
        circ(d, gauge_x - 1, cy - 1, max(1, gauge_r // 3), fill=(50, 55, 60))

    # "Xe" marking indicator (simple colored band)
    if w > 100:
        mark_w = max(10, int(w * 0.12))
        mark_h = max(3, h // 4)
        mark_x = cx - int(w * 0.25)
        d.rectangle([mark_x - mark_w // 2, cy - mark_h // 2,
                     mark_x + mark_w // 2, cy + mark_h // 2],
                    fill=(80, 70, 130))  # Purple-tint for xenon
        d.rectangle([mark_x - mark_w // 2, cy - mark_h // 2,
                     mark_x + mark_w // 2, cy + mark_h // 2],
                    outline=XENON_DARK)

    # Structural weld line through middle
    d.line([(x0 + 2, cy), (x1 - 2, cy)], fill=XENON_DARK, width=1)

    # Outline
    d.rectangle([x0, y0, x1, y1], outline=XENON_DARK)

    return img


# ================================================================
# Registry and main
# ================================================================

PARTS = {
    # NTR Engines -> engines/
    "engine_ntr_salamander": ("NTR Salamander (Small)",
                              lambda: generate_ntr_engine("small"),
                              "engines"),
    "engine_ntr_basilisk":   ("NTR Basilisk (Medium)",
                              lambda: generate_ntr_engine("medium"),
                              "engines"),
    "engine_ntr_wyvern":     ("NTR Wyvern (Large)",
                              lambda: generate_ntr_engine("large"),
                              "engines"),

    # Ion Engines -> engines/
    "engine_ion_moth":       ("Ion Moth (Tiny 1x2)",
                              lambda: generate_ion_engine("tiny_short"),
                              "engines"),
    "engine_ion_cicada":     ("Ion Cicada (Tiny 1x3)",
                              lambda: generate_ion_engine("tiny_tall"),
                              "engines"),

    # Hall-Effect Thrusters -> engines/
    "engine_hall_tern":      ("Hall Tern (Tiny 1x2)",
                              lambda: generate_hall_engine("tiny"),
                              "engines"),
    "engine_hall_albatross": ("Hall Albatross (Small 3x3)",
                              lambda: generate_hall_engine("small"),
                              "engines"),

    # MPD Thrusters -> engines/
    "engine_mpd_mako":       ("MPD Mako (Small 3x4)",
                              lambda: generate_mpd_engine("small"),
                              "engines"),
    "engine_mpd_orca":       ("MPD Orca (Medium 5x5)",
                              lambda: generate_mpd_engine("medium"),
                              "engines"),

    # Small Fission Reactors -> parts/
    "reactor_ember":         ("Reactor Ember (Tiny 1x3)",
                              lambda: generate_small_reactor("tiny"),
                              "parts"),
    "reactor_hearth":        ("Reactor Hearth (Small 3x5)",
                              lambda: generate_small_reactor("small"),
                              "parts"),
    "reactor_crucible":      ("Reactor Crucible (Medium 5x5)",
                              lambda: generate_small_reactor("medium"),
                              "parts"),

    # Xenon Tanks -> parts/
    "tank_xe_tiny":          ("Xe Tank Tiny (1x1)",
                              lambda: generate_xenon_tank("tiny"),
                              "parts"),
    "tank_xe_small":         ("Xe Tank Small (3x1)",
                              lambda: generate_xenon_tank("small"),
                              "parts"),
    "tank_xe_medium":        ("Xe Tank Medium (5x1)",
                              lambda: generate_xenon_tank("medium"),
                              "parts"),
    "tank_xe_large":         ("Xe Tank Large (9x1)",
                              lambda: generate_xenon_tank("large"),
                              "parts"),
}


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    engines_dir = os.path.join(project_root, "data", "sprites", "engines")
    parts_dir = os.path.join(project_root, "data", "sprites", "parts")
    os.makedirs(engines_dir, exist_ok=True)
    os.makedirs(parts_dir, exist_ok=True)

    for part_id, (name, gen_func, subdir) in PARTS.items():
        print(f"Generating {name}...")
        img = gen_func()
        if subdir == "engines":
            path = os.path.join(engines_dir, f"{part_id}.png")
        else:
            path = os.path.join(parts_dir, f"{part_id}.png")
        img.save(path)
        print(f"  -> {path}  ({img.size[0]}x{img.size[1]})")

    print(f"\nDone! Generated {len(PARTS)} sprites.")
