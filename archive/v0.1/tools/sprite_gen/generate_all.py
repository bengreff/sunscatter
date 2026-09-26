#!/usr/bin/env python3
"""
Generate sprites for all 16 Sunscatter engines.

Nozzle expansion ratio derived from ISP. Bell height auto-computed for
consistent proportions. Each engine gets distinctive visual features based
on its cycle type, propellant, gimbal, and role.

Distinctive features by cycle/role:
  staged combustion → preburner domes, exhaust stubs
  full-flow         → dual preburner domes, redundant valve bodies, manifold
  gas generator     → GG exhaust dump pipe, turbine exhaust wrap (Mammoth)
  expander          → heat exchanger fins on powerhead
  0° gimbal         → fixed mount frame (rigid truss)
  >0° gimbal        → gimbal actuators (larger for higher gimbal)
  vacuum            → regen/rad transition flange, nozzle extension skirt
  crew-rated        → redundant valve bodies
  twin-chamber      → crossover ducts between chambers
  large engines     → turbopump inlet ducts, thrust structure skirt
"""

from PIL import Image, ImageDraw
import math
import os

# ================================================================
# Palette
# ================================================================
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)
ABLATIVE_TIP = (70, 62, 50)
ABLATIVE_INNER = (55, 48, 38)
PIPE_COLOR = (65, 68, 75)
PIPE_HIGHLIGHT = (100, 105, 115)
NOZZLE_UPPER = (85, 88, 96)
VALVE_COLOR = (72, 75, 82)

PX = 360
MAX_SPRITE_PX = 4096
PAD = 0
BELL_EXP = 1.5

