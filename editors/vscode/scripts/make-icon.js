"use strict";

const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const SIZE = 128;
const SUPER = 4;

const SQUARE = { x: 8, y: 8, w: 112, h: 112, r: 20, color: [0x17, 0x16, 0x14, 255] };
const ACCENT = [0xe0, 0x8b, 0x45, 255];

const A_POLYGON = [
  [64, 28],
  [100, 100],
  [84, 100],
  [64, 56],
  [44, 100],
  [28, 100],
];

const BAR = { x: 54, y: 82, w: 20, h: 10, r: 3 };

function inRoundedRect(px, py, rect) {
  const { x, y, w, h, r } = rect;
  if (px < x || px > x + w || py < y || py > y + h) {
    return false;
  }
  const cx = Math.max(x + r, Math.min(px, x + w - r));
  const cy = Math.max(y + r, Math.min(py, y + h - r));
  const dx = px - cx;
  const dy = py - cy;
  return dx * dx + dy * dy <= r * r || (px >= x + r && px <= x + w - r) || (py >= y + r && py <= y + h - r);
}

function inPolygon(px, py, polygon) {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const [xi, yi] = polygon[i];
    const [xj, yj] = polygon[j];
    const intersects = yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi;
    if (intersects) {
      inside = !inside;
    }
  }
  return inside;
}

function coverage(px, py, test) {
  let hits = 0;
  for (let sy = 0; sy < SUPER; sy++) {
    for (let sx = 0; sx < SUPER; sx++) {
      const sampleX = px + (sx + 0.5) / SUPER;
      const sampleY = py + (sy + 0.5) / SUPER;
      if (test(sampleX, sampleY)) {
        hits += 1;
      }
    }
  }
  return hits / (SUPER * SUPER);
}

function blend(base, over, alpha) {
  return [
    Math.round(over[0] * alpha + base[0] * (1 - alpha)),
    Math.round(over[1] * alpha + base[1] * (1 - alpha)),
    Math.round(over[2] * alpha + base[2] * (1 - alpha)),
    Math.min(255, Math.round(over[3] * alpha + base[3] * (1 - alpha))),
  ];
}

function renderPixels() {
  const pixels = Buffer.alloc(SIZE * SIZE * 4);
  for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
      const squareAlpha = coverage(x, y, (sx, sy) => inRoundedRect(sx, sy, SQUARE));
      let color = [0, 0, 0, 0];
      if (squareAlpha > 0) {
        color = [SQUARE.color[0], SQUARE.color[1], SQUARE.color[2], Math.round(255 * squareAlpha)];
      }
      const markAlpha = coverage(
        x,
        y,
        (sx, sy) => inPolygon(sx, sy, A_POLYGON) || inRoundedRect(sx, sy, BAR)
      );
      if (markAlpha > 0) {
        color = blend(color, ACCENT, markAlpha);
      }
      const offset = (y * SIZE + x) * 4;
      pixels[offset] = color[0];
      pixels[offset + 1] = color[1];
      pixels[offset + 2] = color[2];
      pixels[offset + 3] = color[3];
    }
  }
  return pixels;
}

const CRC_TABLE = (() => {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c;
  }
  return table;
})();

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([length, body, crc]);
}

function encodePng(pixels) {
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(SIZE, 0);
  ihdr.writeUInt32BE(SIZE, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
  for (let y = 0; y < SIZE; y++) {
    raw[y * (SIZE * 4 + 1)] = 0;
    pixels.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
  }
  const idat = zlib.deflateSync(raw, { level: 9 });
  return Buffer.concat([signature, chunk("IHDR", ihdr), chunk("IDAT", idat), chunk("IEND", Buffer.alloc(0))]);
}

function main() {
  const outPath = path.join(__dirname, "..", "media", "icon.png");
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  const png = encodePng(renderPixels());
  fs.writeFileSync(outPath, png);
  process.stdout.write(`wrote ${outPath} (${png.length} bytes)\n`);
}

main();
