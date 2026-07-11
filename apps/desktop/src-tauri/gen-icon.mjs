// Gera um PNG 1024x1024 (gradiente evergreen — "perene") sem dependências.
// Usado só para alimentar `tauri icon`. Descartável.
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const SIZE = 1024;

// CRC32 (tabela).
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const body = Buffer.concat([typeBuf, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}
function lerp(a, b, t) {
  return Math.round(a + (b - a) * t);
}

// Gradiente diagonal de #0d3b2e (verde escuro) para #10b981 (esmeralda).
const c0 = [0x0d, 0x3b, 0x2e];
const c1 = [0x10, 0xb9, 0x81];

const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
let p = 0;
for (let y = 0; y < SIZE; y++) {
  raw[p++] = 0; // filtro None por scanline
  for (let x = 0; x < SIZE; x++) {
    const t = (x + y) / (2 * SIZE);
    raw[p++] = lerp(c0[0], c1[0], t);
    raw[p++] = lerp(c0[1], c1[1], t);
    raw[p++] = lerp(c0[2], c1[2], t);
    raw[p++] = 255;
  }
}

const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;
const idat = deflateSync(raw, { level: 9 });
const png = Buffer.concat([
  sig,
  chunk("IHDR", ihdr),
  chunk("IDAT", idat),
  chunk("IEND", Buffer.alloc(0)),
]);
writeFileSync(new URL("./icon-source.png", import.meta.url), png);
console.log("icon-source.png:", png.length, "bytes");
