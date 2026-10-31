"""Compares 0.2's rendering with the native one for each scene in native/.parity (see
scripts/parity.mjs). Prints the mean colour difference, the share of clearly different pixels,
and a "structural" difference (both blurred first, so glyph anti-aliasing differences between
the two text rasterizers mostly cancel and shifted or missing elements stand out), with the
worst 40x40 blocks. Writes <scene>.side.png (0.2 | native | difference). Needs Pillow."""
import sys
from pathlib import Path
from PIL import Image, ImageChops, ImageFilter

d = Path(__file__).resolve().parent.parent / 'native' / '.parity'
args = [a for a in sys.argv[1:] if not a.startswith('-')]
scenes = args or sorted(p.name[:-8] for p in d.glob('*.ref.png'))
BLOCK = 40


def mean(img):
    hist = img.histogram()
    return sum(i * n for i, n in enumerate(hist)) / (img.size[0] * img.size[1])


for name in scenes:
    ref_p, nat_p = d / f'{name}.ref.png', d / f'{name}.native.png'
    if not nat_p.exists():
        print(f'{name}: no native rendering')
        continue
    ref = Image.open(ref_p).convert('RGB')
    nat = Image.open(nat_p).convert('RGB')
    if nat.size != ref.size:
        print(f'{name}: size {nat.size} != {ref.size}')
        nat = nat.resize(ref.size)
    diff = ImageChops.difference(ref, nat).convert('L')
    total = ref.size[0] * ref.size[1]
    big = sum(diff.histogram()[24:]) / total
    blur = ImageFilter.GaussianBlur(2)
    sdiff = ImageChops.difference(ref.filter(blur), nat.filter(blur)).convert('L')
    print(f'{name}: mean diff {mean(diff):.2f}, pixels off by >24: {big * 100:.2f}%, structural {mean(sdiff):.2f}')
    blocks = []
    for by in range(0, ref.size[1] - BLOCK + 1, BLOCK):
        for bx in range(0, ref.size[0] - BLOCK + 1, BLOCK):
            blocks.append((mean(sdiff.crop((bx, by, bx + BLOCK, by + BLOCK))), bx, by))
    blocks.sort(reverse=True)
    print('  worst blocks (x,y: structural):', ', '.join(f'{x},{y}: {v:.1f}' for v, x, y in blocks[:8]))
    heat = diff.point(lambda v: min(255, v * 4)).convert('RGB')
    side = Image.new('RGB', (ref.size[0] * 3, ref.size[1]))
    for i, im in enumerate((ref, nat, heat)):
        side.paste(im, (i * ref.size[0], 0))
    side.save(d / f'{name}.side.png')
