#!/usr/bin/env python3
"""
EchoMind (灵犀) — Icon Generator
Generates all platform-specific icon files from a programmatic logo design.

Output files (in crates/tauri-app/icons/):
  - icon.png            (512×512, Tauri default)
  - 32x32.png           (Windows/Linux taskbar)
  - 128x128.png         (Linux desktop)
  - 128x128@2x.png      (256×256, HiDPI)
  - icon.icns           (macOS — via iconutil)
  - icon.ico            (Windows — multi-resolution)
  - StoreLogo.png       (Microsoft Store, optional)

Design: Concentric echo-arcs radiating from a central "mind" node.
Colors: gradient sky-blue → deep-blue background, white symbol.

Usage:
    python3 scripts/generate-icons.py
"""

import math
import os
import shutil
import subprocess
import tempfile
from PIL import Image, ImageDraw, ImageFilter

# ── Configuration ──────────────────────────────────────────────
MASTER_SIZE = 1024
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ICONS_DIR = os.path.normpath(os.path.join(SCRIPT_DIR, "..", "crates", "tauri-app", "icons"))

# Colors — matching UI design tokens (FIGMA_DESIGN_SPEC §2.1)
COL_BG_TOP     = (56, 189, 248)    # #38BDF8 accent
COL_BG_MID     = (14, 165, 233)    # #0EA5E9 accent-hover
COL_BG_BOT     = (3, 105, 161)     # #0369A1 deep blue
COL_SYMBOL     = (248, 250, 252)   # #F8FAFC text-primary (near-white)
COL_NODE_INNER = (14, 165, 233)    # #0EA5E9 inner dot

# Arc parameters (fractions of canvas size). Each arc is a top semicircle.
# PIL arc coordinates (Y-down): 0°=right, 90°=bottom, 180°=left, 270°=top.
# arc(180, 360) draws the TOP semicircle (left→top→right) — the echo shape.
ARCS = [
    {"radius": 0.094, "thickness": 0.043, "opacity": 1.00},
    {"radius": 0.156, "thickness": 0.039, "opacity": 0.85},
    {"radius": 0.219, "thickness": 0.035, "opacity": 0.70},
    {"radius": 0.281, "thickness": 0.031, "opacity": 0.50},
]

NODE_OUTER = 0.059   # central circle radius fraction
NODE_INNER = 0.027   # inner dot radius fraction
CORNER_RADIUS = 0.225  # squircle corner radius (~115/512)


def lerp_color(c1, c2, t):
    """Linear interpolation between two RGB tuples."""
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3))


