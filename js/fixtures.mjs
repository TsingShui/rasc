/**
 * DEX and ZIP fixtures for the host boundary suite, built here rather than
 * downloaded.
 *
 * The wasm module refuses anything that is not a DEX inside an archive, so a
 * boundary test needs real DEXes. These are built from the same shapes the Rust
 * fixtures in `src/dex/mod.rs` use - a class per entry, one method per class,
 * one `const-string` in each body - with the names, the values and the class
 * count as parameters, because several scenarios need a class index that a text
 * row cannot survive (a name with `|`, a quote, a newline, an emoji).
 *
 * Usage (writes the whole fixture set for a manual session):
 *   node js/fixtures.mjs /tmp/rasc-fixtures
 */

function writeU32(data, offset, value) {
  data[offset] = value & 0xff;
  data[offset + 1] = (value >>> 8) & 0xff;
  data[offset + 2] = (value >>> 16) & 0xff;
  data[offset + 3] = (value >>> 24) & 0xff;
}

function writeU16(data, offset, value) {
  data[offset] = value & 0xff;
  data[offset + 1] = (value >>> 8) & 0xff;
}

function pushUleb(out, value) {
  let remaining = value;
  for (;;) {
    let byte = remaining & 0x7f;
    remaining >>>= 7;
    if (remaining !== 0) byte |= 0x80;
    out.push(byte);
    if (remaining === 0) return;
  }
}

/**
 * One UTF-16 code unit at a time, which is exactly MUTF-8: a supplementary
 * character is written as its two surrogate halves, and that is the encoding a
 * DEX string table stores.
 *
 * The units become bytes here rather than being pushed raw: a plain array accepted
 * `0xD83D` happily, and `Uint8Array.set` then truncated it to `0x3D`, so every
 * emoji in a fixture quietly became an `=`. The engine was right and the fixture
 * was wrong; this is the difference.
 */
function pushMutf8(out, text) {
  for (let index = 0; index < text.length; index += 1) {
    const unit = text.charCodeAt(index);
    if (unit === 0) {
      out.push(0xc0, 0x80);
    } else if (unit < 0x80) {
      out.push(unit);
    } else if (unit < 0x800) {
      out.push(0xc0 | (unit >> 6), 0x80 | (unit & 0x3f));
    } else {
      out.push(0xe0 | (unit >> 12), 0x80 | ((unit >> 6) & 0x3f), 0x80 | (unit & 0x3f));
    }
  }
}

function adler32(bytes) {
  let a = 1;
  let b = 0;
  for (let index = 12; index < bytes.length; index += 1) {
    a = (a + bytes[index]) % 65521;
    b = (b + a) % 65521;
  }
  return ((b << 16) | a) >>> 0;
}

/** The checksum the decoder validates before parsing anything. */
function sealDex(out) {
  writeU32(out, 0x08, adler32(out));
  return out;
}

/**
 * A DEX of `classCount` classes, each with one direct method whose body is
 * `const-string v0, value` followed by `return-void`.
 *
 * `descriptor(index)` and `methodName(index)` may return anything MUTF-8 can
 * hold; the class definitions do not validate them, which is the point.
 */
