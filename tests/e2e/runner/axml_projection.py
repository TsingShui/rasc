"""Independent AOSP-layout projector for the H03 preparation fixture.

Closed fixture profile: UTF8 pool, resource map, balanced namespace/elements,
String/decimal/boolean attributes. Other profiles fail explicitly; this is not a
production AXML reader or certification of Diana's malformed-input safety.
"""
from __future__ import annotations
import struct
import xml.etree.ElementTree as ET
import zipfile


def project(data):
    def check(offset,size,end):
        if offset<0 or size<0 or offset>end or size>end-offset:raise ValueError('AXML range')
    def u16(offset):
        check(offset,2,len(data));return struct.unpack_from('<H',data,offset)[0]
    def u32(offset):
        check(offset,4,len(data));return struct.unpack_from('<I',data,offset)[0]
    def chunk(offset,end):
        check(offset,8,end);kind,header,size=struct.unpack_from('<HHI',data,offset)
        if offset%4 or header<8 or size<header or size%4:raise ValueError('AXML chunk alignment/header/size')
        check(offset,size,end);return kind,header,offset+size
    kind,header,end=chunk(0,len(data))
    if kind!=3 or end!=len(data):raise ValueError('not a complete binary XML document')
    strings=None;resources=[];namespaces=[];stack=[];root=None;cursor=header
    def string(index,optional=False):
        if index==0xffffffff and optional:return None
        if strings is None or index>=len(strings):raise ValueError('AXML string reference')
        return strings[index]
    def expanded(namespace,name):
        value=string(name);ns=string(namespace,True)
        if ns is not None and not any(uri==ns for _,uri in namespaces):raise ValueError('unbound AXML namespace')
        return '{'+ns+'}'+value if ns else value
    while cursor<end:
        kind,header,stop=chunk(cursor,end)
        if kind==1:
            if strings is not None or header<28:raise ValueError('duplicate/short string pool')
            count,styles,flags,start,style_start=struct.unpack_from('<5I',data,cursor+8)
            if styles or style_start or flags!=0x100:raise ValueError('unsupported fixture pool profile')
            check(cursor+header,count*4,stop)
            if start<header+count*4:raise ValueError('overlapping string offset table')
            strings=[]
            for n in range(count):
                position=cursor+start+u32(cursor+header+4*n)
                def length(position):
                    check(position,1,stop);value=data[position];position+=1
                    if value&128:
                        check(position,1,stop);value=(value&127)<<8|data[position];position+=1
                    return value,position
                units,position=length(position);size,position=length(position)
                check(position,size+1,stop)
                if data[position+size]!=0:raise ValueError('AXML string terminator')
                text=data[position:position+size].decode('utf-8',errors='strict')
                if len(text.encode('utf-16-le'))//2!=units:raise ValueError('AXML UTF16 versus UTF8 length')
                strings.append(text)
        elif kind==0x180:
            if strings is None or stack or (stop-cursor-header)%4:raise ValueError('AXML resource-map extent/order')
            resources=[u32(p) for p in range(cursor+header,stop,4)]
            if len(resources)>len(strings):raise ValueError('AXML resource-map index range')
        elif 0x100<=kind<=0x103:
            if strings is None or header<16:raise ValueError('AXML node header/pool')
            string(u32(cursor+12),True)
            extension=cursor+header
            if kind in (0x100,0x101):
                check(extension,8,stop);pair=(string(u32(extension)),string(u32(extension+4)))
                if kind==0x100:namespaces.append(pair)
                elif not namespaces or namespaces.pop()!=pair:raise ValueError('AXML namespace mismatch')
            elif kind==0x102:
                check(extension,20,stop)
                namespace,name,start,stride,count,id_,class_,style=struct.unpack_from('<II6H',data,extension)
                if start<20 or stride<20 or any(x>count for x in [id_,class_,style]):raise ValueError('AXML attribute extension')
                check(extension+start,count*stride,stop)
                node={'name':expanded(namespace,name),'attributes':[],'children':[]}
                for n in range(count):
                    position=extension+start+n*stride
                    ns,attr,raw,size,res0,typ,value=struct.unpack_from('<IIIHBBI',data,position)
                    if size!=8 or res0:raise ValueError('AXML typed-value header')
                    if typ==3:typed=string(value)
                    elif typ==0x10:typed=str(value if value<0x80000000 else value-0x100000000)
                    elif typ==0x12 and value in [0,1]:typed='true' if value else 'false'
                    else:raise ValueError('unsupported fixture typed value')
                    text=string(raw,True)
                    if text is None:text=typed
                    elif text!=typed:raise ValueError('fixture raw/typed value disagreement')
                    node['attributes'].append({'name':expanded(ns,attr),'value':text,'typed_type':typ,'typed_data':value,
                                               'resource_id':resources[attr] if attr<len(resources) else 0})
                if stack:stack[-1]['children'].append(node)
                elif root is None:root=node
                else:raise ValueError('second AXML root')
                stack.append(node)
            else:
                check(extension,8,stop)
                if not stack or stack.pop()['name']!=expanded(u32(extension),u32(extension+4)):raise ValueError('AXML element mismatch')
        else:raise ValueError('unsupported AXML fixture chunk')
        cursor=stop
    if stack or namespaces or root is None:raise ValueError('incomplete AXML document')
    return {'producer':'independent-AOSP-ResourceTypes-layout-reader','strings':strings,'resource_map':resources,'root':root}


def certify(root,cache,report):
    from e2e import load,save,sha
    supplementary=report['supplemental_inputs']
    for path,digest in supplementary['source_sha256'].items():
        if sha(root/path)!=digest:raise ValueError('changed H03 source after preparation')
    directory=cache/'prepared/H03'
    binary=(directory/'AndroidManifest.axml').read_bytes()
    projection=project(binary)
    with zipfile.ZipFile(directory/'manifest.zip') as archive:
        if archive.namelist()!=['AndroidManifest.xml'] or archive.read('AndroidManifest.xml')!=binary:raise ValueError('H03 ZIP/AXML identity differs')
    source=ET.parse(root/'tests/e2e/sources/H03/AndroidManifest.xml').getroot()
    def original(node):return {'name':node.tag,'attributes':list(node.attrib.items()),'children':[original(c) for c in node]}
    def decoded(node):return {'name':node['name'],'attributes':[(a['name'],a['value']) for a in node['attributes']],'children':[decoded(c) for c in node['children']]}
    if original(source)!=decoded(projection['root']):raise ValueError('binary XML and independent source document differ')
    save(directory/'projection.json',projection)
    save(directory/'certificate.json',{'producer':'B03','source_sha256':sha(root/'tests/e2e/sources/H03/AndroidManifest.xml'),
         'axml_sha256':sha(directory/'AndroidManifest.axml'),'zip_sha256':sha(directory/'manifest.zip'),'result':'input-certified',
         'scope':'H03 producer input and source equivalence only; no extra D case or engine manifest acceptance'})
