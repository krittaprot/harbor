# /// script
# dependencies = ["Pillow"]
# ///
"""Draw Harbor's specified anchor. Run from project root with uv run scripts/icons.py."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "icons"

def anchor(draw, scale, color, width):
    def xy(values): return tuple(v * scale for v in values)
    draw.ellipse(xy((9.8, 2.8, 14.2, 7.2)), outline=color, width=width)
    draw.line(xy((12, 7.2, 12, 21)), fill=color, width=width)
    draw.line(xy((8, 11, 16, 11)), fill=color, width=width)
    draw.arc(xy((4.5, 6, 19.5, 21)), 0, 180, fill=color, width=width)

size = 1024
image = Image.new("RGBA", (size, size))
gradient = Image.new("RGBA", (size, size))
d = ImageDraw.Draw(gradient)
for y in range(size):
    t = y / size
    d.line((0, y, size, y), fill=(int(255 - 19*t), int(255 - 18*t), int(255 - 14*t), 255))
mask = Image.new("L", (size, size))
ImageDraw.Draw(mask).rounded_rectangle((40, 40, 984, 984), radius=214, fill=255)
image.paste(gradient, (0, 0), mask)
mark = Image.new("RGBA", (768, 768))
anchor(ImageDraw.Draw(mark), 32, "#14213A", 64)
image.alpha_composite(mark, (128, 112))
image.save(OUT / "harbor.png")

for name, dot in [("template", None), ("green", "#34C759"), ("orange", "#FF9F0A"), ("red", "#FF3B30")]:
    tray = Image.new("RGBA", (192, 192))
    draw = ImageDraw.Draw(tray)
    anchor(draw, 8, "#000000" if dot is None else "#64748B", 14)
    if dot: draw.ellipse((133, 7, 187, 61), fill=dot)
    tray.resize((48, 48), Image.Resampling.LANCZOS).save(OUT / f"tray-{name}.png")
