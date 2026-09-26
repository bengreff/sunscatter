#!/usr/bin/env python3
"""
Sprite generator for the Owl engine.

Owl stats (from engines.ron):
  Size: Small (3 grid = 1.5m)
  Mass: 0.28t (lightweight)
  Thrust: 180 kN vac / 110 kN ASL
  ISP: 460s vac / 370s ASL (highest small engine — vacuum optimized)
  Gimbal: 4.0°
  Propellant: Hydrolox, expander cycle
  Grid dims: 2.9w × 3.0h, top_width 0.6
  Description: "Hydrolox upper stage with oversized vacuum bell. Expander
  cycle uses combustion heat to drive turbopumps."

Visual inspiration: RL-10. Tiny powerhead, enormous thin-walled vacuum bell
with very high expansion ratio. Lightweight, elegant. Expander cycle = simpler
external plumbing than staged combustion. Gimbal actuators visible.
"""

from PIL import Image, ImageDraw
import math
import os

# --- Canvas (hitbox dims: 3 wide x 4 tall) ---
PX_PER_GRID = 90
HITBOX_W = 3
HITBOX_H = 4
IMG_W = HITBOX_W * PX_PER_GRID
IMG_H = HITBOX_H * PX_PER_GRID
CX = IMG_W // 2
PAD = 0

# --- Palette ---
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)
ABLATIVE_TIP = (70, 62, 50)
PIPE_COLOR = (65, 68, 75)
PIPE_HIGHLIGHT = (100, 105, 115)

# Nozzle extension is thinner, lighter material (radiatively cooled)
NOZZLE_UPPER = (85, 88, 96)     # regen-cooled upper section
NOZZLE_LOWER = (60, 55, 52)     # radiatively cooled extension, heat-tinted


def trap(draw, cx, y, tw, bw, h, fill, outline=None):
    draw.polygon([
        (cx - tw / 2, y), (cx + tw / 2, y),
        (cx + bw / 2, y + h), (cx - bw / 2, y + h),
    ], fill=fill, outline=outline)


def circ(draw, cx, cy, r, **kw):
    draw.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)


def bell_w(frac, w_throat, w_exit):
    """Vacuum bell: expands more gradually then flares at the end."""
    return w_throat + (w_exit - w_throat) * (1 - (1 - frac) ** 2.2)


