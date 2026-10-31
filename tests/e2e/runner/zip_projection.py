"""Independent central/local ZIP and plaintext projection, never an extractor."""
from __future__ import annotations
import bz2
import hashlib
from pathlib import Path
import struct
import zipfile
import zlib


def project_zip(path):
    data = Path(path).read_bytes()
    records = []
    with zipfile.ZipFile(path) as archive:
        for ordinal, info in enumerate(archive.infolist()):
            offset = info.header_offset
            if data[offset:offset + 4] != b'PK\x03\x04' or offset + 30 > len(data):
                raise ValueError('invalid ZIP local header')
            version, flags, method, tm, date, crc, compressed, uncompressed, names, extra = struct.unpack_from('<5H3I2H', data, offset + 4)
            start = offset + 30 + names + extra
            if (flags,method,crc,compressed,uncompressed) != (info.flag_bits,info.compress_type,info.CRC,info.compress_size,info.file_size):
                raise ValueError('central/local ZIP fields disagree')
            if start > len(data) or compressed > len(data) - start:
                raise ValueError('ZIP data range')
            raw = data[start:start + compressed]
            encryption_header = None
            if flags & 1:
                if compressed < 12 or flags & (8 | 64):
                    raise ValueError('not the specified traditional ZipCrypto profile')
                decrypt = zipfile._ZipDecrypter(b'fixture')
                encryption_header = list(decrypt(raw[:12]))
                raw = decrypt(raw[12:])
            if method == 0:
                plain = raw
            elif method == 8:
                decompressor = zlib.decompressobj(-15)
                plain = decompressor.decompress(raw, uncompressed + 1)
                if not decompressor.eof or decompressor.unused_data or decompressor.unconsumed_tail:
                    raise ValueError('incomplete/extra deflate data')
            elif method == 12:
                plain = bz2.decompress(raw)
            else:
                raise ValueError('uncertified compression method')
            if len(plain) != uncompressed:
                raise ValueError('uncompressed ZIP size mismatch')
            actual_crc = zlib.crc32(plain) & 0xffffffff
            # zipfile is a second independent high-level plaintext path. A bad
            # CRC must fail there rather than masquerade as a successful read.
            try:
                library_plain = archive.read(info, pwd=b'fixture' if flags & 1 else None)
            except zipfile.BadZipFile:
                if actual_crc == crc:
                    raise
                library_plain = None
            if library_plain is not None and library_plain != plain:
                raise ValueError('independent ZIP plaintext disagreement')
            record = {'ordinal':ordinal,'name':info.filename,'method':method,'version_needed':version,'flags':flags,
                      'local_header_offset':offset,'data_offset':start,'compressed_size':compressed,'uncompressed_size':uncompressed,
                      'declared_crc':crc,'actual_crc':actual_crc,'crc_valid':actual_crc==crc,
                      'plaintext_sha256':hashlib.sha256(plain).hexdigest(),'encryption_header':encryption_header,
                      'plaintext':list(plain) if len(plain) <= 4096 else None}
            if len(plain) > 4096:
                multiplier, add = (plain[1] - plain[0]) % 256, plain[0]
                pattern = bytes((i * multiplier + add) % 256 for i in range(256))
                if plain != (pattern * ((len(plain) + 255) // 256))[:len(plain)]:
                    raise ValueError('large fixture has an unproved payload pattern')
                record['pattern'] = {'length':len(plain),'multiplier':multiplier,'add':add}
            records.append(record)
    if not records:
        raise ValueError('empty ZIP certification')
    return {'producer':'independent-struct-and-python-zipfile-zlib-bz2','entries':records}
