#!/usr/bin/env python3
"""
Sprite generator for the Viper engine.

Viper: Maximum thrust density kerolox engine. Oxidizer-rich staged combustion
at extreme chamber pressure. Fixed nozzle. Compact, heavy, no gimbal.

Stats: Small size, 0.7t, 1100kN vac, 325s ISP, 2.8w x 3.0h grid, top_width 1.6

Visual goal: Recognizable silhouette. Compact powerhead, small chamber,
prominent bell nozzle with pipes wrapping around. All steel/gray tones.
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

# --- Palette (all steel/gray, no orange) ---
STEEL_DARK = (48, 50, 55)
STEEL_MID = (80, 84, 92)
STEEL_LIGHT = (120, 125, 135)
STEEL_HIGHLIGHT = (155, 160, 170)
STEEL_VERY_DARK = (32, 34, 38)
INTERIOR = (20, 18, 16)
ABLATIVE_TIP = (70, 62, 50)
PIPE_COLOR = (65, 68, 75)
PIPE_HIGHLIGHT = (100, 105, 115)


def trap(draw, cx, y, tw, bw, h, fill, outline=None):
    """Trapezoid: tw=top width, bw=bottom width, h=height."""
    draw.polygon([
        (cx - tw / 2, y), (cx + tw / 2, y),
        (cx + bw / 2, y + h), (cx - bw / 2, y + h),
    ], fill=fill, outline=outline)


def circ(draw, cx, cy, r, **kw):
    draw.ellipse([cx - r, cy - r, cx + r, cy + r], **kw)


def bell_w(frac, w_throat, w_exit):
    """Bell curve width at fraction 0..1 from throat to exit."""
    return w_throat + (w_exit - w_throat) * (1 - (1 - frac) ** 1.7)


def generate_viper():
    img = Image.new("RGBA", (IMG_W, IMG_H), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    total_h = 3.0 * PX_PER_GRID  # 270
    mount_w = 1.6 * PX_PER_GRID  # 144 (top attachment width)
    exit_w = 2.4 * PX_PER_GRID   # narrower bell than before (~216)

    y = PAD  # running y cursor

    # ============================================================
    # 1. MOUNT RING (thin, ~4% of height)
    # ============================================================
    mount_h = int(total_h * 0.04)
    ring_w = mount_w + 10
    trap(d, CX, y, ring_w, ring_w, mount_h, STEEL_MID, outline=STEEL_DARK)
    # Top edge highlight
    d.line([(CX - int(ring_w/2) + 2, y + 1), (CX + int(ring_w/2) - 2, y + 1)],
           fill=STEEL_HIGHLIGHT, width=1)
    # Bolts
    for i in range(5):
        bx = CX - int(ring_w/2) + 12 + i * int((ring_w - 24) / 4)
        by = y + mount_h // 2
        circ(d, bx, by, 3, fill=STEEL_HIGHLIGHT)
        circ(d, bx, by, 2, fill=STEEL_DARK)
    y += mount_h

    # ============================================================
    # 2. POWERHEAD (compact, ~16% — smaller than before)
    #    This is where the turbopumps and preburner live.
    #    Slightly wider than mount, boxy but short.
    # ============================================================
    ph_h = int(total_h * 0.16)
    ph_top_w = mount_w
    ph_bot_w = mount_w + 20

    # Main housing
    trap(d, CX, y, ph_top_w, ph_bot_w, ph_h, STEEL_MID, outline=STEEL_DARK)

    # Horizontal bands (machined sections)
    for frac in [0.3, 0.65]:
        by = y + int(ph_h * frac)
        bw = ph_top_w + (ph_bot_w - ph_top_w) * frac
        hw = int(bw / 2)
        d.rectangle([CX - hw, by, CX + hw, by + 2], fill=STEEL_LIGHT)
        d.rectangle([CX - hw, by + 2, CX + hw, by + 3], fill=STEEL_DARK)

    # Side bumps (turbine exhaust housings)
    bump_y = y + int(ph_h * 0.4)
    bump_r = int(ph_h * 0.18)
    for side in [-1, 1]:
        bx = CX + side * int(ph_bot_w / 2 + bump_r - 4)
        circ(d, bx, bump_y, bump_r + 2, fill=STEEL_MID, outline=STEEL_DARK)
        circ(d, bx, bump_y, bump_r - 3, fill=INTERIOR)

    # Left edge highlight
    d.line([(CX - int(ph_top_w/2) + 2, y + 2),
            (CX - int(ph_bot_w/2) + 2, y + ph_h - 2)],
           fill=STEEL_LIGHT, width=1)

    y += ph_h

    # ============================================================
    # 3. COMBUSTION CHAMBER (small, ~10%)
    #    High pressure = compact. Pipes wrap around it.
    # ============================================================
    cc_h = int(total_h * 0.10)
    cc_w = ph_bot_w * 0.65  # noticeably narrower than powerhead

    # Main chamber cylinder
    trap(d, CX, y, cc_w, cc_w + 4, cc_h, STEEL_DARK, outline=STEEL_VERY_DARK)

    # Inner body (lighter)
    trap(d, CX, y + 2, cc_w - 10, cc_w - 6, cc_h - 2, STEEL_MID)

    # Reinforcement ring at middle
    mid_y = y + cc_h // 2
    chw = int((cc_w + 2) / 2)
    d.rectangle([CX - chw, mid_y - 2, CX + chw, mid_y + 2], fill=STEEL_LIGHT)

    # --- Pipes curving from powerhead around the chamber into the throat ---
    # Outer pipe: exits powerhead wide, arcs around chamber, tucks into throat
    # Inner pipe: tighter curve, from powerhead center-ish down to chamber wall
    for side in [-1, 1]:
        # Outer feed pipe: starts at powerhead edge, bows out around chamber,
        # curves back in to meet the throat
        pipe_start_x = CX + side * int(ph_bot_w / 2 - 8)  # from powerhead bottom edge
        pipe_start_y = y - 4
        pipe_apex_x = CX + side * int(cc_w / 2 + 18)  # bows outward
        pipe_apex_y = y + cc_h * 0.4
        pipe_end_x = CX + side * int(cc_w * 0.3)  # tucks back into throat area
        pipe_end_y = y + cc_h + 6

        # Build bezier-ish curve with line segments
        n_seg = 20
        pts_outer = []
        for s in range(n_seg + 1):
            t = s / n_seg
            # Quadratic bezier: P0->P1->P2
            x = (1-t)**2 * pipe_start_x + 2*(1-t)*t * pipe_apex_x + t**2 * pipe_end_x
            yy = (1-t)**2 * pipe_start_y + 2*(1-t)*t * pipe_apex_y + t**2 * pipe_end_y
            pts_outer.append((int(x), int(yy)))

        # Draw outer pipe
        for k in range(len(pts_outer) - 1):
            d.line([pts_outer[k], pts_outer[k+1]], fill=PIPE_COLOR, width=5)
        # Highlight
        for k in range(len(pts_outer) - 1):
            # Offset highlight toward center
            hx1 = pts_outer[k][0] - side * 1
            hy1 = pts_outer[k][1]
            hx2 = pts_outer[k+1][0] - side * 1
            hy2 = pts_outer[k+1][1]
            d.line([(hx1, hy1), (hx2, hy2)], fill=PIPE_HIGHLIGHT, width=1)

        # Inner feed pipe: starts closer to center, gentler curve, meets chamber wall
        pipe2_start_x = CX + side * int(ph_bot_w / 2 - 22)
        pipe2_start_y = y - 2
        pipe2_apex_x = CX + side * int(cc_w / 2 + 8)
        pipe2_apex_y = y + cc_h * 0.55
        pipe2_end_x = CX + side * int(cc_w * 0.15)
        pipe2_end_y = y + cc_h + 3

        pts_inner = []
        for s in range(n_seg + 1):
            t = s / n_seg
            x = (1-t)**2 * pipe2_start_x + 2*(1-t)*t * pipe2_apex_x + t**2 * pipe2_end_x
            yy = (1-t)**2 * pipe2_start_y + 2*(1-t)*t * pipe2_apex_y + t**2 * pipe2_end_y
            pts_inner.append((int(x), int(yy)))

        for k in range(len(pts_inner) - 1):
            d.line([pts_inner[k], pts_inner[k+1]], fill=PIPE_COLOR, width=4)
        for k in range(len(pts_inner) - 1):
            hx1 = pts_inner[k][0] - side * 1
            hy1 = pts_inner[k][1]
            hx2 = pts_inner[k+1][0] - side * 1
            hy2 = pts_inner[k+1][1]
            d.line([(hx1, hy1), (hx2, hy2)], fill=PIPE_HIGHLIGHT, width=1)

        # Bracket clamp at the widest point of outer pipe
        clamp_idx = n_seg // 3
        clamp_pt = pts_outer[clamp_idx]
        d.line([(CX + side * int(cc_w / 2), clamp_pt[1]),
                (clamp_pt[0], clamp_pt[1])],
               fill=STEEL_DARK, width=2)

    y += cc_h

    # ============================================================
    # 4. THROAT (~4%)
    # ============================================================
    throat_h = int(total_h * 0.04)
    throat_w = cc_w * 0.55

    # Converging section
    trap(d, CX, y, cc_w + 4, throat_w, throat_h, STEEL_DARK, outline=STEEL_VERY_DARK)

    # Throat band
    thw = int(throat_w / 2)
    d.rectangle([CX - thw, y + throat_h - 3, CX + thw, y + throat_h],
                fill=STEEL_LIGHT)
    y += throat_h

    # ============================================================
    # 5. NOZZLE BELL (~66% — the defining feature)
    #    Moderate expansion ratio. Clean walls with heat banding,
    #    stiffening rings, and a left-edge highlight for depth.
    # ============================================================
    nozzle_h = int(total_h * 0.66)
    nozzle_top_y = y

    WALL_TOP = 14
    WALL_BOT = 6

    n_strips = 50
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

        # Outer wall: smooth gradient with subtle heat banding
        heat = t ** 2.0
        # Base color shifts warmer toward exit
        base_r = STEEL_MID[0] - 8 + heat * 18
        base_g = STEEL_MID[1] - 8 + heat * 6
        base_b = STEEL_MID[2] - 8 - heat * 14

        # Subtle horizontal banding (alternating slightly lighter/darker zones)
        band = math.sin(t * math.pi * 6) * 4  # gentle wave
        base_r += band
        base_g += band
        base_b += band

        oc = (max(0, min(255, int(base_r))),
              max(0, min(255, int(base_g))),
              max(0, min(255, int(base_b))))

        trap(d, CX, int(sy), ow1, ow2, int(sh), fill=oc)

        # Interior
        if iw1 > 4 and iw2 > 4:
            ir = int(INTERIOR[0] + heat * 12)
            ig = int(INTERIOR[1] + heat * 5)
            ib = int(INTERIOR[2])
            trap(d, CX, int(sy), iw1, iw2, int(sh),
                 fill=(ir, ig, ib))

    # Stiffening rings (the main structural detail on the bell)
    for rf in [0.10, 0.25, 0.40, 0.55, 0.70, 0.85]:
        ry = int(nozzle_top_y + rf * nozzle_h)
        row = bell_w(rf, throat_w, exit_w)
        wt = WALL_TOP + (WALL_BOT - WALL_TOP) * rf
        riw = row - wt * 2

        rohw = int(row / 2)
        rihw = int(riw / 2)

        # Top edge highlight
        d.rectangle([CX - rohw, ry - 2, CX + rohw, ry - 1], fill=STEEL_HIGHLIGHT)
        # Ring body
        d.rectangle([CX - rohw, ry - 1, CX + rohw, ry + 2], fill=STEEL_LIGHT)
        # Interior clear
        if rihw > 2:
            d.rectangle([CX - rihw, ry - 1, CX + rihw, ry + 1],
                        fill=(28, 26, 22))
        # Shadow below
        d.rectangle([CX - rohw + 1, ry + 2, CX + rohw - 1, ry + 4],
                    fill=STEEL_DARK)

    # Exit lip
    ey = int(nozzle_top_y + nozzle_h - 4)
    ehw = int(exit_w / 2)
    d.rectangle([CX - ehw, ey, CX + ehw, ey + 4],
                fill=ABLATIVE_TIP, outline=STEEL_VERY_DARK)
    eihw = int((exit_w - WALL_BOT * 2) / 2)
    d.rectangle([CX - eihw, ey + 1, CX + eihw, ey + 3], fill=INTERIOR)

    return img


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    output_dir = os.path.join(project_root, "data", "sprites", "engines")
    os.makedirs(output_dir, exist_ok=True)

    out = os.path.join(output_dir, "engine_viper.png")
    img = generate_viper()
    img.save(out)
    print(f"Saved: {out}  ({img.size[0]}x{img.size[1]} px)")
