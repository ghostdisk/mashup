"""Inspect source headers for models skipped by the main-world import."""
from pathlib import Path
import struct
import math,sys

root=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas')
names=set(sys.argv[1:]) or {'cunterb01.dff','cunterb03.dff','smallradar02_lvs.dff','ap_smallradar1_sfse.dff','lodce_brewery.dff'}
with (root/'models/gta3.img').open('rb') as f:
    magic,count=struct.unpack('<4sI',f.read(8));directory=f.read(count*32)
    for i in range(count):
        offset,a,b,name=struct.unpack_from('<IHH24s',directory,i*32);name=name.split(b'\0')[0].decode().lower()
        if name not in names:continue
        f.seek(offset*2048);data=f.read((a or b)*2048)
        print(name)
        def chunks(data,owner=0):
            p=0
            while p+12<=len(data):
                kind,size,version=struct.unpack_from('<III',data,p)
                if kind==0:break
                body=data[p+12:p+12+size]
                if kind==1 and owner in (0xe,0xf):
                    print('owner',hex(owner),'version',hex(version),'bytes',size,'header',body[:64].hex())
                    invalid=[(i,hex(struct.unpack_from('<I',body,i)[0])) for i in range(0,len(body)-3,4) if not math.isfinite(struct.unpack_from('<f',body,i)[0])]
                    print('nonfinite interpretations',invalid[:20])
                    if owner==0xe:print('root frame',struct.unpack_from('<12f',body,4))
                    if owner==0xf:
                        flags,nt,nv,nm=struct.unpack_from('<4I',body)
                        sets=(flags>>16)&255 or (2 if flags&128 else 1 if flags&4 else 0)
                        uvstart=16+(4*nv if flags&8 else 0)
                        tri_start=uvstart+sets*nv*8
                        morph=tri_start+nt*8
                        position_present,normal_present=struct.unpack_from('<II',body,morph+16)
                        used=set()
                        for t in range(nt):
                            b,a,material,c=struct.unpack_from('<4H',body,tri_start+t*8);used.update((a,b,c))
                        for label,start,n,width in [('uv',uvstart,nv,2),('positions',morph+24,nv if position_present else 0,3),('normals',morph+24+(nv*12 if position_present else 0),nv if normal_present else 0,3)]:
                            bad=[i for i in range(n) if any(not math.isfinite(v) for v in struct.unpack_from('<'+'f'*width,body,start+i*width*4))]
                            print(label,'bad vertices',bad[:20],'referenced',len(set(bad)&used),'total',len(bad))
                if kind in (0x10,0xe,0x1a,0xf):chunks(body,kind)
                p+=12+size
        chunks(data)
