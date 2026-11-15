# Generates packaging/AppIcon.icns: a plain rounded square with a "D" (not Dia's logo).
import subprocess, os, tempfile
from PIL import Image, ImageDraw, ImageFont
here = os.path.dirname(os.path.abspath(__file__))
S = 1024
im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
g = Image.new("RGBA", (S, S))
for y in range(S):
    t = y / S
    ImageDraw.Draw(g).line([(0, y), (S, y)], fill=(int(40 + 30 * t), int(48 + 20 * t), int(70 + 60 * t), 255))
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([90, 90, S - 90, S - 90], radius=190, fill=255)
im.paste(g, (0, 0), mask)
d = ImageDraw.Draw(im)
font = ImageFont.truetype("/System/Library/Fonts/SFNSRounded.ttf" if os.path.exists("/System/Library/Fonts/SFNSRounded.ttf") else "/System/Library/Fonts/Helvetica.ttc", 560)
d.text((S / 2, S / 2 + 10), "D", font=font, fill=(245, 245, 250, 255), anchor="mm")
d.line([(300, 760), (724, 300)], fill=(255, 120, 110, 255), width=46)  # strike-through: "no AI"
tmp = tempfile.mkdtemp(suffix=".iconset")
for sz in (16, 32, 128, 256, 512):
    im.resize((sz, sz), Image.LANCZOS).save(f"{tmp}/icon_{sz}x{sz}.png")
    im.resize((sz * 2, sz * 2), Image.LANCZOS).save(f"{tmp}/icon_{sz}x{sz}@2x.png")
subprocess.run(["iconutil", "-c", "icns", tmp, "-o", os.path.join(here, "AppIcon.icns")], check=True)
print("icon ok")