export function fixtureDex({
  classCount = 3,
  value = "Authorization",
  descriptor = (index) => `Lcom/example/Fixture${index};`,
  methodName = (index) => `m${index}`,
  extraStrings = [],
} = {}) {
  const headerSize = 0x70;
  const descriptors = Array.from({ length: classCount }, (_, index) => descriptor(index));
  const methods = Array.from({ length: classCount }, (_, index) => methodName(index));
  const strings = [value, ...descriptors, "V", ...methods, ...extraStrings];

  const stringIdsOff = headerSize;
  const typeIdsOff = stringIdsOff + strings.length * 4;
  const protoIdsOff = typeIdsOff + (classCount + 2) * 4;
  const methodIdsOff = protoIdsOff + 12;
  const classDefsOff = methodIdsOff + classCount * 8;
  const dataOff = classDefsOff + classCount * 32;

  const data = [];
  const stringOffsets = [];
  for (const text of strings) {
    stringOffsets.push(dataOff + data.length);
    pushUleb(data, text.length);
    pushMutf8(data, text);
    data.push(0);
  }

  const classDataOffsets = [];
  for (let index = 0; index < classCount; index += 1) {
    while ((dataOff + data.length) % 4 !== 0) data.push(0);
    const codeOff = dataOff + data.length;
    data.push(1, 0); // registers_size: v0
    data.push(0, 0); // ins_size
    data.push(0, 0); // outs_size
    data.push(0, 0); // tries_size
    data.push(0, 0, 0, 0); // debug_info_off
    // Two loads of the same string in one method: a count of uses would say two, a count of
    // methods says one, and only a fixture with both in one body can tell them apart.
    data.push(5, 0, 0, 0); // insns_size, in code units
    data.push(0x1a, 0x00, 0x00, 0x00); // const-string v0, #0
    data.push(0x1a, 0x00, 0x00, 0x00); // const-string v0, #0 (the same string again)
    data.push(0x0e, 0x00); // return-void

    classDataOffsets.push(dataOff + data.length);
    data.push(0); // static_fields_size
    data.push(0); // instance_fields_size
    data.push(1); // direct_methods_size
    data.push(0); // virtual_methods_size
    pushUleb(data, index); // method_idx_diff
    pushUleb(data, 0); // access_flags
    pushUleb(data, codeOff);
  }

  const total = dataOff + data.length;
  const out = new Uint8Array(total);
  out.set([..."dex\n039\0"].map((character) => character.charCodeAt(0)), 0);
  writeU32(out, 0x20, total);
  writeU32(out, 0x24, headerSize);
  writeU32(out, 0x28, 0x12345678);
  writeU32(out, 0x38, strings.length);
  writeU32(out, 0x3c, stringIdsOff);
  writeU32(out, 0x40, classCount + 2);
  writeU32(out, 0x44, typeIdsOff);
  writeU32(out, 0x48, 1);
  writeU32(out, 0x4c, protoIdsOff);
  writeU32(out, 0x58, classCount);
  writeU32(out, 0x5c, methodIdsOff);
  writeU32(out, 0x60, classCount);
  writeU32(out, 0x64, classDefsOff);

  for (let index = 0; index < stringOffsets.length; index += 1) {
    writeU32(out, stringIdsOff + index * 4, stringOffsets[index]);
  }
  writeU32(out, typeIdsOff, 0);
  writeU32(out, typeIdsOff + (classCount + 1) * 4, classCount + 1);
  writeU32(out, protoIdsOff, classCount + 1);
  writeU32(out, protoIdsOff + 4, classCount + 1);
  writeU32(out, protoIdsOff + 8, 0);

  for (let index = 0; index < classCount; index += 1) {
    writeU32(out, typeIdsOff + (index + 1) * 4, index + 1);
    const method = methodIdsOff + index * 8;
    writeU16(out, method, index + 1);
    writeU16(out, method + 2, 0);
    writeU32(out, method + 4, classCount + 2 + index);
    const classDef = classDefsOff + index * 32;
    writeU32(out, classDef, index + 1);
    writeU32(out, classDef + 4, 1);
    writeU32(out, classDef + 8, 0xffffffff);
    writeU32(out, classDef + 16, 0xffffffff);
    writeU32(out, classDef + 24, classDataOffsets[index]);
  }

  out.set(data, dataOff);
  return sealDex(out);
}

/** The string table of a fixture DEX, in index order, as the builder wrote it. */
export function fixtureStrings({
  classCount = 3,
  value = "Authorization",
  descriptor = (index) => `Lcom/example/Fixture${index};`,
  methodName = (index) => `m${index}`,
  extraStrings = [],
} = {}) {
  const descriptors = Array.from({ length: classCount }, (_, index) => descriptor(index));
  const methods = Array.from({ length: classCount }, (_, index) => methodName(index));
  return [value, ...descriptors, "V", ...methods, ...extraStrings];
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    table[index] = value >>> 0;
  }
  return table;
})();

function crc32(bytes) {
  let value = 0xffffffff;
  for (const byte of bytes) value = CRC_TABLE[(value ^ byte) & 0xff] ^ (value >>> 8);
  return (value ^ 0xffffffff) >>> 0;
}