def generate_owl():
    img = Image.new("RGBA", (IMG_W, IMG_H), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    total_h = 3.0 * PX_PER_GRID   # 270
    mount_w = 0.6 * PX_PER_GRID   # 54 — very narrow top
    exit_w = 2.9 * PX_PER_GRID    # 261 — wide vacuum bell

    y = PAD

    # ============================================================
    # 1. MOUNT RING (thin, narrow)
    # ============================================================
    mount_h = int(total_h * 0.03)
    ring_w = mount_w + 8
    trap(d, CX, y, ring_w, ring_w, mount_h, STEEL_MID, outline=STEEL_DARK)
    # Bolts
    hw = int(ring_w / 2)
    for i in range(3):
        bx = CX - hw + 8 + i * int((ring_w - 16) / 2)
        circ(d, bx, y + mount_h // 2, 2, fill=STEEL_HIGHLIGHT)
        circ(d, bx, y + mount_h // 2, 1, fill=STEEL_DARK)
    y += mount_h

    # ============================================================
    # 2. POWERHEAD (tiny — expander cycle is compact and simple)
    # ============================================================
    ph_h = int(total_h * 0.10)
    ph_top_w = mount_w
    ph_bot_w = mount_w + 12

    # Main housing
    trap(d, CX, y, ph_top_w, ph_bot_w, ph_h, STEEL_MID, outline=STEEL_DARK)

    # Horizontal band
    band_y = y + int(ph_h * 0.5)
    bhw = int((ph_top_w + (ph_bot_w - ph_top_w) * 0.5) / 2)
    d.rectangle([CX - bhw, band_y, CX + bhw, band_y + 2], fill=STEEL_LIGHT)

    # Left highlight
    d.line([(CX - int(ph_top_w / 2) + 1, y + 2),
            (CX - int(ph_bot_w / 2) + 1, y + ph_h - 2)],
           fill=STEEL_LIGHT, width=1)

    y += ph_h

    # ============================================================
    # 3. COMBUSTION CHAMBER (small, narrow cylinder)
    # ============================================================
    cc_h = int(total_h * 0.08)
    cc_w = ph_bot_w + 4

    trap(d, CX, y, cc_w, cc_w, cc_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    # Inner lighter body
    trap(d, CX, y + 2, cc_w - 8, cc_w - 8, cc_h - 2, STEEL_MID)

    # Reinforcement ring
    chw = int(cc_w / 2)
    d.rectangle([CX - chw, y + cc_h // 2 - 1, CX + chw, y + cc_h // 2 + 1],
                fill=STEEL_LIGHT)

    # Curved feed pipes from powerhead around chamber
    for side in [-1, 1]:
        n_seg = 16
        # Pipe curves from powerhead side, bows out, tucks into bottom of chamber
        p_start_x = CX + side * int(ph_bot_w / 2 - 4)
        p_start_y = y - 3
        p_apex_x = CX + side * int(cc_w / 2 + 12)
        p_apex_y = y + cc_h * 0.45
        p_end_x = CX + side * int(cc_w * 0.2)
        p_end_y = y + cc_h + 3

        pts = []
        for s in range(n_seg + 1):
            t = s / n_seg
            px = (1-t)**2 * p_start_x + 2*(1-t)*t * p_apex_x + t**2 * p_end_x
            py = (1-t)**2 * p_start_y + 2*(1-t)*t * p_apex_y + t**2 * p_end_y
            pts.append((int(px), int(py)))

        for k in range(len(pts) - 1):
            d.line([pts[k], pts[k+1]], fill=PIPE_COLOR, width=4)
        for k in range(len(pts) - 1):
            d.line([(pts[k][0] - side, pts[k][1]),
                    (pts[k+1][0] - side, pts[k+1][1])],
                   fill=PIPE_HIGHLIGHT, width=1)

    # Gimbal actuators (small angled struts from mount area to chamber sides)
    for side in [-1, 1]:
        act_top_x = CX + side * int(ring_w / 2 + 6)
        act_top_y = y - ph_h - mount_h + 4
        act_bot_x = CX + side * int(cc_w / 2 + 2)
        act_bot_y = y + int(cc_h * 0.6)
        # Actuator body
        d.line([(act_top_x, act_top_y), (act_bot_x, act_bot_y)],
               fill=STEEL_MID, width=3)
        d.line([(act_top_x + side, act_top_y), (act_bot_x + side, act_bot_y)],
               fill=STEEL_HIGHLIGHT, width=1)
        # Pivot dots
        circ(d, act_top_x, act_top_y, 3, fill=STEEL_LIGHT, outline=STEEL_DARK)
        circ(d, act_bot_x, act_bot_y, 3, fill=STEEL_LIGHT, outline=STEEL_DARK)

    y += cc_h

    # ============================================================
    # 4. THROAT (narrow constriction)
    # ============================================================
    throat_h = int(total_h * 0.03)
    throat_w = cc_w * 0.5

    trap(d, CX, y, cc_w, throat_w, throat_h, STEEL_DARK, outline=STEEL_VERY_DARK)
    thw = int(throat_w / 2)
    d.rectangle([CX - thw, y + throat_h - 2, CX + thw, y + throat_h],
                fill=STEEL_LIGHT)
    y += throat_h

    # ============================================================
    # 5. NOZZLE BELL (~76% — enormous vacuum bell)
    #    High expansion ratio. Upper portion regen-cooled (lighter),
    #    lower extension radiatively cooled (darker, heat-tinted).
    #    Thinner walls than Viper. Elegant, lightweight.
    # ============================================================
    nozzle_h = int(total_h * 0.76)
    nozzle_top_y = y

    # Transition point: upper 40% is regen-cooled, lower 60% is rad-cooled
    REGEN_FRAC = 0.4

    WALL_TOP = 10
    WALL_BOT = 4

    n_strips = 55
    strip_h = nozzle_h / n_strips

    for i in range(n_strips):
        t = i / n_strips
        t1 = (i + 1) / n_strips

        ow1 = bell_w(t, throat_w, exit_w)
        ow2 = bell_w(t1, throat_w, exit_w)

        wt = WALL_TOP + (WALL_BOT - WALL_TOP) * t
        iw1 = max(0, ow1 - wt * 2)
        iw2 = max(0, ow2 - wt * 2)

        sy = nozzle_top_y + t * nozzle_h
        sh = strip_h + 1

        # Color transitions from regen (lighter steel) to rad-cooled (darker, warm)
        if t < REGEN_FRAC:
            # Upper regen section: lighter steel
            rf = t / REGEN_FRAC
            r = int(NOZZLE_UPPER[0] + rf * (NOZZLE_LOWER[0] - NOZZLE_UPPER[0]) * 0.3)
            g = int(NOZZLE_UPPER[1] + rf * (NOZZLE_LOWER[1] - NOZZLE_UPPER[1]) * 0.3)
            b = int(NOZZLE_UPPER[2] + rf * (NOZZLE_LOWER[2] - NOZZLE_UPPER[2]) * 0.3)
        else:
            # Lower rad-cooled section: flat very dark grey
            r, g, b = 35, 33, 32

        # Gentle banding
        band = math.sin(t * math.pi * 5) * 3
        r = max(0, min(255, int(r + band)))
        g = max(0, min(255, int(g + band)))
        b = max(0, min(255, int(b + band)))

        trap(d, CX, int(sy), ow1, ow2, int(sh), fill=(r, g, b))

    # Transition seam between regen and rad-cooled sections
    seam_y = int(nozzle_top_y + REGEN_FRAC * nozzle_h)
    seam_w = bell_w(REGEN_FRAC, throat_w, exit_w)
    shw = int(seam_w / 2)
    d.rectangle([CX - shw, seam_y - 1, CX + shw, seam_y + 2], fill=STEEL_LIGHT)
    sihw = int((seam_w - (WALL_TOP + (WALL_BOT - WALL_TOP) * REGEN_FRAC) * 2) / 2)
    if sihw > 2:
        d.rectangle([CX - sihw, seam_y, CX + sihw, seam_y + 1],
                    fill=(28, 26, 22))
    d.rectangle([CX - shw + 1, seam_y + 2, CX + shw - 1, seam_y + 3],
                fill=STEEL_DARK)

    # Stiffening rings (only in the upper regen section)
    for rf in [0.12, 0.25, 0.35]:
        ry = int(nozzle_top_y + rf * nozzle_h)
        row = bell_w(rf, throat_w, exit_w)
        wt = WALL_TOP + (WALL_BOT - WALL_TOP) * rf
        riw = row - wt * 2
        rohw = int(row / 2)
        rihw = int(riw / 2)

        d.rectangle([CX - rohw, ry - 1, CX + rohw, ry + 2], fill=STEEL_LIGHT)
        if rihw > 2:
            d.rectangle([CX - rihw, ry, CX + rihw, ry + 1],
                        fill=(28, 26, 22))
        d.rectangle([CX - rohw + 1, ry + 2, CX + rohw - 1, ry + 3],
                    fill=STEEL_DARK)

    # Exit lip (thin — lightweight)
    ey = int(nozzle_top_y + nozzle_h - 3)
    ehw = int(exit_w / 2)
    d.rectangle([CX - ehw, ey, CX + ehw, ey + 3],
                fill=ABLATIVE_TIP, outline=STEEL_VERY_DARK)
    eihw = int((exit_w - WALL_BOT * 2) / 2)
    d.rectangle([CX - eihw, ey + 1, CX + eihw, ey + 2], fill=INTERIOR)

    return img


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    output_dir = os.path.join(project_root, "data", "sprites", "engines")
    os.makedirs(output_dir, exist_ok=True)

    out = os.path.join(output_dir, "engine_owl.png")
    img = generate_owl()
    img.save(out)
    print(f"Saved: {out}  ({img.size[0]}x{img.size[1]} px)")
