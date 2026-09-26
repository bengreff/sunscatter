#!/usr/bin/env python3
"""
Generate sprites for reactors and particle deflection shields.

Reactors (6 variants):
  Fission Small/Large  — gas-core nuclear, control rods, shadow shield
  Fusion Small/Large   — D+He-3 magnetic confinement, toroidal coils
  Antimatter Small/Large — Penning traps, annihilation chamber

Shields (9 variants):
  Passive Whipple Small/Medium/Large — multi-layer physical barrier
  Active FRES Small/Medium/Large     — electromagnetic coil arrays
  Geodesic Deflector Small/Medium/Large — exotic-matter toroidal ring

Visual palette matches generate_interstellar.py. All parts use PX=360 px/grid.
"""

from PIL import Image, ImageDraw
import math
import os

# ================================================================
# Constants
# ================================================================
PX = 360
MAX_SPRITE_PX = 4096  # cap output image size to avoid multi-GB RGBA decode
PAD = 0  # No padding — atlas packer adds spacing

def _capped_px(w, h):
    """Return effective PX capped so the output image stays within MAX_SPRITE_PX."""
    return min(PX, MAX_SPRITE_PX // max(w, h))

# ================================================================
# Palette (shared with interstellar engine generator)
# ================================================================

STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)

COIL_DARK = (90, 60, 35)
COIL_MID = (140, 95, 55)
COIL_LIGHT = (180, 130, 75)
COIL_HIGHLIGHT = (210, 165, 100)

REACTOR_DARK = (40, 50, 65)
REACTOR_MID = (55, 70, 92)
REACTOR_LIGHT = (80, 100, 130)
REACTOR_HIGHLIGHT = (105, 130, 160)

AM_DARK = (55, 38, 75)
AM_MID = (80, 58, 110)
AM_LIGHT = (110, 85, 145)
AM_HIGHLIGHT = (140, 115, 175)

ENERGY_DARK = (35, 65, 85)
ENERGY_MID = (50, 90, 120)
ENERGY_LIGHT = (70, 120, 155)

# Fission-specific (warm industrial)
FISSION_BODY = (70, 68, 62)
FISSION_DARK = (50, 48, 44)
FISSION_LIGHT = (95, 92, 85)
FISSION_HIGHLIGHT = (115, 112, 105)
WARNING_YELLOW = (200, 180, 40)
WARNING_ORANGE = (200, 120, 40)

# Geodesic-specific (exotic green-teal)
EXOTIC_DARK = (25, 60, 55)
EXOTIC_MID = (40, 95, 85)
EXOTIC_LIGHT = (65, 135, 120)
EXOTIC_HIGHLIGHT = (90, 170, 155)
EXOTIC_GLOW = (120, 210, 190)

# Whipple shield layers
WHIPPLE_PLATE = (130, 135, 145)
WHIPPLE_DARK = (100, 105, 115)
WHIPPLE_GAP = (22, 20, 18)


# ================================================================
# Drawing primitives
# ================================================================

def trap(d, cx, y, tw, bw, h, fill, outline=None):
    d.polygon([(cx - tw / 2, y), (cx + tw / 2, y),
               (cx + bw / 2, y + h), (cx - bw / 2, y + h)],
              fill=fill, outline=outline)

def rect(d, cx, y, w, h, fill, outline=None):
    d.rectangle([cx - w / 2, y, cx + w / 2, y + h], fill=fill, outline=outline)

def circ(d, cx, cy, r, **kw):
    d.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)

def lerp_color(c1, c2, t):
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


def draw_structural_band(d, cx, y, w, h, color_light, color_dark):
    hw = int(w / 2)
    d.rectangle([cx - hw, y, cx + hw, y + h], fill=color_light)
    d.rectangle([cx - hw, y + h, cx + hw, y + h + 1], fill=color_dark)


# ================================================================
# Reactor generators
# ================================================================

