"""Read actual vehicle frame names, hierarchy, pivots and atomic attachments."""
from pathlib import Path
import struct,sys
root=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas')
requested=(sys.argv[1] if len(sys.argv)>1 else 'admiral').lower()+'.dff'
with (root/'models/gta3.img').open('rb') as f:
    magic,count=struct.unpack('<4sI',f.read(8));directory=f.read(count*32)
    for i in range(count):
        sector,a,b,name=struct.unpack_from('<IHH24s',directory,i*32)
        if name.split(b'\0')[0].decode().lower()!=requested:continue
        f.seek(sector*2048);data=f.read((a or b)*2048);break
    else:raise ValueError('vehicle DFF absent')
def chunks(data):
    p=0
    while p+12<=len(data):
        kind,length,version=struct.unpack_from('<III',data,p)
        if kind==0 and length==0:break
        yield kind,data[p+12:p+12+length]
        p+=12+length
def child(data,kind):return next(body for id,body in chunks(data) if id==kind)
clump=child(data,0x10);frame_list=child(clump,0xe);s=child(frame_list,1);count=struct.unpack_from('<I',s)[0]
extensions=[body for id,body in chunks(frame_list) if id==3];names=[]
for i in range(count):
    name=next((body.split(b'\0')[0].decode() for id,body in chunks(extensions[i]) if id==0x253f2fe),f'frame_{i}')
    names.append(name);matrix=struct.unpack_from('<12fii',s,4+i*56)
    print('frame',i,name,'parent',matrix[12],'position',matrix[9:12])
for id,atomic in chunks(clump):
    if id!=0x14:continue
    frame,geometry,flags,unused=struct.unpack_from('<4I',child(atomic,1))
    print('atomic',names[frame],'geometry',geometry,'flags',hex(flags))