/** A STORE-only ZIP: rasc reads the central directory and copies a stored entry. */
export function makeArchive(files) {
  const encoder = new TextEncoder();
  const entries = files.map(([name, data]) => ({
    name: encoder.encode(name),
    bytes: data,
    crc: crc32(data),
  }));

  const chunks = [];
  const central = [];
  let offset = 0;
  for (const entry of entries) {
    const local = new Uint8Array(30 + entry.name.length + entry.bytes.length);
    const view = new DataView(local.buffer);
    view.setUint32(0, 0x04034b50, true);
    view.setUint16(4, 20, true);
    view.setUint16(6, 0x0800, true);
    view.setUint32(14, entry.crc, true);
    view.setUint32(18, entry.bytes.length, true);
    view.setUint32(22, entry.bytes.length, true);
    view.setUint16(26, entry.name.length, true);
    local.set(entry.name, 30);
    local.set(entry.bytes, 30 + entry.name.length);
    chunks.push(local);
    central.push({ entry, offset });
    offset += local.length;
  }

  const centralStart = offset;
  for (const { entry, offset: at } of central) {
    const row = new Uint8Array(46 + entry.name.length);
    const view = new DataView(row.buffer);
    view.setUint32(0, 0x02014b50, true);
    view.setUint16(4, 20, true);
    view.setUint16(6, 20, true);
    view.setUint16(8, 0x0800, true);
    view.setUint32(16, entry.crc, true);
    view.setUint32(20, entry.bytes.length, true);
    view.setUint32(24, entry.bytes.length, true);
    view.setUint16(28, entry.name.length, true);
    view.setUint32(42, at, true);
    row.set(entry.name, 46);
    chunks.push(row);
    offset += row.length;
  }

  const end = new Uint8Array(22);
  const endView = new DataView(end.buffer);
  endView.setUint32(0, 0x06054b50, true);
  endView.setUint16(8, entries.length, true);
  endView.setUint16(10, entries.length, true);
  endView.setUint32(12, offset - centralStart, true);
  endView.setUint32(16, centralStart, true);
  chunks.push(end);

  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(total);
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.length;
  }
  return out;
}

/** An APK holding one DEX, which is what the native CLI is pointed at. */
export function apkOf(dex, name = "classes.dex") {
  return makeArchive([
    ["AndroidManifest.xml", new TextEncoder().encode("<manifest />")],
    [name, dex],
  ]);
}

/** An APK whose class index is spread over `count` DEX entries. */
export function multiEntryApk(count = 8, options = {}) {
  const files = [["AndroidManifest.xml", new TextEncoder().encode("<manifest />")]];
  for (let index = 1; index <= count; index += 1) {
    const name = index === 1 ? "classes.dex" : `classes${index}.dex`;
    files.push([
      name,
      fixtureDex({
        classCount: 2,
        descriptor: (slot) => `Lcom/example/E${index}C${slot};`,
        methodName: (slot) => `m${slot}`,
        ...options,
      }),
    ]);
  }
  return makeArchive(files);
}

/** Bytes that are neither a DEX nor an archive. */
export function junkFile(size = 4096) {
  const out = new Uint8Array(size);
  for (let index = 0; index < size; index += 1) out[index] = (index * 31 + 7) & 0xff;
  // Not a ZIP magic, and not a DEX magic either.
  out.set([0x4d, 0x5a], 0);
  return out;
}

/** The names and sizes of a ZIP's entries, read the way its directory states them. */
export function centralDirectory(zip) {
  const view = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
  const eocd = zip.length - 22;
  const count = view.getUint16(eocd + 10, true);
  let offset = view.getUint32(eocd + 16, true);
  const entries = [];
  const decoder = new TextDecoder();
  for (let index = 0; index < count; index += 1) {
    const nameLength = view.getUint16(offset + 28, true);
    const extraLength = view.getUint16(offset + 30, true);
    const commentLength = view.getUint16(offset + 32, true);
    entries.push({
      name: decoder.decode(zip.subarray(offset + 46, offset + 46 + nameLength)),
      method: view.getUint16(offset + 10, true),
      compressed: view.getUint32(offset + 20, true),
      uncompressed: view.getUint32(offset + 24, true),
    });
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return entries;
}

if (process.argv[1] !== undefined && import.meta.url === new URL(`file://${process.argv[1].startsWith("/") ? process.argv[1] : `${process.cwd()}/${process.argv[1]}`}`).href) {
  const { mkdirSync, writeFileSync } = await import("node:fs");
  const destination = process.argv[2] ?? ".";
  mkdirSync(destination, { recursive: true });
  const plain = fixtureDex({ classCount: 4 });
  const hostile = fixtureDex({
    classCount: 3,
    descriptor: (index) =>
      ["Lcom/example/Pipe|Inside;", 'Lcom/example/Quote"Inside;', "Lcom/example/New\nLine;"][index],
    methodName: (index) => ["a|b", 'c"d', "e\nf"][index],
  });
  writeFileSync(`${destination}/plain.dex`, plain);
  writeFileSync(`${destination}/plain.apk`, apkOf(plain));
  writeFileSync(`${destination}/hostile.dex`, hostile);
  writeFileSync(`${destination}/multi.apk`, multiEntryApk(8));
  console.log(`wrote plain.dex, plain.apk, hostile.dex, multi.apk to ${destination}`);
}
