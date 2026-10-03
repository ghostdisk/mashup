"""Read-only installed asset inspection; no game files are modified."""
from pathlib import Path
import struct
from collections import Counter

root = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas')
archive = root / 'models/gta3.img'
with archive.open('rb') as f:
    magic, count = struct.unpack('<4sI', f.read(8))
    entries = {}
    for _ in range(count):
        sector, size, archive_size, name = struct.unpack('<IHH24s', f.read(32))
        entries[name.split(b'\0')[0].decode().lower()] = (sector * 2048, (size or archive_size) * 2048)
    print('IMG', magic, count, Counter(Path(n).suffix for n in entries))
    def read(name):
        offset, size = entries[name]
        f.seek(offset)
        return f.read(size)
    def chunks(data, depth=0):
        offset = 0
        while offset + 12 <= len(data):
            kind, size, version = struct.unpack_from('<III', data, offset)
            if kind == 0 or offset + 12 + size > len(data): break
            body = data[offset+12:offset+12+size]
            print(' ' * depth, hex(kind), size, hex(version), body[:36].hex() if kind == 1 else '')
            if kind in (0x10, 0xe, 0x1a, 0xf, 0x8, 0x7, 0x6, 0x16, 0x15): chunks(body, depth+2)
            offset += 12 + size
    for name in ['lae2_ground04.dff', 'landlae2e.txd']:
        print(name); chunks(read(name))
    for name in entries:
        if name.endswith('.ipl') and name.startswith('lae2'):
            data = read(name)
            print(name, data[:76].hex(), struct.unpack_from('<10I', data, 4))
            break
    stats = Counter()
    for name in entries:
        if not name.endswith('.col'): continue
        data = read(name); p = 0
        while p + 108 <= len(data) and data[p:p+4] in (b'COLL', b'COL2', b'COL3', b'COL4'):
            kind = data[p:p+4].decode(); size = struct.unpack_from('<I', data, p+4)[0] + 8
            stats[kind] += 1
            if kind != 'COLL':
                spheres, boxes, faces, lines = struct.unpack_from('<HHHB', data, p+72)
                stats['spheres'] += spheres; stats['boxes'] += boxes; stats['faces'] += faces; stats['lines'] += lines
            p += size
    print('COL inventory', stats)
    rasters = Counter()
    for name in entries:
        if not name.endswith('.txd'): continue
        data = read(name); end = 12 + struct.unpack_from('<I', data, 4)[0]; p = 12
        while p + 12 <= end:
            kind, size = struct.unpack_from('<II', data, p)
            if kind == 0x15:
                s = p + 24
                platform = struct.unpack_from('<I', data, s)[0]
                raster, fmt, w, h, depth, levels, ty, flags = struct.unpack_from('<IIHHBBBB', data, s+72)
                rasters[(platform, hex(raster), hex(fmt), depth, flags)] += 1
            p += 12 + size
    print('Raster inventory', rasters)
