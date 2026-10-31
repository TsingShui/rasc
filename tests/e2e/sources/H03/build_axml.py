"""Self-made H03 input producer, not a production XML compiler.

Layout reference: AOSP android-15.0.0_r1 ResourceTypes.h (SOURCE_MAP §7).
The independently readable source XML is the input; no engine output is used.
"""
from pathlib import Path
import struct
import sys
import xml.etree.ElementTree as ET
import zipfile

NONE=0xffffffff
URI='http://schemas.android.com/apk/res/android'


def build(source):
    root=ET.fromstring(source.read_bytes())
    strings=['versionCode','label','debuggable','android',URI]
    def index(value):
        if value not in strings:strings.append(value)
        return strings.index(value)
    def split(name):
        if name.startswith('{'):
            namespace,local=name[1:].split('}',1)
            return index(namespace),index(local)
        return NONE,index(name)
    def node(kind,body):
        return struct.pack('<HHIII',kind,16,16+len(body),1,NONE)+body
    def element(current):
        namespace,name=split(current.tag);attributes=bytearray()
        for key,value in current.attrib.items():
            ns,attr=split(key);raw=index(value)
            typ,data=3,raw
            if key=='{'+URI+'}versionCode':typ,data=0x10,int(value)
            elif key=='{'+URI+'}debuggable':typ,data=0x12,int(value=='true')
            attributes+=struct.pack('<IIIHBBI',ns,attr,raw,8,0,typ,data)
        body=struct.pack('<II6H',namespace,name,20,20,len(current.attrib),0,0,0)+attributes
        output=node(0x102,body)
        for child in current:output+=element(child)
        return output+node(0x103,struct.pack('<II',namespace,name))
    nodes=node(0x100,struct.pack('<II',index('android'),index(URI)))
    nodes+=element(root)
    nodes+=node(0x101,struct.pack('<II',index('android'),index(URI)))
    data=bytearray();offsets=[]
    def length(value):
        if value>=0x8000:raise ValueError('fixture string exceeds its UTF8 length representation')
        return bytes([value]) if value<128 else bytes([0x80|(value>>8),value&255])
    for value in strings:
        offsets.append(len(data));encoded=value.encode('utf-8');units=len(value.encode('utf-16-le'))//2
        data+=length(units)+length(len(encoded))+encoded+b'\0'
    while len(data)%4:data.append(0)
    start=28+len(strings)*4
    pool=struct.pack('<HH6I',1,28,start+len(data),len(strings),0,0x100,start,0)
    pool+=b''.join(struct.pack('<I',offset) for offset in offsets)+data
    resources=struct.pack('<HHI3I',0x180,8,20,0x0101021b,0x01010001,0x0101000f)
    body=pool+resources+nodes
    return struct.pack('<HHI',3,8,len(body)+8)+body


if __name__=='__main__':
    destination=Path(sys.argv[1]);destination.mkdir(parents=True,exist_ok=True)
    binary=build(Path(__file__).with_name('AndroidManifest.xml'))
    (destination/'AndroidManifest.axml').write_bytes(binary)
    with zipfile.ZipFile(destination/'manifest.zip','w') as archive:
        info=zipfile.ZipInfo('AndroidManifest.xml',date_time=(1980,1,1,0,0,0))
        info.compress_type=zipfile.ZIP_STORED;info.external_attr=0o100644<<16
        archive.writestr(info,binary)