def _capped_px(w, h):
    """Return effective PX capped so the output image stays within MAX_SPRITE_PX."""
    return min(PX, MAX_SPRITE_PX // max(w, h))


# ================================================================
# Drawing primitives
# ================================================================

def trap(d, cx, y, tw, bw, h, fill, outline=None):
    d.polygon([(cx-tw/2, y), (cx+tw/2, y),
               (cx+bw/2, y+h), (cx-bw/2, y+h)],
              fill=fill, outline=outline)

def circ(d, cx, cy, r, **kw):
    d.ellipse([cx-r, cy-r, cx+r, cy+r], **kw)

def bell_w(frac, throat, exit_w):
    return throat + (exit_w - throat) * (1 - (1 - frac) ** BELL_EXP)

def bezier2(p0, p1, p2, n=20):
    pts = []
    for i in range(n + 1):
        t = i / n
        x = (1-t)**2*p0[0] + 2*(1-t)*t*p1[0] + t**2*p2[0]
        y = (1-t)**2*p0[1] + 2*(1-t)*t*p1[1] + t**2*p2[1]
        pts.append((int(x), int(y)))
    return pts

def draw_pipe_pts(d, pts, width=4):
    for k in range(len(pts) - 1):
        d.line([pts[k], pts[k+1]], fill=PIPE_COLOR, width=width)
    for k in range(len(pts) - 1):
        d.line([pts[k], pts[k+1]], fill=PIPE_HIGHLIGHT, width=1)


# ================================================================
# Component drawing functions
# ================================================================

def draw_mount(d, cx, y, w, h, n_bolts=5, bolt_r=3, scale=1.0):
    trap(d, cx, y, w, w, h, STEEL_MID, outline=STEEL_DARK)
    hw = int(w / 2)
    d.line([(cx-hw+2, y+1), (cx+hw-2, y+1)], fill=STEEL_HIGHLIGHT,
           width=max(1, int(scale)))
    for i in range(n_bolts):
        if n_bolts > 1:
            bx = cx - hw + bolt_r*2 + i * int((w - bolt_r*4) / (n_bolts-1))
        else:
            bx = cx
        circ(d, bx, y + h//2, bolt_r, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y + h//2, max(1, bolt_r-1), fill=STEEL_DARK)

def draw_powerhead(d, cx, y, tw, bw, h,
                   exhaust_stubs=False, gas_gen_dump=False, scale=1.0):
    trap(d, cx, y, tw, bw, h, STEEL_MID, outline=STEEL_DARK)
    d.line([(cx-int(tw/2)+2, y+2), (cx-int(bw/2)+2, y+h-2)],
           fill=STEEL_LIGHT, width=max(1, int(scale)))
    band_h = max(2, int(2 * scale))
    for frac in [0.3, 0.65]:
        by = y + int(h * frac)
        bw_at = tw + (bw - tw) * frac
        hw = int(bw_at / 2)
        d.rectangle([cx-hw, by, cx+hw, by+band_h], fill=STEEL_LIGHT)
        d.rectangle([cx-hw, by+band_h, cx+hw, by+band_h+1], fill=STEEL_DARK)
    if exhaust_stubs:
        br = max(4, int(h * 0.16))
        by = y + int(h * 0.4)
        for s in [-1, 1]:
            # Position stubs overlapping the powerhead edge
            bx = cx + s * int(bw/2 - br * 0.3)
            circ(d, bx, by, br+2, fill=STEEL_MID, outline=STEEL_DARK)
            circ(d, bx, by, br-2, fill=INTERIOR)
    if gas_gen_dump:
        pw = max(3, int(4 * scale))
        # Dump pipe curves out from powerhead then back down to chamber bottom
        start = (cx + int(bw/2 - 5), y + int(h*0.3))
        mid = (cx + int(bw/2 + h*0.2), y + int(h*0.55))
        end = (cx + int(bw/2 - 2), y + h - 2)
        draw_pipe_pts(d, bezier2(start, mid, end, 12), width=pw)


# --- NEW DISTINCTIVE COMPONENTS ---

def draw_preburner_domes(d, cx, y_top, ph_top_w, n=1, px_scale=1.0):
    """Rounded dome(s) on top of powerhead for staged combustion / full-flow."""
    dome_r = max(int(4 * px_scale), int(ph_top_w * 0.06))
    gap = max(2, int(2 * px_scale))
    if n == 1:
        # Single centered dome
        circ(d, cx, y_top, dome_r, fill=STEEL_LIGHT, outline=STEEL_DARK)
        circ(d, cx, y_top, dome_r - gap, fill=STEEL_MID)
    elif n == 2:
        # Dual domes (full-flow: ox-rich + fuel-rich preburners)
        for s in [-1, 1]:
            dx = cx + s * int(ph_top_w * 0.2)
            circ(d, dx, y_top, dome_r, fill=STEEL_LIGHT, outline=STEEL_DARK)
            circ(d, dx, y_top, dome_r - gap, fill=STEEL_MID)

def draw_fixed_mount_frame(d, cx, mount_y, ring_w, ch_y, cc_w, cc_h, scale=1.0):
    """Rigid truss for 0° gimbal engines (instead of actuators)."""
    sw = max(2, int(3 * scale))
    for s in [-1, 1]:
        # Anchor struts ON the ring edge and chamber edge
        tx = cx + s * int(ring_w / 2 - 2)
        ty = mount_y + 3
        bx = cx + s * int(cc_w / 2)
        by = ch_y + int(cc_h * 0.7)
        d.line([(tx, ty), (bx, by)], fill=STEEL_DARK, width=sw)
        d.line([(tx + s, ty), (bx + s, by)], fill=STEEL_MID, width=1)
        # Cross brace connecting strut to chamber wall
        mx = (tx + bx) // 2
        my = (ty + by) // 2
        d.line([(mx, my), (cx + s * int(cc_w / 2), my)],
               fill=STEEL_DARK, width=max(1, sw - 1))

def draw_thrust_skirt(d, cx, y, top_w, bot_w, h):
    """Structural shell between mount and powerhead for larger engines."""
    trap(d, cx, y, top_w, bot_w, h, STEEL_DARK, outline=STEEL_VERY_DARK)
    # Inner lighter section
    trap(d, cx, y + 1, top_w - 6, bot_w - 6, h - 1, STEEL_MID)
    # Horizontal stiffener
    hw = int((top_w + bot_w) / 4)
    d.rectangle([cx - hw, y + h // 2 - 1, cx + hw, y + h // 2 + 1],
                fill=STEEL_LIGHT)

def draw_heat_exchanger_fins(d, cx, y, ph_bot_w, ph_h, scale=1.0):
    """Zigzag fins on powerhead sides for expander cycle engines."""
    fin_w = max(3, int(6 * scale))
    n_fins = max(3, int(ph_h / (8 * scale)))
    for s in [-1, 1]:
        base_x = cx + s * int(ph_bot_w / 2 - 2)
        for i in range(n_fins):
            fy = y + int((i + 0.5) * ph_h / n_fins)
            tip_x = base_x + s * fin_w
            # Fin triangle
            d.polygon([
                (base_x, fy - 2), (tip_x, fy), (base_x, fy + 2)
            ], fill=STEEL_LIGHT, outline=STEEL_DARK)

def draw_turbopump_inlet_ducts(d, cx, mount_y, mount_h, ring_w, ph_top_y, ph_top_w, scale=1.0):
    """Thick tubes from mount ring down into powerhead top."""
    duct_w = max(3, int(5 * scale))
    for s in [-1, 1]:
        # Position ducts well inside the mount ring
        dx = cx + s * int(ph_top_w / 2 - 4 * scale)
        d.line([(dx, mount_y + mount_h), (dx, ph_top_y + 4)],
               fill=PIPE_COLOR, width=duct_w)
        d.line([(dx - s, mount_y + mount_h), (dx - s, ph_top_y + 4)],
               fill=PIPE_HIGHLIGHT, width=1)
        # Flange at bottom
        flange_w = duct_w // 2 + max(1, int(scale))
        d.rectangle([dx - flange_w, ph_top_y + 2,
                     dx + flange_w, ph_top_y + 2 + max(2, int(3*scale))],
                    fill=STEEL_LIGHT)

def draw_regen_manifold(d, cx, y, throat_w, scale=1.0):
    """Horizontal ring at top of nozzle where coolant enters nozzle walls."""
    mw = throat_w + 12 * scale
    mh = max(3, int(5 * scale))
    hw = int(mw / 2)
    d.rectangle([cx - hw, y - mh, cx + hw, y], fill=PIPE_COLOR)
    d.rectangle([cx - hw, y - mh, cx + hw, y - mh + 1], fill=PIPE_HIGHLIGHT)
    d.rectangle([cx - hw, y - 1, cx + hw, y], fill=STEEL_DARK)

def draw_redundant_valves(d, cx, y, ph_bot_w, ph_h, scale=1.0):
    """Box-shaped valve body protrusions on powerhead for crew-rated engines."""
    vw = max(4, int(8 * scale))
    vh = max(3, int(6 * scale))
    for s in [-1, 1]:
        vx = cx + s * int(ph_bot_w / 2 - vw / 2 - 4 * scale)
        vy = y + int(ph_h * 0.55)
        d.rectangle([vx - vw//2, vy, vx + vw//2, vy + vh],
                    fill=VALVE_COLOR, outline=STEEL_DARK)
        # Small highlight
        d.rectangle([vx - vw//2 + 1, vy + 1, vx + vw//2 - 1, vy + 2],
                    fill=STEEL_LIGHT)

def draw_crossover_ducts(d, cx, y, spacing, cc_h, pw=4):
    """Horizontal pipes connecting twin chambers on Panther."""
    for frac in [0.3, 0.7]:
        dy = y + int(cc_h * frac)
        left_x = cx - int(spacing) + 5
        right_x = cx + int(spacing) - 5
        d.line([(left_x, dy), (right_x, dy)], fill=PIPE_COLOR, width=pw)
        d.line([(left_x, dy - 1), (right_x, dy - 1)], fill=PIPE_HIGHLIGHT, width=1)

def draw_turbine_exhaust_wrap(d, cx, bell_y, nozzle_h, throat_w, exit_w, frac=0.15, scale=1.0):
    """Ring of small exhaust ports on nozzle bell (F-1 style GG exhaust dump)."""
    ry = int(bell_y + frac * nozzle_h)
    rw = bell_w(frac, throat_w, exit_w)
    rhw = int(rw / 2)
    ring_h = max(3, int(4 * scale))
    # Exhaust manifold ring
    d.rectangle([cx - rhw, ry - ring_h, cx + rhw, ry + ring_h], fill=STEEL_DARK)
    d.rectangle([cx - rhw, ry - ring_h + 1, cx + rhw, ry - ring_h + 2], fill=STEEL_HIGHLIGHT)
    # Exhaust port holes
    port_r = max(2, int(3 * scale))
    spacing = max(8, int(port_r * 4))
    n_ports = max(4, int(rw / spacing))
    for i in range(n_ports):
        px = cx - rhw + spacing//2 + i * int((rw - spacing) / max(1, n_ports - 1))
        circ(d, px, ry + 1, port_r, fill=INTERIOR)

def draw_transition_flange(d, cx, bell_y, nozzle_h, throat_w, exit_w, regen_frac, scale=1.0):
    """Bolted flange joint at regen/rad-cooled boundary (more prominent than seam)."""
    fy = int(bell_y + regen_frac * nozzle_h)
    fw = bell_w(regen_frac, throat_w, exit_w)
    fhw = int(fw / 2)
    flange_extra = int(4 * scale)
    # Flange wider than bell at that point
    d.rectangle([cx - fhw - flange_extra, fy - 3,
                 cx + fhw + flange_extra, fy + 3], fill=STEEL_LIGHT)
    d.rectangle([cx - fhw - flange_extra, fy + 3,
                 cx + fhw + flange_extra, fy + 4], fill=STEEL_DARK)
    # Bolts along flange
    bolt_spacing = 20 * PX / 90
    bolt_r = max(2, int(2 * PX / 90))
    n_bolts = max(3, int(fw / bolt_spacing))
    pad = int(4 * PX / 90)
    for i in range(n_bolts):
        bx = cx - fhw + pad + i * int((fw - pad * 2) / max(1, n_bolts - 1))
        circ(d, bx, fy, bolt_r, fill=STEEL_HIGHLIGHT)

def draw_nozzle_extension_seam(d, cx, bell_y, nozzle_h, throat_w, exit_w, frac=0.6):
    """Visible joint where deployable nozzle extension attaches."""
    sy = int(bell_y + frac * nozzle_h)
    sw = bell_w(frac, throat_w, exit_w)
    shw = int(sw / 2)
    # Step offset — outer wall jogs inward slightly
    d.rectangle([cx - shw - 2, sy - 1, cx + shw + 2, sy + 2], fill=STEEL_MID)
    d.rectangle([cx - shw, sy + 2, cx + shw, sy + 3], fill=STEEL_VERY_DARK)

def draw_throat_heatshield(d, cx, y, throat_w, h, scale=1.0):
    """Ablative heat shield band visible at throat for high-pressure engines."""
    thw = int(throat_w / 2 + 2 * scale)
    sh = max(2, int(4 * scale))
    d.rectangle([cx - thw, y + h - sh - 2, cx + thw, y + h - 2],
                fill=ABLATIVE_INNER)


# --- Standard components (unchanged) ---

def draw_chamber(d, cx, y, w, h, scale=1.0):
    inset = max(4, int(8 * scale))
    trap(d, cx, y, w, w + max(2, int(4*scale)), h, STEEL_DARK, outline=STEEL_VERY_DARK)
    trap(d, cx, y+2, w - inset, w - inset + 4, h-2, STEEL_MID)
    chw = int((w+2) / 2)
    ring_h = max(1, int(2 * scale))
    d.rectangle([cx-chw, y+h//2-ring_h, cx+chw, y+h//2+ring_h], fill=STEEL_LIGHT)

def draw_pipes_around_chamber(d, cx, y, cc_h, cc_w, ph_bot_w, n_pipes=2, pw=4, scale=1.0):
    bow_outer = max(10, int(18 * scale))
    bow_inner = max(5, int(8 * scale))
    clamp_len = max(6, int(12 * scale))
    clamp_w = max(2, int(2 * scale))
    for side in [-1, 1]:
        offsets = [(0, bow_outer, 0.3), (int(-14 * scale), bow_inner, 0.15)][:n_pipes]
        for dx, bow, ef in offsets:
            start = (cx + side * int(ph_bot_w/2 + dx - 8), y - 4)
            apex = (cx + side * int(cc_w/2 + bow), y + cc_h * 0.45)
            end = (cx + side * int(cc_w * ef), y + cc_h + 5)
            draw_pipe_pts(d, bezier2(start, apex, end), width=pw)
        cy_b = y + int(cc_h * 0.35)
        d.line([(cx + side*int(cc_w/2), cy_b),
                (cx + side*int(cc_w/2 + clamp_len), cy_b)],
               fill=STEEL_DARK, width=clamp_w)

def draw_throat(d, cx, y, top_w, throat_w, h, scale=1.0):
    trap(d, cx, y, top_w, throat_w, h, STEEL_DARK, outline=STEEL_VERY_DARK)
    thw = int(throat_w / 2)
    band = max(2, int(3 * scale))
    d.rectangle([cx-thw, y+h-band, cx+thw, y+h], fill=STEEL_LIGHT)

def draw_gimbal(d, cx, mount_y, ring_w, ch_y, cc_w, cc_h, gimbal, scale=1.0):
    if gimbal <= 0:
        return
    sw = max(2, int(2 * scale))
    pr = max(2, int(2 * scale))
    for s in [-1, 1]:
        # Anchor top pivot ON mount ring edge, bottom ON chamber edge
        tx = cx + s * int(ring_w / 2 - 2)
        ty = mount_y + 3
        bx = cx + s * int(cc_w / 2)
        by = ch_y + int(cc_h * 0.5)
        d.line([(tx, ty), (bx, by)], fill=STEEL_MID, width=sw)
        d.line([(tx + s, ty), (bx + s, by)], fill=STEEL_HIGHLIGHT, width=1)
        circ(d, tx, ty, pr, fill=STEEL_LIGHT, outline=STEEL_DARK)
        circ(d, bx, by, pr, fill=STEEL_LIGHT, outline=STEEL_DARK)

def draw_bell(d, cx, y, throat_w, exit_w, nozzle_h,
              n_rings=None, vacuum=False, regen_frac=0.4, scale=1.0):
    n_strips = max(30, int(nozzle_h / 4))
    sh = nozzle_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        t1 = (i+1) / n_strips
        ow1 = bell_w(t, throat_w, exit_w)
        ow2 = bell_w(t1, throat_w, exit_w)
        sy = y + t * nozzle_h
        if vacuum and t >= regen_frac:
            r, g, b = 35, 33, 32
        elif vacuum:
            rf = t / regen_frac
            r = int(NOZZLE_UPPER[0] - rf * 18)
            g = int(NOZZLE_UPPER[1] - rf * 18)
            b = int(NOZZLE_UPPER[2] - rf * 18)
        else:
            heat = t ** 2.0
            r = int(STEEL_MID[0] - 8 + heat * 18)
            g = int(STEEL_MID[1] - 8 + heat * 6)
            b = int(STEEL_MID[2] - 8 - heat * 14)
            band = math.sin(t * math.pi * 6) * 3
            r += int(band); g += int(band); b += int(band)
        r = max(0, min(255, r)); g = max(0, min(255, g)); b = max(0, min(255, b))
        trap(d, cx, int(sy), ow1, ow2, int(sh)+1, fill=(r,g,b))
    rh = max(2, int(2 * scale))  # ring half-height
    if n_rings:
        for rf in n_rings:
            ry = int(y + rf * nozzle_h)
            row = bell_w(rf, throat_w, exit_w)
            rohw = int(row / 2)
            d.rectangle([cx-rohw, ry-rh-1, cx+rohw, ry-rh], fill=STEEL_HIGHLIGHT)
            d.rectangle([cx-rohw, ry-rh, cx+rohw, ry+rh], fill=STEEL_LIGHT)
            d.rectangle([cx-rohw+1, ry+rh, cx+rohw-1, ry+rh+1], fill=STEEL_DARK)
    # Vacuum seam (basic — may be overridden by transition_flange)
    if vacuum:
        seam_y = int(y + regen_frac * nozzle_h)
        sw = bell_w(regen_frac, throat_w, exit_w)
        shw = int(sw / 2)
        d.rectangle([cx-shw, seam_y-1, cx+shw, seam_y+rh], fill=STEEL_LIGHT)
        d.rectangle([cx-shw+1, seam_y+rh, cx+shw-1, seam_y+rh+1], fill=STEEL_DARK)
    lip_h = max(3, int(3 * scale))
    ey = int(y + nozzle_h - lip_h)
    ehw = int(exit_w / 2)
    d.rectangle([cx-ehw, ey, cx+ehw, ey+lip_h], fill=ABLATIVE_TIP, outline=STEEL_VERY_DARK)


# ================================================================
# Engine definitions
# ================================================================

BELL_ASPECT_RATIO = 1.15

def compute_height(spec):
    exit_w = spec["exit_w"]
    top_w = spec["top_w"]
    overhead = 0.3 + top_w * 0.38
    if spec.get("twin"):
        single_exit = exit_w * 0.47
        bell_h = single_exit * BELL_ASPECT_RATIO
    else:
        bell_h = exit_w * BELL_ASPECT_RATIO
    return bell_h + overhead

# Feature flags per engine:
#   preburner: 0, 1, or 2 domes
#   fixed_frame: rigid truss instead of gimbal
#   thrust_skirt: structural shell between mount and powerhead
#   heat_fins: expander cycle heat exchanger fins
#   inlet_ducts: turbopump inlet ducts from mount
#   regen_manifold: coolant manifold ring at nozzle top
#   valves: redundant valve body protrusions
#   exhaust_wrap: GG exhaust ring on nozzle (Mammoth)
#   transition_flange: bolted flange at regen/rad boundary
#   extension_seam: deployable nozzle extension joint
#   crossover: ducts between twin chambers
#   throat_shield: ablative heat shield at throat

ENGINE_DEFS = {
    # --- TINY ---
    "engine_hummingbird": {
        "exit_w": 0.85, "top_w": 0.5, "er": 22,
        "vacuum": False, "gimbal": 8, "cycle": "staged",
        "preburner": 1, "hitbox_w": 1, "hitbox_h": 2,
    },
    "engine_gecko": {
        "exit_w": 0.85, "top_w": 0.55, "er": 16,
        "vacuum": False, "gimbal": 4, "cycle": "gasgen",
        "hitbox_w": 1, "hitbox_h": 2,
    },
    "engine_firefly": {
        "exit_w": 0.9, "top_w": 0.25, "er": 80,
        "vacuum": True, "gimbal": 0, "cycle": "expander",
        "heat_fins": True, "fixed_frame": True, "extension_seam": True,
        "hitbox_w": 1, "hitbox_h": 1,
    },
    # --- SMALL ---
    "engine_wolf": {
        "exit_w": 2.6, "top_w": 1.3, "er": 16,
        "vacuum": False, "gimbal": 8, "cycle": "staged",
        "preburner": 1, "regen_manifold": True, "thrust_skirt": True,
        "hitbox_w": 3, "hitbox_h": 4,
    },
    "engine_falcon": {
        "exit_w": 1.7, "top_w": 0.9, "er": 28,
        "vacuum": False, "gimbal": 12, "cycle": "fullflow",
        "preburner": 2, "valves": True,
        "hitbox_w": 2, "hitbox_h": 3,
    },
    "engine_wren": {
        "exit_w": 2.1, "top_w": 0.5, "er": 80,
        "vacuum": True, "gimbal": 3, "cycle": "expander",
        "heat_fins": True, "extension_seam": True,
        "hitbox_w": 3, "hitbox_h": 3,
    },
    "engine_owl": {
        "exit_w": 2.8, "top_w": 0.6, "er": 100,
        "vacuum": True, "gimbal": 4, "cycle": "expander",
        "heat_fins": True, "transition_flange": True,
        "hitbox_w": 3, "hitbox_h": 4,
    },
    "engine_viper": {
        "exit_w": 2.6, "top_w": 1.6, "er": 20,
        "vacuum": False, "gimbal": 0, "cycle": "staged",
        "preburner": 1, "fixed_frame": True, "throat_shield": True,
        "regen_manifold": True, "hitbox_w": 3, "hitbox_h": 4,
    },
    # --- MEDIUM ---
    "engine_bear": {
        "exit_w": 4.2, "top_w": 2.0, "er": 28,
        "vacuum": False, "gimbal": 8, "cycle": "staged",
        "preburner": 1, "valves": True, "thrust_skirt": True,
        "regen_manifold": True, "hitbox_w": 4, "hitbox_h": 6,
    },
    "engine_eagle": {
        "exit_w": 4.6, "top_w": 1.2, "er": 70,
        "vacuum": True, "gimbal": 10.5, "cycle": "fullflow",
        "preburner": 2, "inlet_ducts": True, "transition_flange": True,
        "hitbox_w": 5, "hitbox_h": 6,
    },
    "engine_panther": {
        "exit_w": 4.4, "top_w": 2.2, "er": 18,
        "vacuum": False, "gimbal": 6, "cycle": "staged",
        "twin": True, "preburner": 1, "crossover": True,
        "hitbox_w": 4, "hitbox_h": 4,
    },
    "engine_crane": {
        "exit_w": 3.4, "top_w": 1.6, "er": 35,
        "vacuum": False, "gimbal": 15, "cycle": "staged",
        "preburner": 1, "regen_manifold": True,
        "hitbox_w": 3, "hitbox_h": 5,
    },
    # --- LARGE ---
    "engine_mammoth": {
        "exit_w": 8.0, "top_w": 3.8, "er": 12,
        "vacuum": False, "gimbal": 6, "cycle": "gasgen",
        "thrust_skirt": True, "inlet_ducts": True, "exhaust_wrap": True,
        "hitbox_w": 8, "hitbox_h": 11,
    },
    "engine_whale": {
        "exit_w": 8.2, "top_w": 2.4, "er": 80,
        "vacuum": True, "gimbal": 8, "cycle": "staged",
        "preburner": 1, "inlet_ducts": True, "transition_flange": True,
        "hitbox_w": 8, "hitbox_h": 11,
    },
    "engine_bison": {
        "exit_w": 6.0, "top_w": 3.0, "er": 28,
        "vacuum": False, "gimbal": 12, "cycle": "fullflow",
        "preburner": 2, "valves": True, "inlet_ducts": True,
        "regen_manifold": True, "hitbox_w": 6, "hitbox_h": 8,
    },
    "engine_titan": {
        "exit_w": 8.4, "top_w": 1.8, "er": 60,
        "vacuum": True, "gimbal": 0, "cycle": "staged",
        "preburner": 1, "fixed_frame": True, "inlet_ducts": True,
        "transition_flange": True, "extension_seam": True,
        "hitbox_w": 8, "hitbox_h": 11,
    },
}


# ================================================================
# Main generator
# ================================================================

def generate_engine(name, spec):
    PX = _capped_px(spec["hitbox_w"], spec["hitbox_h"])
    is_twin = spec.get("twin", False)
    exit_w_px = spec["exit_w"] * PX
    top_w_px = spec["top_w"] * PX
    gh = compute_height(spec)
    total_h = gh * PX
    er = spec["er"]
    vacuum = spec["vacuum"]
    gimbal = spec["gimbal"]
    cycle = spec["cycle"]

    throat_w_px = exit_w_px / math.sqrt(er)
    throat_w_px = max(throat_w_px, 12)

    # Canvas sized to hitbox dims — engine drawing centered horizontally, top-aligned
    hitbox_w = spec["hitbox_w"]
    hitbox_h = spec["hitbox_h"]
    canvas_w = hitbox_w * PX
    canvas_h = hitbox_h * PX
    img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = canvas_w // 2
    y = 0

    # Scale detail size proportionally to engine pixel size
    # Reference: ~200px exit width (small engine like Wolf)
    scale = max(0.4, exit_w_px / 200.0)
    px_scale = PX / 90.0  # Resolution multiplier (for PX-dependent constants)
    is_tiny = spec["top_w"] < 0.67  # Grid-based, not pixel-based
    is_large = exit_w_px > 400
    bolt_r = max(2, int(2 * scale))

    # ---- MOUNT ----
    mount_h = max(6, int(total_h * 0.03))
    ring_w = top_w_px + (4 if is_tiny else 12)
    n_bolts = max(2, int(ring_w / (bolt_r * 6)))
    draw_mount(d, cx, y, ring_w, mount_h, n_bolts=n_bolts, bolt_r=bolt_r, scale=scale)
    mount_top_y = y
    y += mount_h

    # ---- THRUST SKIRT (large/medium workhorse engines) ----
    if spec.get("thrust_skirt"):
        skirt_h = max(6, int(total_h * 0.025))
        draw_thrust_skirt(d, cx, y, ring_w, top_w_px + 4, skirt_h)
        y += skirt_h

    # ---- INLET DUCTS (drawn behind powerhead) ----
    ph_top_y = y  # save for later

    # ---- POWERHEAD ----
    ph_h = int(total_h * (0.10 if is_tiny else 0.12))
    ph_top_w = top_w_px
    ph_widen = max(6, int(12 * scale))
    ph_bot_w = top_w_px + ph_widen
    # Twin engines need wider powerhead to span both chambers
    if is_twin:
        ph_bot_w = max(ph_bot_w, exit_w_px * 0.58)

    exhaust_stubs = cycle in ("staged", "fullflow")
    gas_gen = cycle == "gasgen"
    draw_powerhead(d, cx, y, ph_top_w, ph_bot_w, ph_h,
                   exhaust_stubs=exhaust_stubs, gas_gen_dump=gas_gen, scale=scale)

    # ---- PREBURNER DOMES (on top of powerhead) ----
    n_domes = spec.get("preburner", 0)
    if n_domes > 0:
        draw_preburner_domes(d, cx, y, ph_top_w, n=n_domes, px_scale=px_scale)

    # ---- HEAT EXCHANGER FINS (expander cycle) ----
    if spec.get("heat_fins"):
        draw_heat_exchanger_fins(d, cx, y, ph_bot_w, ph_h, scale=scale)

    # ---- REDUNDANT VALVES (crew-rated) ----
    if spec.get("valves"):
        draw_redundant_valves(d, cx, y, ph_bot_w, ph_h, scale=scale)

    # ---- INLET DUCTS (from mount into powerhead) ----
    if spec.get("inlet_ducts"):
        draw_turbopump_inlet_ducts(d, cx, mount_top_y, mount_h, ring_w,
                                    ph_top_y, ph_top_w, scale=scale)

    y += ph_h

    if is_twin:
        # ---- TWIN BELL (Panther) ----
        # Each bell is ~40% of total width, spaced so they don't overlap
        single_exit = exit_w_px * 0.40
        single_throat = single_exit / math.sqrt(er)
        single_throat = max(single_throat, 12)

        cc_h = int(total_h * 0.06)
        cc_w = max(single_throat + 10, ph_bot_w * 0.28)
        # Spacing must be > half exit width to avoid overlap, plus a gap
        bell_spacing = single_exit / 2 + max(6, int(6 * scale))

        pw = max(3, int(4 * scale))

        for s in [-1, 1]:
            bcx = cx + int(s * bell_spacing)
            draw_chamber(d, bcx, y, cc_w, cc_h, scale=scale)
            pts = bezier2(
                (cx + s * int(ph_bot_w * 0.2), y - 4),
                (bcx + s * int(cc_w/2 + 8 * scale), y + cc_h * 0.3),
                (bcx, y + cc_h + 3))
            draw_pipe_pts(d, pts, width=pw)

        # Crossover ducts between twin chambers
        if spec.get("crossover"):
            draw_crossover_ducts(d, cx, y, bell_spacing, cc_h, pw=pw)

        draw_gimbal(d, cx, mount_top_y, ring_w, y, ph_bot_w * 0.7,
                    cc_h, gimbal, scale=scale)
        y += cc_h

        for s in [-1, 1]:
            bcx = cx + int(s * bell_spacing)
            th_h = max(6, int(total_h * 0.025))
            draw_throat(d, bcx, y, cc_w + 4, single_throat, th_h, scale=scale)
            nozzle_h = int(total_h - (y - PAD) - th_h - 3)
            n_rings = [0.12, 0.28, 0.44, 0.60, 0.78, 0.92]
            draw_bell(d, bcx, y + th_h, single_throat, single_exit,
                      nozzle_h, n_rings=n_rings, vacuum=vacuum, scale=scale)

    else:
        # ---- SINGLE BELL ----
        cc_h = int(total_h * (0.07 if is_tiny else 0.08))
        cc_w_ratio = 0.65 if cycle in ("staged", "fullflow") else 0.75
        cc_w = max(throat_w_px + 10, ph_bot_w * cc_w_ratio)

        draw_chamber(d, cx, y, cc_w, cc_h, scale=scale)

        # Pipes around chamber
        pw = max(2, int(4 * scale))
        n_pipes = 2 if cycle in ("fullflow", "staged") else 1
        bow = max(8, int(14 * scale))  # how far pipes bow out from chamber
        if n_pipes == 2 and not is_tiny:
            draw_pipes_around_chamber(d, cx, y, cc_h, cc_w, ph_bot_w,
                                      n_pipes=2, pw=pw, scale=scale)
        else:
            for side in [-1, 1]:
                pts = bezier2(
                    (cx + side * int(ph_bot_w/2 - 4), y - 3),
                    (cx + side * int(cc_w/2 + bow), y + cc_h * 0.45),
                    (cx + side * int(cc_w * 0.2), y + cc_h + 3))
                draw_pipe_pts(d, pts, width=pw)

        # Manifold pipe for fullflow
        if cycle == "fullflow" and not is_tiny:
            mhw = int(cc_w * 0.45)
            mh = max(3, int(4 * scale))
            my = y + int(cc_h * 0.25)
            d.rectangle([cx-mhw, my, cx+mhw, my+mh], fill=PIPE_COLOR)
            d.rectangle([cx-mhw, my, cx+mhw, my+1], fill=PIPE_HIGHLIGHT)

        # Turbopump exhaust manifold for large gasgen
        if cycle == "gasgen" and not is_tiny:
            mhw = int(cc_w * 0.5)
            mh = max(3, int(5 * scale))
            my = y + int(cc_h * 0.3)
            d.rectangle([cx-mhw, my, cx+mhw, my+mh], fill=PIPE_COLOR)
            d.rectangle([cx-mhw, my, cx+mhw, my+2], fill=PIPE_HIGHLIGHT)

        # Gimbal actuators OR fixed mount frame
        if spec.get("fixed_frame") or gimbal == 0:
            draw_fixed_mount_frame(d, cx, mount_top_y, ring_w, y, cc_w, cc_h,
                                    scale=scale)
        else:
            draw_gimbal(d, cx, mount_top_y, ring_w, y, cc_w, cc_h,
                        gimbal, scale=scale)
        y += cc_h

        # Throat
        throat_h = max(6, int(total_h * 0.03))
        # Throat heat shield for high-pressure engines
        if spec.get("throat_shield"):
            draw_throat_heatshield(d, cx, y, throat_w_px, throat_h, scale=scale)
        draw_throat(d, cx, y, cc_w + 4, throat_w_px, throat_h, scale=scale)
        y += throat_h

        # Regen cooling manifold ring at nozzle top
        if spec.get("regen_manifold"):
            draw_regen_manifold(d, cx, y, throat_w_px, scale=scale)

        # Bell
        nozzle_h = int(total_h - (y - PAD) - 3)
        bell_y = y

        # Ring positions (spacing scaled with resolution)
        ring_spacing = px_scale
        if vacuum:
            regen_frac = 0.35 if er > 60 else 0.45
            n_per = max(2, int(regen_frac * nozzle_h / (40 * ring_spacing)))
            ring_pos = [regen_frac * (i+1) / (n_per+1) for i in range(n_per)]
        else:
            n_r = max(3, int(nozzle_h / (35 * ring_spacing)))
            ring_pos = [(i+1) / (n_r+1) for i in range(n_r)]

        regen_frac = 0.35 if vacuum and er > 60 else 0.45

        draw_bell(d, cx, y, throat_w_px, exit_w_px, nozzle_h,
                  n_rings=ring_pos, vacuum=vacuum, regen_frac=regen_frac,
                  scale=scale)

        # Post-bell overlays
        if spec.get("exhaust_wrap"):
            draw_turbine_exhaust_wrap(d, cx, bell_y, nozzle_h,
                                      throat_w_px, exit_w_px, frac=0.12, scale=scale)

        if spec.get("transition_flange") and vacuum:
            draw_transition_flange(d, cx, bell_y, nozzle_h,
                                    throat_w_px, exit_w_px, regen_frac, scale=scale)

        if spec.get("extension_seam") and vacuum:
            draw_nozzle_extension_seam(d, cx, bell_y, nozzle_h,
                                        throat_w_px, exit_w_px, frac=0.55)

    return img


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    output_dir = os.path.join(project_root, "data", "sprites", "engines")
    os.makedirs(output_dir, exist_ok=True)

    for name, spec in ENGINE_DEFS.items():
        img = generate_engine(name, spec)
        out = os.path.join(output_dir, f"{name}.png")
        img.save(out)

        er = spec["er"]
        throat = spec["exit_w"] / math.sqrt(er)
        gh = compute_height(spec)
        features = [k for k in ("preburner", "fixed_frame", "thrust_skirt",
                                "heat_fins", "inlet_ducts", "regen_manifold",
                                "valves", "exhaust_wrap", "transition_flange",
                                "extension_seam", "crossover", "throat_shield",
                                "twin") if spec.get(k)]
        print(f"  {name:25s}  {img.size[0]:4d}x{img.size[1]:4d}  "
              f"[{', '.join(features)}]")

    print(f"\nGenerated {len(ENGINE_DEFS)} sprites in {output_dir}")
