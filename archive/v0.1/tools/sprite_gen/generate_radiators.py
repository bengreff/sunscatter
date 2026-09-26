#!/usr/bin/env python3
"""
Generate sprites for radiator parts (12 variants).

Three technology tiers × four sizes:
  Heat Pipe   (1,200 K) — carbon-carbon panels, cherry-red glow, sodium heat pipes
  Droplet     (2,500 K) — magnetic emitter/collector, molten-metal curtain, amber glow
  Phononic    (6,000 K) — metamaterial lattice, blue-white O-star glow

All radiators are 1 grid wide × {2,4,8,16} grid tall for Tiny/Small/Medium/Large.
All sizes use the same detail density as the tiny variant — larger sizes tile the
panel section vertically so the visual style is identical at every zoom level.
PX=360 px/grid.
"""

from PIL import Image, ImageDraw
import math
import os

# ================================================================
# Constants
# ================================================================
PX = 360
PAD = 0

# Reference dimensions for detail sizing — matches the tiny (1x2) variant.
# All generators use these fixed values so detail density is size-independent.
REF_W = PX        # 360
REF_H = 2 * PX    # 720
REF_SCALE = REF_H / 600.0  # ~1.2

# ================================================================
# Palette (shared with other generators)
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

# Heat Pipe tier — warm industrial carbon-carbon
HP_BODY = (65, 60, 55)
HP_DARK = (45, 42, 38)
HP_LIGHT = (90, 85, 78)
HP_HIGHLIGHT = (110, 105, 96)
HP_GLOW = (180, 70, 40)
HP_GLOW_BRIGHT = (220, 100, 50)
HP_PIPE = (75, 68, 58)
HP_PIPE_LIGHT = (100, 92, 80)

# Droplet tier — silvery with amber/orange glow
DR_BODY = (70, 75, 85)
DR_DARK = (45, 48, 58)
DR_LIGHT = (100, 108, 120)
DR_HIGHLIGHT = (130, 138, 150)
DR_GLOW = (220, 160, 50)
DR_GLOW_BRIGHT = (245, 190, 70)
DR_EMITTER = (55, 60, 72)
DR_COLLECTOR = (60, 55, 50)

# Phononic tier — exotic blue-white
PH_BODY = (50, 60, 80)
PH_DARK = (30, 38, 55)
PH_LIGHT = (75, 90, 120)
PH_HIGHLIGHT = (100, 118, 155)
PH_GLOW = (160, 190, 240)
PH_GLOW_BRIGHT = (200, 220, 255)
PH_LATTICE = (60, 75, 105)
PH_LATTICE_LIGHT = (90, 110, 150)


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
# Heat Pipe Panel generator
# ================================================================