def generate_fission_reactor(size="small"):
    """Fission reactor: industrial cylindrical pressure vessel.

    Control rod cluster at top, thick cylindrical body with coolant
    channels, shadow shield (radiation shielding cap), and heat
    exchanger fins on sides. Warm grey-brown industrial palette.
    """
    if size == "small":
        GW, GH = 5, 7
    else:
        GW, GH = 9, 9

    PX = _capped_px(GW, GH)
    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = body_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(body_h * 0.03))
    ring_w = body_w * 0.6
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- SHADOW SHIELD (radiation shielding cap) ---
    shield_h = int(body_h * 0.10)
    shield_w = ring_w + 20
    rect(d, cx, y, shield_w, shield_h, FISSION_DARK, outline=STEEL_VERY_DARK)
    # Dense cross-hatching pattern
    shw = int(shield_w / 2)
    hatch_spacing = max(6, int(8 * scale))
    for i in range(0, int(shield_w), hatch_spacing):
        hx = cx - shw + i
        d.line([(hx, y + 2), (hx + hatch_spacing // 2, y + shield_h - 2)],
               fill=(40, 38, 35), width=1)
    # Warning stripe
    stripe_h = max(3, int(4 * scale))
    stripe_y = y + shield_h - stripe_h - 2
    d.rectangle([cx - shw + 4, stripe_y, cx + shw - 4, stripe_y + stripe_h],
                fill=WARNING_YELLOW)
    # Small radiation trefoil hint (three dots in triangle)
    tr = max(3, int(4 * scale))
    circ(d, cx, y + shield_h // 3, tr, fill=WARNING_YELLOW)
    circ(d, cx - tr - 1, y + shield_h // 3 + tr + 2, max(2, tr - 1),
         fill=WARNING_YELLOW)
    circ(d, cx + tr + 1, y + shield_h // 3 + tr + 2, max(2, tr - 1),
         fill=WARNING_YELLOW)
    y += shield_h

    # --- CONTROL ROD CLUSTER ---
    rod_section_h = int(body_h * 0.08)
    rod_w = body_w * 0.75
    rect(d, cx, y, rod_w, rod_section_h, FISSION_BODY, outline=FISSION_DARK)
    # Control rod channels (vertical lines with caps)
    n_rods = max(3, int(GW * 1.2))
    rod_r = max(3, int(5 * scale))
    rod_spacing = (rod_w - rod_r * 4) / max(1, n_rods - 1)
    rhw = int(rod_w / 2)
    for i in range(n_rods):
        rx = cx - rhw + rod_r * 2 + int(i * rod_spacing)
        # Rod channel
        d.rectangle([rx - rod_r // 2, y + 2, rx + rod_r // 2, y + rod_section_h - 2],
                    fill=STEEL_DARK)
        # Rod cap (actuator)
        circ(d, rx, y + rod_r + 1, rod_r, fill=STEEL_LIGHT, outline=STEEL_DARK)
        circ(d, rx, y + rod_r + 1, max(1, rod_r - 2), fill=STEEL_MID)
    y += rod_section_h

    # --- MAIN PRESSURE VESSEL ---
    # Fill remaining space minus bottom sections (heat exchanger + bottom mount)
    bot_sections = int(body_h * 0.14) + max(10, int(body_h * 0.03))
    vessel_h = body_h - (y - PAD) - bot_sections
    vessel_w = body_w * 0.82
    rect(d, cx, y, vessel_w, vessel_h, FISSION_BODY, outline=FISSION_DARK)

    # Left edge highlight
    vhw = int(vessel_w / 2)
    d.line([(cx - vhw + 3, y + 3), (cx - vhw + 3, y + vessel_h - 3)],
           fill=FISSION_HIGHLIGHT, width=max(2, int(2 * scale)))

    # Horizontal structural bands
    n_bands = max(3, int(vessel_h / (60 * scale)))
    for i in range(n_bands):
        frac = (i + 1) / (n_bands + 1)
        by = y + int(vessel_h * frac)
        draw_structural_band(d, cx, by, vessel_w + 8, max(3, int(4 * scale)),
                             FISSION_LIGHT, FISSION_DARK)

    # Vertical panel lines
    n_panels = max(2, int(vessel_w / (50 * scale)))
    for i in range(n_panels):
        px = cx - vhw + int((i + 1) * vessel_w / (n_panels + 1))
        d.line([(px, y + 4), (px, y + vessel_h - 4)], fill=FISSION_DARK, width=1)

    # Coolant pipe nubs on sides
    nub_len = max(8, int(12 * scale))
    nub_h = max(4, int(6 * scale))
    for s in [-1, 1]:
        for frac in [0.25, 0.5, 0.75]:
            ny = y + int(vessel_h * frac)
            nx = cx + s * vhw
            nx2 = nx + s * nub_len
            d.rectangle([min(nx, nx2), ny - nub_h // 2,
                         max(nx, nx2), ny + nub_h // 2],
                        fill=STEEL_MID, outline=STEEL_DARK)
            # Pipe end circle
            circ(d, nx2, ny, max(2, nub_h // 3), fill=STEEL_LIGHT)

    # Central viewport / inspection port
    vp_r = max(6, int(10 * scale))
    vp_y = y + vessel_h // 2
    circ(d, cx, vp_y, vp_r + 2, fill=FISSION_DARK)
    circ(d, cx, vp_y, vp_r, fill=(60, 50, 35))
    circ(d, cx, vp_y, max(3, vp_r - 3), fill=(80, 65, 40))
    # Warm glow hint
    circ(d, cx - 1, vp_y - 1, max(2, vp_r // 3), fill=(120, 80, 30))
    y += vessel_h

    # --- HEAT EXCHANGER SECTION ---
    hex_h = int(body_h * 0.14)
    hex_w = vessel_w + 20
    rect(d, cx, y, hex_w, hex_h, STEEL_MID, outline=STEEL_DARK)
    # Heat exchanger fin pattern
    hhw = int(hex_w / 2)
    n_fins = max(6, int(hex_w / (12 * scale)))
    fin_spacing = hex_w / (n_fins + 1)
    for i in range(n_fins):
        fx = cx - hhw + int((i + 1) * fin_spacing)
        t = i / max(1, n_fins - 1)
        fin_color = lerp_color(WARNING_ORANGE, (180, 60, 30), t)
        d.line([(fx, y + 3), (fx, y + hex_h - 3)], fill=fin_color, width=2)
    y += hex_h

    # --- BOTTOM MOUNT ---
    bot_mount_h = max(10, int(body_h * 0.03))
    draw_mount_ring(d, cx, y, mount_w, bot_mount_h, scale=scale)

    return img


def generate_fusion_reactor(size="small"):
    """Fusion reactor: D+He-3 magnetic confinement vessel.

    Toroidal magnetic coils wrapping a cylindrical/barrel-shaped
    confinement vessel. Blue-steel housing with copper coils,
    plasma viewports showing cyan glow, fuel injector nubs.
    """
    if size == "small":
        GW, GH = 7, 7
    else:
        GW, GH = 11, 9

    PX = _capped_px(GW, GH)
    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = body_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(body_h * 0.03))
    ring_w = body_w * 0.5
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- FUEL INJECTOR HEADER ---
    header_h = int(body_h * 0.06)
    header_w = ring_w + 10
    rect(d, cx, y, header_w, header_h, REACTOR_MID, outline=REACTOR_DARK)
    # Injector ports
    n_ports = max(3, int(header_w / (30 * scale)))
    port_r = max(2, int(3 * scale))
    for i in range(n_ports):
        px = cx - int(header_w / 2) + int((i + 0.5) * header_w / n_ports)
        circ(d, px, y + header_h // 2, port_r + 1, fill=REACTOR_DARK)
        circ(d, px, y + header_h // 2, port_r, fill=ENERGY_MID)
    y += header_h

    # --- CONFINEMENT VESSEL (barrel shape with coils) ---
    # Fill remaining space minus bottom mount
    bot_mount_total = max(10, int(body_h * 0.03))
    vessel_h = body_h - (y - PAD) - bot_mount_total
    vessel_top_w = header_w + 30
    vessel_max_w = body_w * 0.85
    vessel_bot_w = vessel_top_w + 20

    # Draw barrel profile — expands to max at middle, contracts at bottom
    n_strips = max(40, int(vessel_h / 4))
    sh = vessel_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        # Barrel curve: peaks at 0.45
        curve = math.sin(t * math.pi)
        w_at = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
        w_next = vessel_top_w + (vessel_max_w - vessel_top_w) * math.sin((i + 1) / n_strips * math.pi)
        sy = y + t * vessel_h
        shade = 0.3 + 0.4 * curve
        c = lerp_color(REACTOR_DARK, REACTOR_MID, shade)
        trap(d, cx, int(sy), w_at, w_next, int(sh) + 1, fill=c)

    # Left edge highlight (following barrel curve)
    for j in range(20):
        t = j / 19
        curve = math.sin(t * math.pi)
        w_at = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
        py = y + int(t * vessel_h)
        px = cx - int(w_at / 2) + 3
        d.point((px, py), fill=REACTOR_LIGHT)
        d.point((px, py + 1), fill=REACTOR_LIGHT)

    # Horizontal structural bands
    band_h = max(3, int(3 * scale))
    for frac in [0.15, 0.5, 0.85]:
        by = y + int(vessel_h * frac)
        curve = math.sin(frac * math.pi)
        bw = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
        bhw = int(bw / 2)
        d.rectangle([cx - bhw, by, cx + bhw, by + band_h], fill=REACTOR_LIGHT)
        d.rectangle([cx - bhw, by + band_h, cx + bhw, by + band_h + 1],
                    fill=REACTOR_DARK)

    # Vertical panel lines
    n_panels = max(2, int(vessel_max_w / (50 * scale)))
    for i in range(n_panels):
        pfrac = (i + 1) / (n_panels + 1)
        for j in range(n_strips):
            t = j / n_strips
            curve = math.sin(t * math.pi)
            w_at = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
            px = cx - int(w_at / 2) + int(w_at * pfrac)
            py = y + int(t * vessel_h / n_strips * n_strips)
            d.point((px, py), fill=REACTOR_DARK)

    # Toroidal magnetic coils
    n_coils = 5 if size == "small" else 8
    coil_th = max(5, int(7 * scale))
    coil_spacing = vessel_h / (n_coils + 1)
    for i in range(n_coils):
        cy_pos = y + int((i + 1) * coil_spacing)
        frac = (i + 1) / (n_coils + 1)
        curve = math.sin(frac * math.pi)
        cw = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
        chw = int(cw / 2)
        protrude = max(3, int(5 * scale))

        # Full ring band (the toroid appears as a full horizontal band from side view)
        band_y = cy_pos - coil_th // 2
        for row in range(coil_th):
            t_r = row / max(1, coil_th - 1)
            roundness = math.sin(t_r * math.pi)
            shade = 0.15 + 0.6 * roundness
            r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
            g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
            b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)
            ry = band_y + row
            d.rectangle([cx - chw - protrude, ry,
                         cx + chw + protrude, ry + 1],
                        fill=clamp_color((r, g, b)))
        # Top highlight
        d.line([(cx - chw - protrude + 2, band_y),
                (cx + chw + protrude - 2, band_y)],
               fill=COIL_HIGHLIGHT, width=1)
        # Bottom shadow
        d.line([(cx - chw - protrude + 2, band_y + coil_th - 1),
                (cx + chw + protrude - 2, band_y + coil_th - 1)],
               fill=(30, 25, 18), width=1)

    # Plasma viewport windows
    vp_r = max(5, int(8 * scale))
    viewport_fracs = [0.35, 0.65] if size == "small" else [0.25, 0.5, 0.75]
    for vfrac in viewport_fracs:
        curve = math.sin(vfrac * math.pi)
        vw = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
        vpy = y + int(vessel_h * vfrac)
        # Place viewport between coils
        circ(d, cx, vpy, vp_r + 2, fill=REACTOR_DARK)
        circ(d, cx, vpy, vp_r, fill=ENERGY_MID)
        circ(d, cx, vpy, max(3, vp_r - 2), fill=ENERGY_LIGHT)
        circ(d, cx - 1, vpy - 1, max(1, vp_r // 3), fill=(120, 180, 220))

    # Fuel injector nubs on sides
    nub_len = max(6, int(10 * scale))
    nub_h = max(4, int(6 * scale))
    for s in [-1, 1]:
        for frac in [0.3, 0.7]:
            curve = math.sin(frac * math.pi)
            nw = vessel_top_w + (vessel_max_w - vessel_top_w) * curve
            ny = y + int(vessel_h * frac)
            nx = cx + s * int(nw / 2)
            nx2 = nx + s * nub_len
            d.rectangle([min(nx, nx2), ny - nub_h // 2,
                         max(nx, nx2), ny + nub_h // 2],
                        fill=STEEL_MID, outline=STEEL_DARK)
            circ(d, nx2, ny, max(2, nub_h // 3), fill=ENERGY_MID)

    y += vessel_h

    # --- BOTTOM MOUNT ---
    bot_mount_h = max(10, int(body_h * 0.03))
    draw_mount_ring(d, cx, y, mount_w, bot_mount_h, scale=scale)

    return img


def generate_am_reactor(size="small"):
    """Antimatter reactor: controlled annihilation with magnetic capture.

    Penning trap antimatter storage at top (purple), annihilation
    chamber in the middle (blue-steel), magnetic nozzle capture coils,
    heavy containment rings. Very compact, dense.
    """
    if size == "small":
        GW, GH = 7, 7
    else:
        GW, GH = 11, 9

    PX = _capped_px(GW, GH)
    img_w = int(GW * PX) + PAD * 2
    img_h = int(GH * PX) + PAD * 2
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2
    body_w = GW * PX
    body_h = GH * PX
    scale = body_w / 300.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(body_h * 0.025))
    ring_w = body_w * 0.45
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- PENNING TRAP MODULE (antimatter storage) ---
    trap_h = int(body_h * 0.18)
    trap_w = ring_w + 30
    rect(d, cx, y, trap_w, trap_h, AM_MID, outline=AM_DARK)

    # Containment coil rings on trap
    thw = int(trap_w / 2)
    coil_ring_h = max(3, int(4 * scale))
    extra = max(3, int(5 * scale))
    for frac in [0.2, 0.5, 0.8]:
        ry = y + int(trap_h * frac)
        d.rectangle([cx - thw - extra, ry - coil_ring_h,
                     cx + thw + extra, ry + coil_ring_h], fill=AM_LIGHT)
        d.rectangle([cx - thw - extra, ry - coil_ring_h,
                     cx + thw + extra, ry - coil_ring_h + 1], fill=AM_HIGHLIGHT)
        d.rectangle([cx - thw - extra, ry + coil_ring_h - 1,
                     cx + thw + extra, ry + coil_ring_h], fill=AM_DARK)

    # Central glow line (antimatter containment field)
    glow_y = y + trap_h // 2
    glow_hw = int(trap_w * 0.35)
    d.line([(cx - glow_hw, glow_y), (cx + glow_hw, glow_y)],
           fill=AM_HIGHLIGHT, width=max(2, int(3 * scale)))
    d.line([(cx - glow_hw + 4, glow_y - 1), (cx + glow_hw - 4, glow_y - 1)],
           fill=(180, 150, 210), width=1)

    # Left edge highlight
    d.line([(cx - thw + 2, y + 2), (cx - thw + 2, y + trap_h - 2)],
           fill=AM_LIGHT, width=max(1, int(2 * scale)))
    y += trap_h

    # --- TRANSITION RING ---
    trans_ring_h = max(6, int(body_h * 0.02))
    trans_w = trap_w + 20
    hw = int(trans_w / 2)
    d.rectangle([cx - hw, y, cx + hw, y + trans_ring_h], fill=COIL_MID)
    d.rectangle([cx - hw, y, cx + hw, y + 1], fill=COIL_HIGHLIGHT)
    d.rectangle([cx - hw, y + trans_ring_h - 1, cx + hw, y + trans_ring_h],
                fill=COIL_DARK)
    y += trans_ring_h

    # --- ANNIHILATION CHAMBER ---
    # Fill remaining space minus bottom sections (power output + bottom mount)
    bot_sections = int(body_h * 0.10) + max(10, int(body_h * 0.025))
    chamber_h = body_h - (y - PAD) - bot_sections
    chamber_top_w = trans_w + 10
    chamber_max_w = body_w * 0.82
    chamber_bot_w = chamber_top_w + 10

    # Draw octagonal barrel shape
    n_strips = max(40, int(chamber_h / 4))
    sh = chamber_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        curve = math.sin(t * math.pi)
        w_at = chamber_top_w + (chamber_max_w - chamber_top_w) * curve
        w_next = chamber_top_w + (chamber_max_w - chamber_top_w) * \
                 math.sin((i + 1) / n_strips * math.pi)
        sy = y + t * chamber_h
        shade = 0.3 + 0.4 * curve
        c = lerp_color(REACTOR_DARK, REACTOR_MID, shade)
        trap(d, cx, int(sy), w_at, w_next, int(sh) + 1, fill=c)

    # Structural bands
    band_h = max(3, int(3 * scale))
    for frac in [0.2, 0.5, 0.8]:
        by = y + int(chamber_h * frac)
        curve = math.sin(frac * math.pi)
        bw = chamber_top_w + (chamber_max_w - chamber_top_w) * curve
        bhw = int(bw / 2)
        d.rectangle([cx - bhw, by, cx + bhw, by + band_h], fill=REACTOR_LIGHT)
        d.rectangle([cx - bhw, by + band_h, cx + bhw, by + band_h + 1],
                    fill=REACTOR_DARK)

    # Left edge highlight
    for j in range(20):
        t = j / 19
        curve = math.sin(t * math.pi)
        w_at = chamber_top_w + (chamber_max_w - chamber_top_w) * curve
        py = y + int(t * chamber_h)
        px = cx - int(w_at / 2) + 3
        d.point((px, py), fill=REACTOR_LIGHT)
        d.point((px, py + 1), fill=REACTOR_LIGHT)

    # Magnetic capture coils (copper rings around chamber)
    n_coils = 4 if size == "small" else 6
    coil_th = max(5, int(7 * scale))
    coil_spacing = chamber_h / (n_coils + 1)
    for i in range(n_coils):
        cy_pos = y + int((i + 1) * coil_spacing)
        frac = (i + 1) / (n_coils + 1)
        curve = math.sin(frac * math.pi)
        cw = chamber_top_w + (chamber_max_w - chamber_top_w) * curve
        chw = int(cw / 2)
        protrude = max(3, int(5 * scale))

        band_y = cy_pos - coil_th // 2
        for row in range(coil_th):
            t_r = row / max(1, coil_th - 1)
            shade = 0.15 + 0.6 * math.sin(t_r * math.pi)
            r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
            g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
            b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)
            ry = band_y + row
            d.rectangle([cx - chw - protrude, ry,
                         cx + chw + protrude, ry + 1],
                        fill=clamp_color((r, g, b)))
        d.line([(cx - chw - protrude + 2, band_y),
                (cx + chw + protrude - 2, band_y)],
               fill=COIL_HIGHLIGHT, width=1)
        d.line([(cx - chw - protrude + 2, band_y + coil_th - 1),
                (cx + chw + protrude - 2, band_y + coil_th - 1)],
               fill=(30, 25, 18), width=1)

    # Annihilation glow viewports
    vp_r = max(5, int(8 * scale))
    for vfrac in [0.35, 0.65]:
        vpy = y + int(chamber_h * vfrac)
        circ(d, cx, vpy, vp_r + 2, fill=REACTOR_DARK)
        circ(d, cx, vpy, vp_r, fill=(80, 50, 120))
        circ(d, cx, vpy, max(3, vp_r - 2), fill=(120, 80, 170))
        circ(d, cx - 1, vpy - 1, max(1, vp_r // 3), fill=(160, 120, 210))

    # Ion beam injector nubs
    nub_len = max(6, int(10 * scale))
    nub_h = max(4, int(6 * scale))
    for s in [-1, 1]:
        for frac in [0.3, 0.5, 0.7]:
            curve = math.sin(frac * math.pi)
            nw = chamber_top_w + (chamber_max_w - chamber_top_w) * curve
            ny = y + int(chamber_h * frac)
            nx = cx + s * int(nw / 2)
            nx2 = nx + s * nub_len
            d.rectangle([min(nx, nx2), ny - nub_h // 2,
                         max(nx, nx2), ny + nub_h // 2],
                        fill=STEEL_MID, outline=STEEL_DARK)
            circ(d, nx2, ny, max(2, nub_h // 3), fill=ENERGY_MID)

    y += chamber_h

    # --- POWER OUTPUT SECTION ---
    output_h = int(body_h * 0.10)
    output_w = body_w * 0.6
    rect(d, cx, y, output_w, output_h, REACTOR_MID, outline=REACTOR_DARK)
    # Power conduit terminals
    ohw = int(output_w / 2)
    n_terms = max(3, int(output_w / (30 * scale)))
    for i in range(n_terms):
        tx = cx - ohw + int((i + 0.5) * output_w / n_terms)
        tr = max(2, int(3 * scale))
        circ(d, tx, y + output_h // 2, tr, fill=ENERGY_LIGHT)
        circ(d, tx, y + output_h // 2, max(1, tr - 1), fill=ENERGY_MID)
    y += output_h

    # --- BOTTOM MOUNT ---
    bot_mount_h = max(10, int(body_h * 0.025))
    draw_mount_ring(d, cx, y, mount_w, bot_mount_h, scale=scale)

    return img


# ================================================================
# Shield generators
# ================================================================

def generate_whipple_shield(size="small"):
    """Passive Whipple Shield: multi-layer physical barrier.

    Stacked horizontal plates with visible separation gaps.
    Simple, industrial, heavy. Zero power requirement.
    The outermost layer is thickest (bumper plate), inner layers
    are thinner (spacing plates), backed by a structural frame.
    """
    sizes = {
        "small":  (9, 3),
        "medium": (13, 5),
        "large":  (19, 5),
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
    scale = body_w / 400.0
    y = PAD

    # --- MOUNTING BRACKET ---
    bracket_h = max(8, int(body_h * 0.06))
    bracket_w = nearest_odd(body_w * 0.4 / PX) * PX
    rect(d, cx, y, bracket_w, bracket_h, STEEL_MID, outline=STEEL_DARK)
    draw_bolts(d, cx, y, bracket_w, bracket_h,
               max(2, int(bracket_w / (15 * scale))), r=max(2, int(3 * scale)))
    y += bracket_h

    # --- STRUCTURAL BACKBONE ---
    backbone_h = int(body_h * 0.12)
    backbone_w = body_w * 0.3
    rect(d, cx, y, backbone_w, backbone_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    y += backbone_h

    # --- SUPPORT STRUTS (spread from backbone to full width) ---
    strut_h = int(body_h * 0.08)
    strut_w = max(3, int(4 * scale))
    n_struts = max(4, int(GW * 0.8))
    strut_spacing = body_w / (n_struts + 1)
    for i in range(n_struts):
        sx = cx - int(body_w / 2) + int((i + 1) * strut_spacing)
        # Strut from backbone down
        bx = cx - int(backbone_w / 2) + int(backbone_w * (i + 1) / (n_struts + 1))
        d.line([(bx, y), (sx, y + strut_h)], fill=STEEL_MID, width=strut_w)
    y += strut_h

    # --- WHIPPLE LAYERS ---
    remaining_h = body_h - (y - PAD)
    n_layers = {"small": 4, "medium": 5, "large": 6}[size]
    gap_h = max(3, int(remaining_h * 0.08))
    total_gaps = gap_h * (n_layers - 1)
    plate_h_total = remaining_h - total_gaps
    # First plate (bumper) is thicker
    bumper_h = int(plate_h_total * 0.35)
    inner_h = int((plate_h_total - bumper_h) / max(1, n_layers - 1))

    for layer in range(n_layers):
        if layer == 0:
            # Gap before first plate (from struts)
            pass

        h = bumper_h if layer == n_layers - 1 else inner_h
        # Each layer is full width
        plate_w = body_w

        # Plate color gets lighter toward outside (bumper is darkest)
        t = layer / max(1, n_layers - 1)
        plate_color = lerp_color(WHIPPLE_PLATE, WHIPPLE_DARK, t * 0.5)
        plate_edge = lerp_color(STEEL_LIGHT, STEEL_DARK, t * 0.3)

        phw = int(plate_w / 2)
        d.rectangle([cx - phw, y, cx + phw, y + h],
                    fill=plate_color, outline=STEEL_DARK)

        # Surface texture — horizontal seam lines
        if h > 8:
            n_seams = max(1, h // max(8, int(10 * scale)))
            for si in range(n_seams):
                seam_y = y + int((si + 1) * h / (n_seams + 1))
                d.line([(cx - phw + 3, seam_y), (cx + phw - 3, seam_y)],
                       fill=plate_edge, width=1)

        # Top highlight
        d.line([(cx - phw + 2, y + 1), (cx + phw - 2, y + 1)],
               fill=lerp_color(plate_color, (200, 205, 215), 0.4), width=1)

        y += h

        # Gap between layers (visible dark space with support posts)
        if layer < n_layers - 1:
            # Dark gap
            d.rectangle([cx - phw, y, cx + phw, y + gap_h],
                        fill=WHIPPLE_GAP)
            # Small support posts in gap
            n_posts = max(3, int(GW * 0.6))
            post_w = max(2, int(3 * scale))
            for pi in range(n_posts):
                post_x = cx - phw + int((pi + 0.5) * plate_w / n_posts)
                d.rectangle([post_x - post_w // 2, y,
                             post_x + post_w // 2, y + gap_h],
                            fill=STEEL_MID)
            y += gap_h

    return img


def generate_fres_shield(size="small"):
    """Active FRES (Field-Reinforced Electromagnetic Shield).

    A stack of superconducting solenoid coils generates an intense
    magnetic bubble. Forward-facing pre-ionization UV laser array
    converts neutral interstellar hydrogen to ions that the field can
    deflect. Electrostatic electrode discs between coils provide
    supplementary deflection. Cryocooler radiator fins on the sides
    keep superconductors cold. Power bus runs the full length.
    Power scales as v³.
    """
    sizes = {
        "small":  (9, 9),
        "medium": (13, 11),
        "large":  (17, 13),
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
    scale = body_w / 400.0
    y = PAD

    n_coils = {"small": 4, "medium": 6, "large": 8}[size]
    coil_w = body_w * 0.78
    coil_hw = int(coil_w / 2)
    coil_th = max(10, int(16 * scale))
    electrode_protrude = max(6, int(12 * scale))

    # --- PRE-IONIZATION EMITTER ARRAY (forward-facing UV lasers) ---
    emitter_h = int(body_h * 0.06)
    emitter_w = coil_w * 0.7
    ehw = int(emitter_w / 2)
    rect(d, cx, y, emitter_w, emitter_h, REACTOR_MID, outline=REACTOR_DARK)
    # UV emitter apertures — row of small lenses
    n_emitters = max(4, int(emitter_w / (18 * scale)))
    em_r = max(3, int(4 * scale))
    for i in range(n_emitters):
        ex = cx - ehw + int((i + 0.5) * emitter_w / n_emitters)
        ey = y + emitter_h // 2
        circ(d, ex, ey, em_r + 1, fill=REACTOR_DARK)
        circ(d, ex, ey, em_r, fill=(100, 80, 180))  # UV violet tint
        circ(d, ex, ey, max(1, em_r - 2), fill=(140, 110, 210))
    # Label band
    d.line([(cx - ehw + 3, y + 1), (cx + ehw - 3, y + 1)],
           fill=REACTOR_HIGHLIGHT, width=1)
    y += emitter_h

    # --- MOUNT RING ---
    mount_h = max(10, int(body_h * 0.02))
    ring_w = emitter_w + 20
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- POWER BUS CONNECTOR ---
    bus_h = int(body_h * 0.03)
    bus_w = ring_w - 10
    rect(d, cx, y, bus_w, bus_h, ENERGY_MID, outline=ENERGY_DARK)
    n_terms = max(3, int(bus_w / (30 * scale)))
    bhw = int(bus_w / 2)
    for i in range(n_terms):
        tx = cx - bhw + int((i + 0.5) * bus_w / n_terms)
        tr = max(2, int(3 * scale))
        circ(d, tx, y + bus_h // 2, tr, fill=ENERGY_LIGHT)
    y += bus_h

    # --- SOLENOID STACK (the main body — coils with open bays between) ---
    # Fill remaining space minus bottom mount
    bot_mount_total = max(10, int(body_h * 0.02))
    stack_h = body_h - (y - PAD) - bot_mount_total
    bay_h = stack_h / (n_coils * 2 - 1)  # alternating coil/bay

    # Calculate all coil and bay positions
    coil_positions = []  # (y_center, width)
    electrode_positions = []  # (y_center, width)

    for i in range(n_coils):
        coil_y_center = y + int((i * 2) * bay_h + bay_h / 2)
        coil_positions.append(coil_y_center)
        # Electrostatic electrode between every other pair of coils
        if i > 0 and i % 2 == 0:
            elec_y = y + int((i * 2 - 1) * bay_h + bay_h / 2)
            electrode_positions.append(elec_y)

    # Step 1: Draw structural longerons (vertical rails connecting coils)
    longeron_w = max(3, int(4 * scale))
    n_long = max(3, int(coil_w / (80 * scale)))
    for li in range(n_long):
        frac = li / max(1, n_long - 1)
        lx = cx - coil_hw + int(frac * coil_w)
        d.rectangle([lx - longeron_w // 2, y,
                     lx + longeron_w // 2, y + stack_h],
                    fill=STEEL_MID)
        if li == 0:
            d.rectangle([lx - longeron_w // 2, y,
                         lx - longeron_w // 2 + 1, y + stack_h],
                        fill=STEEL_LIGHT)

    # Step 2: Draw power bus cables running the full length on both sides
    cable_w = max(3, int(4 * scale))
    cable_inset = max(8, int(15 * scale))
    for s in [-1, 1]:
        cable_x = cx + s * (coil_hw - cable_inset)
        d.rectangle([cable_x - cable_w // 2, y,
                     cable_x + cable_w // 2, y + stack_h],
                    fill=ENERGY_MID)
        d.rectangle([cable_x - cable_w // 2 + (1 if s == -1 else 0), y,
                     cable_x - cable_w // 2 + 1 + (1 if s == -1 else 0),
                     y + stack_h],
                    fill=ENERGY_LIGHT)

    # Step 3: Draw electrostatic deflection electrode discs
    elec_th = max(4, int(6 * scale))
    for ey in electrode_positions:
        elec_w = coil_w + electrode_protrude * 2
        ehw_e = int(elec_w / 2)
        # Thin metallic disc protruding beyond the coils
        for row in range(elec_th):
            t_r = row / max(1, elec_th - 1)
            shade = 0.4 + 0.4 * math.sin(t_r * math.pi)
            r = int(STEEL_DARK[0] + (STEEL_HIGHLIGHT[0] - STEEL_DARK[0]) * shade)
            g = int(STEEL_DARK[1] + (STEEL_HIGHLIGHT[1] - STEEL_DARK[1]) * shade)
            b = int(STEEL_DARK[2] + (STEEL_HIGHLIGHT[2] - STEEL_DARK[2]) * shade)
            ry = ey - elec_th // 2 + row
            d.rectangle([cx - ehw_e, ry, cx + ehw_e, ry + 1],
                        fill=clamp_color((r, g, b)))
        # Edge highlight
        d.line([(cx - ehw_e + 2, ey - elec_th // 2),
                (cx + ehw_e - 2, ey - elec_th // 2)],
               fill=STEEL_HIGHLIGHT, width=1)
        # Charge indicator dots at the tips
        for s in [-1, 1]:
            circ(d, cx + s * ehw_e, ey, max(2, int(3 * scale)),
                 fill=ENERGY_LIGHT)

    # Step 4: Draw superconducting solenoid coils (over longerons)
    for ci, coil_y in enumerate(coil_positions):
        band_y = coil_y - coil_th // 2
        protrude = max(4, int(6 * scale))

        # Main coil body with 3D cross-section shading
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
            d.rectangle([cx - coil_hw - protrude, ry,
                         cx + coil_hw + protrude, ry + 1],
                        fill=clamp_color((r, g, b)))

        # Specular highlight
        d.line([(cx - coil_hw - protrude + 2, band_y + 1),
                (cx + coil_hw + protrude - 2, band_y + 1)],
               fill=COIL_HIGHLIGHT, width=1)
        # Bottom shadow
        d.line([(cx - coil_hw - protrude + 2, band_y + coil_th - 1),
                (cx + coil_hw + protrude - 2, band_y + coil_th - 1)],
               fill=(30, 25, 18), width=1)

        # Winding marks (diagonal lines showing wire wraps)
        wind_color = clamp_color((COIL_LIGHT[0] + 15,
                                  COIL_LIGHT[1] + 15,
                                  COIL_LIGHT[2] + 10))
        full_span = (coil_hw + protrude) * 2
        n_winds = max(6, int(full_span / max(3, int(5 * scale))))
        wind_spacing = max(2, full_span // max(1, n_winds))
        for wi in range(n_winds):
            wx = cx - coil_hw - protrude + 3 + wi * wind_spacing
            if wx < cx + coil_hw + protrude - 3:
                d.line([(wx, band_y + 2),
                        (wx + max(1, coil_th // 5), band_y + coil_th - 2)],
                       fill=wind_color, width=1)

    # Step 5: Cryocooler radiator fins on the sides
    fin_len = max(8, int(14 * scale))
    fin_h = max(2, int(3 * scale))
    n_fins_per_side = max(6, int(stack_h / (20 * scale)))
    for s in [-1, 1]:
        base_x = cx + s * (coil_hw + max(4, int(6 * scale)))
        for fi in range(n_fins_per_side):
            fy = y + int((fi + 0.5) * stack_h / n_fins_per_side)
            tip_x = base_x + s * fin_len
            # Fin as thin trapezoid
            d.polygon([
                (base_x, fy - fin_h),
                (tip_x, fy - max(1, fin_h // 2)),
                (tip_x, fy + max(1, fin_h // 2)),
                (base_x, fy + fin_h),
            ], fill=STEEL_LIGHT, outline=STEEL_DARK)

    y += stack_h

    # --- BOTTOM MOUNT ---
    bot_h = max(10, int(body_h * 0.02))
    bot_w = coil_w + 10
    draw_mount_ring(d, cx, y, mount_w, bot_h, scale=scale)

    return img


def generate_geodesic_deflector(size="small"):
    """Geodesic Deflector: single horizontal stadium-shaped exotic matter vessel.

    A containment vessel holding stabilized exotic matter (negative energy
    density) that generates controlled spacetime curvature. Single horizontal
    stadium shape (rectangle with semicircular left/right caps). Structural
    scaffolding wraps around the perimeter. Teal glow line across the center.
    """
    sizes = {
        "small":  (9, 9),
        "medium": (13, 11),
        "large":  (17, 13),
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
    scale = body_w / 400.0
    y = PAD

    # --- MOUNT RING ---
    mount_h = max(10, int(body_h * 0.02))
    ring_w = body_w * 0.3
    mount_w = nearest_odd(ring_w / PX) * PX
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=scale)

    # --- POWER CONDITIONING UNIT ---
    pcu_h = int(body_h * 0.06)
    pcu_w = ring_w + 30
    rect(d, cx, y, pcu_w, pcu_h, REACTOR_MID, outline=REACTOR_DARK)
    pcu_hw = int(pcu_w / 2)
    n_ind = max(3, int(pcu_w / (18 * scale)))
    for i in range(n_ind):
        ix = cx - pcu_hw + int((i + 0.5) * pcu_w / n_ind)
        circ(d, ix, y + pcu_h // 2, max(2, int(3 * scale)),
             fill=EXOTIC_LIGHT)
    d.line([(cx - pcu_hw + 3, y + 1), (cx + pcu_hw - 3, y + 1)],
           fill=REACTOR_HIGHLIGHT, width=1)
    y += pcu_h
    pcu_y_bottom = y

    # --- Horizontal stadium geometry ---
    gap_h = int(body_h * 0.03)
    bot_mount_h = max(10, int(body_h * 0.02))
    bot_mount_w = body_w * 0.35

    # Stadium fills available vertical space, capped for horizontal aspect ratio
    stadium_y_start = y + gap_h
    bot_y = body_h + PAD - bot_mount_h
    available_h = bot_y - gap_h - stadium_y_start
    stadium_w = int(body_w * 0.92)
    stadium_h = min(available_h, int(stadium_w * 0.65))
    # Re-center vertically in the available space
    stadium_y_start += (available_h - stadium_h) // 2
    stadium_cy = stadium_y_start + stadium_h // 2
    R = stadium_h // 2  # radius of semicircular caps
    straight_w = max(0, stadium_w - 2 * R)  # flat middle width

    def stadium_hw(d_from_center):
        """Half-width of the stadium at vertical distance d from center."""
        ad = abs(d_from_center)
        if ad >= R:
            return 0
        cap = math.sqrt(max(0, R * R - ad * ad))
        return int(straight_w / 2 + cap)

    # --- SCAFFOLDING STRUTS (drawn behind stadium) ---
    strut_w = max(3, int(5 * scale))
    standoff = max(4, int(8 * scale))
    n_struts = max(5, int(GW * 0.7))

    for si in range(n_struts):
        frac = si / max(1, n_struts - 1)

        top_spread = stadium_w + standoff * 4
        bot_spread = stadium_w + standoff * 4
        top_x = cx - int(top_spread / 2) + int(frac * top_spread)
        bot_x = cx - int(bot_spread / 2) + int(frac * bot_spread)

        # Polyline wrapping around the stadium silhouette
        points = [(top_x, pcu_y_bottom)]

        n_samples = max(20, stadium_h // 6)
        for sample in range(n_samples + 1):
            d_fc = -R + (sample / n_samples) * 2 * R
            py = stadium_cy + int(d_fc)
            hw = stadium_hw(d_fc) + standoff

            sil_left = cx - hw
            sil_right = cx + hw
            sx = sil_left + int(frac * (sil_right - sil_left))
            points.append((sx, py))

        points.append((bot_x, bot_y))

        for i in range(len(points) - 1):
            d.line([points[i], points[i + 1]],
                   fill=STEEL_MID, width=strut_w)
        if si == 0:
            for i in range(len(points) - 1):
                d.line([points[i], points[i + 1]],
                       fill=STEEL_LIGHT, width=1)

    # Horizontal cross braces on scaffolding
    for cfrac in [0.15, 0.5, 0.85]:
        d_fc = -R + cfrac * 2 * R
        hw = stadium_hw(d_fc) + standoff
        cy_b = stadium_cy + int(d_fc)
        d.line([(cx - hw, cy_b), (cx + hw, cy_b)],
               fill=STEEL_LIGHT, width=max(2, int(3 * scale)))

    # --- STADIUM BODY (exotic matter vessel) ---
    n_stab_coils = {"small": 6, "medium": 8, "large": 12}[size]

    for row in range(stadium_h):
        d_from_center = row - stadium_h // 2
        py = stadium_y_start + row

        hw = stadium_hw(d_from_center)
        if hw < 2:
            continue

        t_vert = d_from_center / max(1, R)
        tv_clamped = max(-1.0, min(1.0, t_vert))

        # Top-down lighting (brighter at top)
        vert_light = 0.55 + 0.35 * (1.0 - (tv_clamped + 1) / 2)
        # Vertical tube curvature (round cross-section)
        tube_round = 0.35 + 0.50 * math.sqrt(max(0, 1.0 - tv_clamped * tv_clamped))

        # Coil bands
        coil_spacing = 2.0 / max(1, n_stab_coils)
        coil_phase = ((tv_clamped + 1.0) % coil_spacing) / coil_spacing
        on_coil = coil_phase < 0.12

        x_left = cx - hw
        x_right = cx + hw
        draw_w = x_right - x_left

        for col in range(draw_w):
            px = x_left + col
            dx = px - cx

            # Horizontal shading: flat in straight section, curved in caps
            if abs(dx) <= straight_w / 2:
                shade = vert_light * tube_round
            else:
                cap_cx = cx + (straight_w / 2 if dx > 0 else -straight_w / 2)
                cap_dx = min(1.0, abs(px - cap_cx) / max(1, R))
                cap_shade = 0.5 + 0.5 * math.sqrt(max(0, 1.0 - cap_dx * cap_dx))
                shade = vert_light * tube_round * cap_shade

            if on_coil:
                r = int(COIL_DARK[0] + (COIL_LIGHT[0] - COIL_DARK[0]) * shade)
                g = int(COIL_DARK[1] + (COIL_LIGHT[1] - COIL_DARK[1]) * shade)
                b = int(COIL_DARK[2] + (COIL_LIGHT[2] - COIL_DARK[2]) * shade)
            else:
                r = int(EXOTIC_DARK[0] + (EXOTIC_LIGHT[0] - EXOTIC_DARK[0]) * shade)
                g = int(EXOTIC_DARK[1] + (EXOTIC_LIGHT[1] - EXOTIC_DARK[1]) * shade)
                b = int(EXOTIC_DARK[2] + (EXOTIC_LIGHT[2] - EXOTIC_DARK[2]) * shade)

            d.point((px, py), fill=clamp_color((r, g, b)))

    # --- CENTER GLOW LINE ---
    glow_hw = stadium_hw(0)
    glow_w = max(2, int(3 * scale))
    d.line([(cx - glow_hw + 4, stadium_cy), (cx + glow_hw - 4, stadium_cy)],
           fill=EXOTIC_GLOW, width=glow_w)
    d.line([(cx - glow_hw + 6, stadium_cy - 1),
            (cx + glow_hw - 6, stadium_cy - 1)],
           fill=EXOTIC_HIGHLIGHT, width=1)
    d.line([(cx - glow_hw + 6, stadium_cy + 1),
            (cx + glow_hw - 6, stadium_cy + 1)],
           fill=EXOTIC_HIGHLIGHT, width=1)

    # Cherenkov-like glow highlight along the top edge
    for col in range(0, stadium_w, max(2, int(3 * scale))):
        gx = cx - stadium_w // 2 + col
        d_fc = -R + int(R * 0.15)
        hw_at = stadium_hw(d_fc)
        if abs(gx - cx) <= hw_at:
            gy = stadium_cy + d_fc
            if 0 <= gx < img_w and 0 <= gy < img_h:
                d.point((gx, gy), fill=EXOTIC_GLOW)

    # Gravimetric sensor pods around the perimeter
    sensor_r = max(3, int(5 * scale))
    n_sensors = {"small": 4, "medium": 6, "large": 8}[size]
    for i in range(n_sensors):
        angle = (i / n_sensors) * math.pi * 2
        if math.cos(angle) < -0.3:
            continue
        d_fc = -math.cos(angle) * R
        hw_at = stadium_hw(d_fc)
        sx = cx + int(math.sin(angle) * (hw_at + sensor_r + 3))
        sy = stadium_cy + int(d_fc)
        if 0 <= sx - sensor_r and sx + sensor_r < img_w and \
           0 <= sy - sensor_r and sy + sensor_r < img_h:
            circ(d, sx, sy, sensor_r + 1, fill=STEEL_MID, outline=STEEL_DARK)
            circ(d, sx, sy, max(1, sensor_r - 1), fill=EXOTIC_LIGHT)

    # Exotic matter injector ports at left and right tips
    port_r = max(3, int(4 * scale))
    for s in [-1, 1]:
        px = cx + s * (straight_w // 2 + R - port_r)
        circ(d, px, stadium_cy, port_r + 2, fill=STEEL_MID)
        circ(d, px, stadium_cy, port_r, fill=STEEL_DARK)
        circ(d, px, stadium_cy, max(1, port_r - 2), fill=EXOTIC_MID)

    # --- BOTTOM MOUNT ---
    draw_mount_ring(d, cx, bot_y, mount_w, bot_mount_h, scale=scale)

    return img


# ================================================================
# Registry and main
# ================================================================

PARTS = {
    # Reactors
    "reactor_fission_small":   ("Fission Reactor (Small)",   lambda: generate_fission_reactor("small")),
    "reactor_fission_large":   ("Fission Reactor (Large)",   lambda: generate_fission_reactor("large")),
    "reactor_fusion_small":    ("Fusion Reactor (Small)",    lambda: generate_fusion_reactor("small")),
    "reactor_fusion_large":    ("Fusion Reactor (Large)",    lambda: generate_fusion_reactor("large")),
    "reactor_am_small":        ("Antimatter Reactor (Small)", lambda: generate_am_reactor("small")),
    "reactor_am_large":        ("Antimatter Reactor (Large)", lambda: generate_am_reactor("large")),
    # Shields
    "shield_whipple_small":    ("Passive Whipple (Small)",   lambda: generate_whipple_shield("small")),
    "shield_whipple_medium":   ("Passive Whipple (Medium)",  lambda: generate_whipple_shield("medium")),
    "shield_whipple_large":    ("Passive Whipple (Large)",   lambda: generate_whipple_shield("large")),
    "shield_fres_small":       ("Active FRES (Small)",       lambda: generate_fres_shield("small")),
    "shield_fres_medium":      ("Active FRES (Medium)",      lambda: generate_fres_shield("medium")),
    "shield_fres_large":       ("Active FRES (Large)",       lambda: generate_fres_shield("large")),
    "shield_geodesic_small":   ("Geodesic Deflector (Small)", lambda: generate_geodesic_deflector("small")),
    "shield_geodesic_medium":  ("Geodesic Deflector (Medium)", lambda: generate_geodesic_deflector("medium")),
    "shield_geodesic_large":   ("Geodesic Deflector (Large)", lambda: generate_geodesic_deflector("large")),
}


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    output_dir = os.path.join(project_root, "data", "sprites", "parts")
    os.makedirs(output_dir, exist_ok=True)

    for part_id, (name, gen_func) in PARTS.items():
        print(f"Generating {name}...")
        img = gen_func()
        path = os.path.join(output_dir, f"{part_id}.png")
        img.save(path)
        print(f"  -> {path}  ({img.size[0]}x{img.size[1]})")

    print(f"\nDone! Generated {len(PARTS)} sprites.")
