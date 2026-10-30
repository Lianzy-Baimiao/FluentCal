# Which icon codepoints in src/fluent/icons.ts are missing from Segoe MDL2 Assets (the icon
# font Windows 10 has) though Segoe Fluent Icons (Windows 11) has them. Reads the fonts' cmap
# (format 4) directly, so no extra packages are needed.
import os, re, struct, sys

def cmap(path):
    data = open(path, 'rb').read()
    num = struct.unpack('>H', data[4:6])[0]
    tables = {data[12 + i * 16:16 + i * 16].decode('latin1'): struct.unpack('>II', data[20 + i * 16:28 + i * 16]) for i in range(num)}
    off = tables['cmap'][0]
    n = struct.unpack('>H', data[off + 2:off + 4])[0]
    for i in range(n):
        pid, eid, sub = struct.unpack('>HHI', data[off + 4 + i * 8:off + 12 + i * 8])
        s = off + sub
        if (pid, eid) in ((3, 1), (0, 3)) and struct.unpack('>H', data[s:s + 2])[0] == 4:
            seg2 = struct.unpack('>H', data[s + 6:s + 8])[0]
            seg = seg2 // 2
            arr = lambda k: struct.unpack(f'>{seg}H', data[s + 14 + k * seg2 + (2 if k else 0): s + 14 + k * seg2 + (2 if k else 0) + seg2])
            ends, starts = arr(0), arr(1)
            deltas = struct.unpack(f'>{seg}h', data[s + 16 + 2 * seg2: s + 16 + 3 * seg2])
            ro_pos = s + 16 + 3 * seg2
            ros = struct.unpack(f'>{seg}H', data[ro_pos: ro_pos + seg2])
            def gid(c):
                for k in range(seg):
                    if starts[k] <= c <= ends[k]:
                        if ros[k] == 0:
                            return (c + deltas[k]) & 0xFFFF
                        p = ro_pos + 2 * k + ros[k] + 2 * (c - starts[k])
                        g = struct.unpack('>H', data[p:p + 2])[0]
                        return (g + deltas[k]) & 0xFFFF if g else 0
                return 0
            return gid
    raise SystemExit(f'no format-4 cmap in {path}')

fonts = os.path.join(os.environ['WINDIR'], 'Fonts')
mdl2, fluent = cmap(os.path.join(fonts, 'segmdl2.ttf')), cmap(os.path.join(fonts, 'SegoeIcons.ttf'))
src = open(sys.argv[1], encoding='utf-8').read()
glyphs = src[src.index('export const GLYPHS'):src.index('};', src.index('export const GLYPHS'))]
used = re.findall(r"^\s*(\w+):\s*'\\u([0-9A-Fa-f]{4})'", glyphs, re.M)
missing = [(name, cp) for name, cp in used if not mdl2(int(cp, 16))]
print(f'{len(used)} icons; in Segoe Fluent Icons: {sum(1 for _, cp in used if fluent(int(cp, 16)))}; missing from Segoe MDL2 Assets: {len(missing)}')
for name, cp in missing:
    print(f'  {name} U+{cp.upper()}')
