"""Generate the FluentCal app icon (calendar page + gantt bars) at 1024px."""
from PIL import Image, ImageDraw, ImageFilter
import sys

S = 2048  # supersample, downscaled to 1024 at the end
img = Image.new('RGBA', (S, S), (0, 0, 0, 0))


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def vgradient(size, top, bottom):
    w, h = size
    g = Image.new('RGBA', size)
    px = g.load()
    for y in range(h):
        c = lerp(top, bottom, y / max(1, h - 1))
        for x in range(w):
            px[x, y] = c
    return g


def rounded_mask(size, radius, box=None):
    m = Image.new('L', size, 0)
    ImageDraw.Draw(m).rounded_rectangle(box or (0, 0, size[0] - 1, size[1] - 1), radius=radius, fill=255)
    return m


L, T, R, B = 200, 300, 1848, 1800
rad = 240

# soft shadow
shadow = Image.new('RGBA', (S, S), (0, 0, 0, 0))
ImageDraw.Draw(shadow).rounded_rectangle((L, T + 40, R, B + 40), radius=rad, fill=(0, 0, 0, 90))
img = Image.alpha_composite(img, shadow.filter(ImageFilter.GaussianBlur(60)))

# page body
body = vgradient((R - L, B - T), (255, 255, 255, 255), (236, 240, 246, 255))
img.paste(body, (L, T), rounded_mask((R - L, B - T), rad))

# header band
HB = 760
header = vgradient((R - L, HB - T), (40, 153, 245, 255), (15, 108, 189, 255))
hm = rounded_mask((R - L, HB - T + rad), rad).crop((0, 0, R - L, HB - T))
img.paste(header, (L, T), hm)

d = ImageDraw.Draw(img)
# binder rings
for cx in (640, 1408):
    d.rounded_rectangle((cx - 70, T - 130, cx + 70, T + 170), radius=70, fill=(11, 69, 128, 255))
    d.rounded_rectangle((cx - 40, T - 100, cx + 40, T + 140), radius=40, fill=(222, 236, 249, 255))

# gantt bars
bars = [
    ((400, 930, 1200, 1100), (40, 153, 245, 255)),
    ((760, 1200, 1640, 1370), (91, 95, 199, 255)),
    ((520, 1470, 1120, 1640), (0, 178, 148, 255)),
]
for box, col in bars:
    d.rounded_rectangle(box, radius=(box[3] - box[1]) // 2, fill=col)

out = img.resize((1024, 1024), Image.LANCZOS)
out.save(sys.argv[1])
print('saved', sys.argv[1])