def create_gradient_background(size):
    """Diagonal gradient: top-left bright → bottom-right deep."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    px = img.load()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * size)
            if t < 0.55:
                color = lerp_color(COL_BG_TOP, COL_BG_MID, t / 0.55)
            else:
                color = lerp_color(COL_BG_MID, COL_BG_BOT, (t - 0.55) / 0.45)
            px[x, y] = (*color, 255)
    return img


def apply_squircle_mask(img, radius_frac):
    """Apply a rounded-rectangle (squircle) alpha mask."""
    size = img.width
    radius = int(size * radius_frac)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, size - 1, size - 1], radius=radius, fill=255
    )
    img.putalpha(mask)
    return img


def draw_echo_arcs(layer, size):
    """Draw concentric echo arcs (top semicircles) on a transparent layer."""
    draw = ImageDraw.Draw(layer)
    cx = cy = size // 2

    for arc in ARCS:
        r = int(size * arc["radius"])
        t = max(2, int(size * arc["thickness"]))
        opacity = int(255 * arc["opacity"])
        color = (*COL_SYMBOL, opacity)

        # Draw thick arc by stacking 1px-wide arcs symmetrically around radius r
        for offset in range(t):
            half = offset - t // 2
            r_off = r + half
            if r_off <= 0:
                continue
            bb = [cx - r_off, cy - r_off, cx + r_off, cy + r_off]
            draw.arc(bb, start=180, end=360, fill=color, width=1)

    # Slight blur for anti-aliasing smoothness
    layer = layer.filter(ImageFilter.GaussianBlur(radius=max(1, size // 512)))
    return layer


def draw_central_node(layer, size):
    """Draw the central 'mind' node: outer white circle + inner colored dot."""
    draw = ImageDraw.Draw(layer)
    cx = cy = size // 2
    r_outer = max(2, int(size * NODE_OUTER))
    r_inner = max(1, int(size * NODE_INNER))

    draw.ellipse(
        [cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer],
        fill=(*COL_SYMBOL, 255),
    )
    draw.ellipse(
        [cx - r_inner, cy - r_inner, cx + r_inner, cy + r_inner],
        fill=(*COL_NODE_INNER, 255),
    )
    return layer


def render_master(size=MASTER_SIZE):
    """Render the master icon at the given size."""
    # 1. Gradient background
    img = create_gradient_background(size)
    # 2. Squircle mask
    img = apply_squircle_mask(img, CORNER_RADIUS)
    # 3. Echo arcs
    arcs_layer = draw_echo_arcs(Image.new("RGBA", (size, size), (0, 0, 0, 0)), size)
    img = Image.alpha_composite(img, arcs_layer)
    # 4. Central node
    node_layer = draw_central_node(Image.new("RGBA", (size, size), (0, 0, 0, 0)), size)
    img = Image.alpha_composite(img, node_layer)
    return img


def generate_pngs(master):
    """Generate all PNG sizes from the master image."""
    png_files = {
        "icon.png": 512,
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "StoreLogo.png": 512,
    }
    for filename, px in png_files.items():
        path = os.path.join(ICONS_DIR, filename)
        master.resize((px, px), Image.LANCZOS).save(path, "PNG")
        print(f"  ✓ {filename} ({px}×{px})")


def generate_icns(master):
    """Generate macOS .icns via iconutil."""
    if shutil.which("iconutil") is None:
        print("  ✗ iconutil not found — skipping .icns")
        return

    iconset_sizes = [
        ("icon_16x16.png", 16), ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32), ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128), ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256), ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512), ("icon_512x512@2x.png", 1024),
    ]

    with tempfile.TemporaryDirectory() as tmpdir:
        iconset_path = os.path.join(tmpdir, "icon.iconset")
        os.makedirs(iconset_path)
        for filename, px in iconset_sizes:
            master.resize((px, px), Image.LANCZOS).save(
                os.path.join(iconset_path, filename), "PNG"
            )
        icns_path = os.path.join(ICONS_DIR, "icon.icns")
        if os.path.exists(icns_path):
            os.remove(icns_path)
        result = subprocess.run(
            ["iconutil", "-c", "icns", iconset_path, "-o", icns_path],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            print("  ✓ icon.icns (macOS)")
        else:
            print(f"  ✗ icon.icns failed: {result.stderr}")


def generate_ico(master):
    """Generate Windows .ico with multiple embedded resolutions."""
    # Pillow ICO: save master with `sizes` param — it auto-resizes and embeds all sizes
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    ico_path = os.path.join(ICONS_DIR, "icon.ico")
    master.save(ico_path, format="ICO", sizes=ico_sizes)
    print(f"  ✓ icon.ico (Windows, {len(ico_sizes)} sizes)")


def main():
    print("EchoMind (灵犀) — Icon Generator")
    print("=" * 50)

    os.makedirs(ICONS_DIR, exist_ok=True)

    print(f"\nRendering master at {MASTER_SIZE}×{MASTER_SIZE}…")
    master = render_master(MASTER_SIZE)
    master_path = os.path.join(ICONS_DIR, "logo-master.png")
    master.save(master_path, "PNG")
    print(f"  ✓ logo-master.png ({MASTER_SIZE}×{MASTER_SIZE})")

    print("\nGenerating PNG icons:")
    generate_pngs(master)

    print("\nGenerating macOS .icns:")
    generate_icns(master)

    print("\nGenerating Windows .ico:")
    generate_ico(master)

    print("\n" + "=" * 50)
    print("✅ All icons generated successfully!")
    print(f"   Output directory: {ICONS_DIR}")


if __name__ == "__main__":
    main()