def _draw_heatpipe_panel_tile(d, cx, y, panel_w, tile_h):
    """Draw one repeatable tile of heat pipe panel detail."""
    s = REF_SCALE
    phw = int(panel_w / 2)

    rect(d, cx, y, panel_w, tile_h, HP_BODY, outline=HP_DARK)

    d.line([(cx - phw + 2, y + 3), (cx - phw + 2, y + tile_h - 3)],
           fill=HP_HIGHLIGHT, width=max(1, int(2 * s)))

    spine_w = max(6, int(REF_W * 0.12))
    rect(d, cx, y, spine_w, tile_h, HP_DARK, outline=STEEL_VERY_DARK)
    d.line([(cx - spine_w // 4, y + 2), (cx - spine_w // 4, y + tile_h - 2)],
           fill=HP_LIGHT, width=max(1, int(s)))

    n_pipes = max(3, int(panel_w / (18 * max(1.0, s))))
    pipe_spacing = panel_w / (n_pipes + 1)
    for i in range(n_pipes):
        pipe_x = cx - phw + int((i + 1) * pipe_spacing)
        if abs(pipe_x - cx) < spine_w // 2 + 2:
            continue
        d.line([(pipe_x, y + 4), (pipe_x, y + tile_h - 4)],
               fill=HP_PIPE, width=max(2, int(3 * s)))
        d.line([(pipe_x - 1, y + 4), (pipe_x - 1, y + tile_h - 4)],
               fill=HP_PIPE_LIGHT, width=1)

    glow_spacing = tile_h / max(4, int(tile_h / (40 * s)))
    n_glows = max(3, int(tile_h / glow_spacing))
    for i in range(n_glows):
        gy = y + int((i + 0.5) * tile_h / n_glows)
        glow_w_half = phw - 4
        for gx in range(cx - glow_w_half, cx + glow_w_half, max(2, int(4 * s))):
            if abs(gx - cx) < spine_w // 2 + 2:
                continue
            t = abs(gx - cx) / max(1, glow_w_half)
            intensity = 0.3 + 0.5 * (1.0 - t)
            glow_c = lerp_color(HP_BODY, HP_GLOW, intensity * 0.4)
            d.point((gx, gy), fill=glow_c)
            d.point((gx, gy + 1), fill=glow_c)

    n_bands = max(2, int(tile_h / (80 * s)))
    for i in range(n_bands):
        frac = (i + 1) / (n_bands + 1)
        by = y + int(tile_h * frac)
        band_h_px = max(3, int(4 * s))
        d.rectangle([cx - phw, by, cx + phw, by + band_h_px], fill=HP_LIGHT)
        d.rectangle([cx - phw, by + band_h_px, cx + phw, by + band_h_px + 1],
                    fill=HP_DARK)


def generate_heatpipe_radiator(size="tiny"):
    sizes = {"tiny": 2, "small": 4, "medium": 8, "large": 16}
    GH = sizes[size]
    s = REF_SCALE

    img_w = PX
    img_h = GH * PX
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2

    mount_h = max(10, int(REF_H * 0.04))
    mount_w = REF_W * 0.85
    manifold_h = max(8, int(REF_H * 0.025))
    manifold_w = REF_W * 0.9
    panel_w = REF_W * 0.92
    header_total = mount_h + manifold_h

    y = 0
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=s)

    mhw = int(manifold_w / 2)
    n_ports = max(3, int(manifold_w / (20 * s)))
    rect(d, cx, y, manifold_w, manifold_h, HP_DARK, outline=STEEL_VERY_DARK)
    for i in range(n_ports):
        px_pos = cx - mhw + int((i + 0.5) * manifold_w / n_ports)
        pr = max(2, int(3 * s))
        circ(d, px_pos, y + manifold_h // 2, pr, fill=HP_GLOW)
        circ(d, px_pos, y + manifold_h // 2, max(1, pr - 1), fill=HP_GLOW_BRIGHT)
    y += manifold_h

    panel_h = img_h - 2 * header_total
    tile_h = REF_H - 2 * header_total
    n_tiles = max(1, round(panel_h / tile_h))
    actual_tile_h = panel_h / n_tiles

    for t in range(n_tiles):
        ty = y + int(t * actual_tile_h)
        th = int((t + 1) * actual_tile_h) - int(t * actual_tile_h)
        _draw_heatpipe_panel_tile(d, cx, ty, panel_w, th)

    y += panel_h

    manifold_h2 = manifold_h
    rect(d, cx, y, manifold_w, manifold_h2, HP_DARK, outline=STEEL_VERY_DARK)
    for i in range(n_ports):
        px_pos = cx - mhw + int((i + 0.5) * manifold_w / n_ports)
        pr = max(2, int(3 * s))
        circ(d, px_pos, y + manifold_h2 // 2, pr, fill=HP_GLOW)
    y += manifold_h2

    draw_mount_ring(d, cx, y, mount_w, mount_h, scale=s)
    return img


# ================================================================
# Droplet Radiator generator
# ================================================================

def _draw_droplet_curtain_tile(d, cx, y, curtain_w, tile_h):
    """Draw one repeatable tile of droplet curtain detail."""
    s = REF_SCALE
    chw = int(curtain_w / 2)
    spine_w = max(4, int(REF_W * 0.08))

    rect(d, cx, y, spine_w, tile_h, DR_DARK, outline=STEEL_VERY_DARK)

    n_strips = max(30, int(tile_h / 3))
    strip_h = tile_h / n_strips
    for i in range(n_strips):
        t = i / n_strips
        sy = y + int(t * tile_h)
        heat = 1.0 - t * 0.6
        base = lerp_color(DR_DARK, DR_BODY, 0.4 + 0.3 * heat)
        glow = lerp_color(base, DR_GLOW, heat * 0.35)
        d.rectangle([cx - chw, int(sy), cx - spine_w // 2 - 1, int(sy + strip_h) + 1],
                    fill=glow)
        d.rectangle([cx + spine_w // 2 + 1, int(sy), cx + chw, int(sy + strip_h) + 1],
                    fill=glow)

    n_streams = max(3, int(curtain_w / (14 * max(1.0, s))))
    stream_spacing = curtain_w / (n_streams + 1)
    for i in range(n_streams):
        sx = cx - chw + int((i + 1) * stream_spacing)
        if abs(sx - cx) < spine_w // 2 + 2:
            continue
        for j in range(0, tile_h, max(3, int(5 * s))):
            sy = y + j
            t = j / tile_h
            heat = 1.0 - t * 0.6
            stream_c = lerp_color(DR_LIGHT, DR_GLOW_BRIGHT, heat * 0.5)
            d.point((sx, sy), fill=stream_c)
            if int(2 * s) > 1:
                d.point((sx, sy + 1), fill=stream_c)

    n_rings = max(2, int(tile_h / (60 * s)))
    ring_th = max(3, int(4 * s))
    for i in range(n_rings):
        frac = (i + 1) / (n_rings + 1)
        ry = y + int(tile_h * frac)
        for row in range(ring_th):
            t_r = row / max(1, ring_th - 1)
            shade = 0.2 + 0.6 * math.sin(t_r * math.pi)
            c = lerp_color(COIL_DARK, COIL_MID, shade)
            d.rectangle([cx - chw, ry + row, cx + chw, ry + row + 1], fill=c)


def generate_droplet_radiator(size="tiny"):
    sizes = {"tiny": 2, "small": 4, "medium": 8, "large": 16}
    GH = sizes[size]
    s = REF_SCALE

    img_w = PX
    img_h = GH * PX
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2

    mount_h = max(10, int(REF_H * 0.035))
    mount_w = REF_W * 0.8
    emitter_h = int(REF_H * 0.08)
    emitter_w = REF_W * 0.88
    ehw = int(emitter_w / 2)
    curtain_w = REF_W * 0.85
    collector_h = emitter_h
    header_total = mount_h + emitter_h

    y = 0
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=s)

    # Emitter module
    rect(d, cx, y, emitter_w, emitter_h, DR_EMITTER, outline=DR_DARK)
    coil_h = max(3, int(4 * s))
    for frac in [0.25, 0.75]:
        cy_pos = y + int(emitter_h * frac)
        for row in range(coil_h):
            t_r = row / max(1, coil_h - 1)
            shade = 0.2 + 0.6 * math.sin(t_r * math.pi)
            c = lerp_color(COIL_DARK, COIL_LIGHT, shade)
            ry = cy_pos - coil_h // 2 + row
            d.rectangle([cx - ehw - 2, ry, cx + ehw + 2, ry + 1], fill=c)
        d.line([(cx - ehw - 1, cy_pos - coil_h // 2),
                (cx + ehw + 1, cy_pos - coil_h // 2)],
               fill=COIL_HIGHLIGHT, width=1)
    slit_h = max(2, int(3 * s))
    n_slits = max(3, int(emitter_w / (15 * s)))
    for i in range(n_slits):
        sx = cx - ehw + int((i + 0.5) * emitter_w / n_slits)
        slit_y = y + emitter_h - slit_h - 2
        d.rectangle([sx - 1, slit_y, sx + 1, slit_y + slit_h], fill=DR_GLOW)
    y += emitter_h

    # Tiled curtain
    curtain_h = img_h - 2 * header_total
    ref_curtain_h = REF_H - 2 * header_total
    n_tiles = max(1, round(curtain_h / ref_curtain_h))
    actual_tile_h = curtain_h / n_tiles

    for t in range(n_tiles):
        ty = y + int(t * actual_tile_h)
        th = int((t + 1) * actual_tile_h) - int(t * actual_tile_h)
        _draw_droplet_curtain_tile(d, cx, ty, curtain_w, th)

    y += curtain_h

    # Collector module
    collector_w = emitter_w
    chwc = int(collector_w / 2)
    rect(d, cx, y, collector_w, collector_h, DR_COLLECTOR, outline=DR_DARK)
    for i in range(n_slits):
        sx = cx - chwc + int((i + 0.5) * collector_w / n_slits)
        d.rectangle([sx - 1, y + 2, sx + 1, y + 2 + slit_h], fill=DR_GLOW)
    for frac in [0.4, 0.8]:
        cy_pos = y + int(collector_h * frac)
        for row in range(coil_h):
            t_r = row / max(1, coil_h - 1)
            shade = 0.2 + 0.6 * math.sin(t_r * math.pi)
            c = lerp_color(COIL_DARK, COIL_LIGHT, shade)
            ry = cy_pos - coil_h // 2 + row
            d.rectangle([cx - chwc - 2, ry, cx + chwc + 2, ry + 1], fill=c)
    y += collector_h

    draw_mount_ring(d, cx, y, mount_w, mount_h, scale=s)
    return img


# ================================================================
# Phononic Emitter generator
# ================================================================

def _draw_phononic_panel_tile(d, cx, y, panel_w, tile_h):
    """Draw one repeatable tile of phononic lattice panel detail."""
    s = REF_SCALE
    phw = int(panel_w / 2)
    spine_w = max(5, int(REF_W * 0.1))

    rect(d, cx, y, panel_w, tile_h, PH_BODY, outline=PH_DARK)

    rect(d, cx, y, spine_w, tile_h, PH_DARK, outline=STEEL_VERY_DARK)
    d.line([(cx - spine_w // 4, y + 2), (cx - spine_w // 4, y + tile_h - 2)],
           fill=PH_LIGHT, width=max(1, int(s)))

    cell_size = max(8, int(16 * s))
    for row in range(0, tile_h, cell_size):
        for col_idx in range(-phw, phw, cell_size):
            lx = cx + col_idx
            ly = y + row
            if abs(lx - cx) < spine_w // 2 + 2:
                continue
            if ly + cell_size > y + tile_h:
                continue
            offset = cell_size // 2 if (row // cell_size) % 2 == 1 else 0
            lx += offset
            if lx - cell_size // 2 < cx - phw or lx + cell_size // 2 > cx + phw:
                continue
            half = cell_size // 2
            d.line([(lx, ly), (lx + half, ly + half)],
                   fill=PH_LATTICE, width=max(1, int(1.5 * s)))
            d.line([(lx, ly), (lx - half, ly + half)],
                   fill=PH_LATTICE, width=max(1, int(1.5 * s)))
            d.line([(lx + half, ly + half), (lx, ly + cell_size)],
                   fill=PH_LATTICE, width=max(1, int(1.5 * s)))
            d.line([(lx - half, ly + half), (lx, ly + cell_size)],
                   fill=PH_LATTICE, width=max(1, int(1.5 * s)))
            node_r = max(2, int(3 * s))
            t_vert = row / max(1, tile_h)
            glow_intensity = 0.4 + 0.4 * math.sin(t_vert * math.pi * 3)
            node_c = lerp_color(PH_LATTICE_LIGHT, PH_GLOW, glow_intensity)
            circ(d, lx, ly, node_r, fill=node_c)

    glow_w = max(2, int(3 * s))
    for side in [-1, 1]:
        edge_x = cx + side * (phw - 2)
        for gy in range(y + 4, y + tile_h - 4, max(2, int(3 * s))):
            t = (gy - y) / tile_h
            intensity = 0.3 + 0.5 * math.sin(t * math.pi)
            gc = lerp_color(PH_BODY, PH_GLOW, intensity)
            d.line([(edge_x - glow_w // 2, gy), (edge_x + glow_w // 2, gy)],
                   fill=gc, width=1)

    for gy in range(y + 4, y + tile_h - 4, max(2, int(2 * s))):
        t = (gy - y) / tile_h
        intensity = 0.5 + 0.4 * math.sin(t * math.pi * 2)
        gc = lerp_color(PH_DARK, PH_GLOW_BRIGHT, intensity * 0.6)
        d.point((cx, gy), fill=gc)
        d.point((cx + 1, gy), fill=gc)

    n_bands = max(2, int(tile_h / (80 * s)))
    band_h_px = max(3, int(4 * s))
    for i in range(n_bands):
        frac = (i + 1) / (n_bands + 1)
        by = y + int(tile_h * frac)
        d.rectangle([cx - phw, by, cx + phw, by + band_h_px], fill=PH_LIGHT)
        d.rectangle([cx - phw, by + band_h_px, cx + phw, by + band_h_px + 1],
                    fill=PH_DARK)


def generate_phononic_radiator(size="tiny"):
    sizes = {"tiny": 2, "small": 4, "medium": 8, "large": 16}
    GH = sizes[size]
    s = REF_SCALE

    img_w = PX
    img_h = GH * PX
    img = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = img_w // 2

    mount_h = max(10, int(REF_H * 0.035))
    mount_w = REF_W * 0.75
    header_h = max(8, int(REF_H * 0.03))
    header_w = REF_W * 0.85
    hhw = int(header_w / 2)
    panel_w = REF_W * 0.9
    header_total = mount_h + header_h

    y = 0
    y = draw_mount_ring(d, cx, y, mount_w, mount_h, scale=s)

    n_dots = max(3, int(header_w / (16 * s)))
    rect(d, cx, y, header_w, header_h, PH_DARK, outline=STEEL_VERY_DARK)
    for i in range(n_dots):
        dx = cx - hhw + int((i + 0.5) * header_w / n_dots)
        pr = max(2, int(3 * s))
        circ(d, dx, y + header_h // 2, pr, fill=PH_GLOW)
        circ(d, dx, y + header_h // 2, max(1, pr - 1), fill=PH_GLOW_BRIGHT)
    y += header_h

    # Tiled lattice panel
    panel_h = img_h - 2 * header_total
    ref_panel_h = REF_H - 2 * header_total
    n_tiles = max(1, round(panel_h / ref_panel_h))
    actual_tile_h = panel_h / n_tiles

    for t in range(n_tiles):
        ty = y + int(t * actual_tile_h)
        th = int((t + 1) * actual_tile_h) - int(t * actual_tile_h)
        _draw_phononic_panel_tile(d, cx, ty, panel_w, th)

    y += panel_h

    bot_header_h = header_h
    rect(d, cx, y, header_w, bot_header_h, PH_DARK, outline=STEEL_VERY_DARK)
    for i in range(n_dots):
        dx = cx - hhw + int((i + 0.5) * header_w / n_dots)
        pr = max(2, int(3 * s))
        circ(d, dx, y + bot_header_h // 2, pr, fill=PH_GLOW)
    y += bot_header_h

    draw_mount_ring(d, cx, y, mount_w, mount_h, scale=s)
    return img


# ================================================================
# Registry and main
# ================================================================

PARTS = {
    # Heat Pipe tier
    "radiator_heatpipe_tiny":   ("Heat Pipe Panel T",   lambda: generate_heatpipe_radiator("tiny")),
    "radiator_heatpipe_small":  ("Heat Pipe Panel S",   lambda: generate_heatpipe_radiator("small")),
    "radiator_heatpipe_medium": ("Heat Pipe Panel M",   lambda: generate_heatpipe_radiator("medium")),
    "radiator_heatpipe_large":  ("Heat Pipe Panel L",   lambda: generate_heatpipe_radiator("large")),
    # Droplet tier
    "radiator_droplet_tiny":    ("Droplet Radiator T",  lambda: generate_droplet_radiator("tiny")),
    "radiator_droplet_small":   ("Droplet Radiator S",  lambda: generate_droplet_radiator("small")),
    "radiator_droplet_medium":  ("Droplet Radiator M",  lambda: generate_droplet_radiator("medium")),
    "radiator_droplet_large":   ("Droplet Radiator L",  lambda: generate_droplet_radiator("large")),
    # Phononic tier
    "radiator_phononic_tiny":   ("Phononic Emitter T",  lambda: generate_phononic_radiator("tiny")),
    "radiator_phononic_small":  ("Phononic Emitter S",  lambda: generate_phononic_radiator("small")),
    "radiator_phononic_medium": ("Phononic Emitter M",  lambda: generate_phononic_radiator("medium")),
    "radiator_phononic_large":  ("Phononic Emitter L",  lambda: generate_phononic_radiator("large")),
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
