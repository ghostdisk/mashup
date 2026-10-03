"""Read-only PE/RTTI inventory for locating GTA class vtables before MCP decompilation."""
from pathlib import Path
import hashlib,struct,sys

path=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grand Theft Auto San Andreas\gta_sa.exe')
data=path.read_bytes()
u32=lambda offset:struct.unpack_from('<I',data,offset)[0]
pe=u32(0x3c);optional=pe+24;image_base=u32(optional+28)
section_count=struct.unpack_from('<H',data,pe+6)[0]
section_start=optional+struct.unpack_from('<H',data,pe+20)[0]
sections=[]
for index in range(section_count):
    start=section_start+index*40
    name=data[start:start+8].split(b'\0')[0].decode()
    virtual_size,rva,size,offset=struct.unpack_from('<4I',data,start+8)
    sections.append((name,image_base+rva,size,offset))
def address(offset):
    for name,base,size,start in sections:
        if start<=offset<start+size:return base+offset-start
def raw(location):
    for name,base,size,start in sections:
        if base<=location<base+size:return start+location-base
def references(location,aligned=True):
    needle=struct.pack('<I',location)
    for name,base,size,start in sections:
        pos=data.find(needle,start,start+size)
        while pos>=0:
            if not aligned or (pos-start)%4==0:yield pos
            pos=data.find(needle,pos+1,start+size)
print('Executable',path,'bytes',len(data),'sha256',hashlib.sha256(data).hexdigest(),'base',hex(image_base))
if len(sys.argv)>1 and sys.argv[1]=='--file-offsets':
    for token in sys.argv[2:]:
        offset=int(token,16);location=address(offset)
        print('File offset',f'{offset:08x}','->',f'{location:08x}' if location is not None else 'not mapped')
    sys.exit(0)
if len(sys.argv)>1 and sys.argv[1]=='--references':
    for token in sys.argv[2:]:
        target=int(token,16)
        print('Raw pointer candidates for',f'{target:08x}',','.join(f'{address(ref):08x}' for ref in references(target,False)))
    sys.exit(0)
if len(sys.argv)>1 and sys.argv[1]=='--calls':
    targets={int(token,16) for token in sys.argv[2:]}
    for section,base,size,start in sections:
        if section!='.text':continue
        pos=data.find(b'\xe8',start,start+size-4)
        while pos>=0:
            destination=base+pos-start+5+struct.unpack_from('<i',data,pos+1)[0]
            if destination in targets:print('Relative CALL candidate',f'{base+pos-start:08x}','->',f'{destination:08x}')
            pos=data.find(b'\xe8',pos+1,start+size-4)
    sys.exit(0)
for name in sys.argv[1:] or ['CPlayerPed','CTaskSimplePlayerOnFoot','CTaskSimpleGoToPoint','CTaskSimpleGoTo']:
    pos=data.find(('.?AV'+name+'@@\0').encode())
    if pos<0:print(name,'type descriptor absent');continue
    descriptor=address(pos-8)
    print(name,'type_name',hex(address(pos)),'type_descriptor',hex(descriptor))
    for ref in references(descriptor):
        # CompleteObjectLocator: signature, object offset, ctor offset, type, hierarchy.
        locator=ref-12
        signature,offset,ctor,type_ptr,hierarchy=struct.unpack_from('<5I',data,locator)
        if signature!=0 or offset>4096 or ctor>4096 or raw(hierarchy) is None:continue
        locator_address=address(locator)
        for slot in references(locator_address):
            vtable=slot+4;functions=[]
            for entry in range(24):
                function=u32(vtable+entry*4)
                if not any(section=='.text' and base<=function<base+size for section,base,size,start in sections):break
                functions.append(f'{function:08x}')
            if functions:print(' locator',f'{locator_address:08x}','object_offset',offset,'vtable',f'{address(vtable):08x}','entries',','.join(functions))
