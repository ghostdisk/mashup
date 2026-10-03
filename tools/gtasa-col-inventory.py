"""Read-only collision record inventory, including character/vehicle archives."""
from pathlib import Path
import struct
from collections import Counter

root = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas')
def records(data, source):
    p = 0
    while p + 8 <= len(data) and data[p:p+4] != bytes(4):
        kind, size = struct.unpack_from('<4sI', data, p)
        name = data[p+8:p+30].split(b'\0')[0].decode(errors='replace')
        body = data[p:p+size+8]
        if kind == b'COLL':
            cursor = 72
            counts = []
            try:
                for stride in (20, 0, 28, 12, 16):
                    count = struct.unpack_from('<I', body, cursor)[0]; cursor += 4
                    counts.append(count); cursor += count * stride
                print(source, kind, name, 'size', size+8, 'end', cursor, 'counts', counts)
            except struct.error as e:
                print('INVALID', source, name, len(body), cursor, counts, e)
        elif kind not in (b'COL2', b'COL3', b'COL4'):
            print('UNKNOWN', source, kind, name)
        p += size + 8
    return p

for archive in ('gta3.img', 'gta_int.img'):
    with (root/'models'/archive).open('rb') as f:
        magic, count = struct.unpack('<4sI', f.read(8)); directory = f.read(count*32)
        for i in range(count):
            offset, a, b, name = struct.unpack_from('<IHH24s', directory, i*32)
            name = name.split(b'\0')[0].decode().lower()
            if name.endswith('.col'):
                f.seek(offset*2048); records(f.read((a or b)*2048), archive+'/'+name)
for path in (root/'models/coll').glob('*.col'):
    records(path.read_bytes(), str(path))
