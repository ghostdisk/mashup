"""List actual imported model/placement records around a chosen source region."""
from pathlib import Path
import json,sys,math

root=Path('assets/imported/gtasa/main')
manifest=json.loads((root/'world.mashup.json').read_text())
x,z=map(float,sys.argv[1:3]);radius=float(sys.argv[3]) if len(sys.argv)>3 else 250
records=[]
for chunk in manifest['chunks']:
    bounds=chunk['bounds']
    if bounds['min'][0]>x+radius or bounds['max'][0]<x-radius or bounds['min'][2]>z+radius or bounds['max'][2]<z-radius:continue
    for instance in json.loads((root/chunk['payload']).read_text())['instances']:
        p=instance['translation'];model=manifest['models'][instance['model']]
        if math.hypot(p[0]-x,p[2]-z)>radius:continue
        if model['lod']:continue
        if any(word in model['name'] for word in ('road','ground','land','rail','grass','terrain','sfse','sfs')):
            records.append((round(math.hypot(p[0]-x,p[2]-z)),instance['model'],model['name'],p,instance['rotation'],model['collision'],instance['source']['ipl'],instance['source']['interior_flags']))
for record in sorted(records)[:100]:print(record)
print('Model bounds covering the exact X/Z point:')
def rotate(v,q):
    qx,qy,qz,qw=q;vx,vy,vz=v
    tx=2*(qy*vz-qz*vy);ty=2*(qz*vx-qx*vz);tz=2*(qx*vy-qy*vx)
    return(vx+qw*tx+qy*tz-qz*ty,vy+qw*ty+qz*tx-qx*tz,vz+qw*tz+qx*ty-qy*tx)
for chunk in manifest['chunks']:
    bounds=chunk['bounds']
    if not(bounds['min'][0]<=x<=bounds['max'][0] and bounds['min'][2]<=z<=bounds['max'][2]):continue
    for instance in json.loads((root/chunk['payload']).read_text())['instances']:
        model=manifest['models'][instance['model']];b=model['bounds'];p=instance['translation'];q=instance['rotation']
        points=[rotate((a,c,d),q) for a in (b['min'][0],b['max'][0]) for c in (b['min'][1],b['max'][1]) for d in (b['min'][2],b['max'][2])]
        if min(v[0] for v in points)+p[0]<=x<=max(v[0] for v in points)+p[0] and min(v[2] for v in points)+p[2]<=z<=max(v[2] for v in points)+p[2]:
            print(instance['model'],model['name'],'lod',model['lod'],'collision',model['collision'],'position',p,'y-bounds',(min(v[1] for v in points)+p[1],max(v[1] for v in points)+p[1]),'source',instance['source'])
