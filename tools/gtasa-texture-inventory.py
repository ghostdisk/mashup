"""Inspect loose TXD names that may explain unresolved map references."""
from pathlib import Path
import struct,json,re

root=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas')
report=json.loads(Path('assets/imported/gtasa/main/import-report.json').read_text())
missing=set()
for warning in report['warnings']:
    match=re.search(r'unresolved texture (\S+) in',warning)
    if match:missing.add(match[1])
for filename in ('particle.txd','generic/vehicle.txd','effectsPC.txd'):
    path=root/'models'/filename;data=path.read_bytes();end=12+struct.unpack_from('<I',data,4)[0];p=12;names=[]
    while p+12<=end:
        kind,size=struct.unpack_from('<II',data,p)
        if kind==0x15:
            body=p+24
            name=data[body+8:body+40].split(b'\0')[0].decode().lower();names.append(name)
        p+=12+size
    print(filename,'textures',len(names),'unresolved matches',sorted(missing&set(names)))
