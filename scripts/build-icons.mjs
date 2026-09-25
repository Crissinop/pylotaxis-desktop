// Genera le icone dell'app da branding/app-icon.svg (v0.1.0).
//
// Perché uno script e non solo `tauri icon`: a 16–48 px il ridimensionamento del
// vettoriale sfuoca le tessere. Qui le piccole dimensioni si disegnano su una griglia
// intera di pixel (con la chiave di volta quadrata, come prevede il logo sotto i 32 px),
// e l'.ico include anche 20 e 40 px, usati da Windows con la scala al 125% e al 150%.
//
// Uso: node scripts/build-icons.mjs   (nessuna dipendenza oltre alla CLI di Tauri)

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const OUT = join(ROOT, 'src-tauri', 'icons');
const SOURCE = join(ROOT, 'branding', 'app-icon.svg');

const BASALTO = [0x1a, 0x1f, 0x28];
const MARMO = [0xe4, 0xe7, 0xe0];
const VERDERAME_CHIARO = [0x5b, 0xbb, 0xa9];

/**
 * Geometria in pixel interi per le dimensioni piccole: margine, lato della tessera,
 * spazio tra tessere, raggio del contenitore e delle tessere.
 * Vincolo: margin * 2 + tile * 3 + gap * 2 === size.
 */
const PIXEL_SPECS = [
  { size: 16, margin: 1, tile: 4, gap: 1, radius: 3, tileRadius: 0 },
  { size: 20, margin: 2, tile: 4, gap: 2, radius: 4, tileRadius: 0 },
  { size: 24, margin: 2, tile: 6, gap: 1, radius: 5, tileRadius: 0 },
  { size: 32, margin: 4, tile: 6, gap: 3, radius: 7, tileRadius: 1 },
  { size: 40, margin: 4, tile: 8, gap: 4, radius: 9, tileRadius: 1 },
  { size: 48, margin: 6, tile: 10, gap: 3, radius: 10, tileRadius: 1.5 },
];

/** Dimensioni generate dal vettoriale, dove il trapezio della chiave di volta si legge. */
const VECTOR_SIZES = [64, 128, 256];

/** Posizioni del Π nella griglia 3×3: [colonna, riga]. La [1, 0] è la chiave di volta. */
const TILES = [
  [0, 0],
  [2, 0],
  [0, 1],
  [2, 1],
  [0, 2],
  [2, 2],
];
const KEYSTONE = [1, 0];

/** Sottocampioni per lato: bordi dritti esatti, angoli arrotondati senza scalini. */
const SUPERSAMPLE = 8;

function insideRoundedRect(px, py, x, y, w, h, r) {
  if (px < x || py < y || px > x + w || py > y + h) return false;
  if (r <= 0) return true;
  const cx = Math.min(Math.max(px, x + r), x + w - r);
  const cy = Math.min(Math.max(py, y + r), y + h - r);
  return (px - cx) ** 2 + (py - cy) ** 2 <= r * r;
}

function drawPixelIcon(spec) {
  const { size, margin, tile, gap, radius, tileRadius } = spec;
  if (margin * 2 + tile * 3 + gap * 2 !== size) {
    throw new Error(`Geometria non valida per ${size} px`);
  }
  const origin = (index) => margin + index * (tile + gap);
  const shapes = [
    ...TILES.map(([c, r]) => ({ x: origin(c), y: origin(r), color: MARMO })),
    { x: origin(KEYSTONE[0]), y: origin(KEYSTONE[1]), color: VERDERAME_CHIARO },
  ];

  const rgba = Buffer.alloc(size * size * 4);
  const step = 1 / SUPERSAMPLE;
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      let covered = 0;
      for (let sy = 0; sy < SUPERSAMPLE; sy += 1) {
        for (let sx = 0; sx < SUPERSAMPLE; sx += 1) {
          const px = x + (sx + 0.5) * step;
          const py = y + (sy + 0.5) * step;
          if (!insideRoundedRect(px, py, 0, 0, size, size, radius)) continue;
          const hit = shapes.find((s) =>
            insideRoundedRect(px, py, s.x, s.y, tile, tile, tileRadius),
          );
          const color = hit ? hit.color : BASALTO;
          r += color[0];
          g += color[1];
          b += color[2];
          covered += 1;
        }
      }
      const offset = (y * size + x) * 4;
      if (covered > 0) {
        rgba[offset] = Math.round(r / covered);
        rgba[offset + 1] = Math.round(g / covered);
        rgba[offset + 2] = Math.round(b / covered);
        rgba[offset + 3] = Math.round((covered / SUPERSAMPLE ** 2) * 255);
      }
    }
  }
  return encodePng(size, size, rgba);
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function encodePng(width, height, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit per canale
  header[9] = 6; // RGBA
  const rows = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    rgba.copy(rows, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(rows, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** File .ico con voci PNG, il formato supportato da Windows dalla Vista in poi. */
function encodeIco(entries) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  let offset = 6 + entries.length * 16;
  const directory = entries.map(({ size, png }) => {
    const entry = Buffer.alloc(16);
    entry[0] = size >= 256 ? 0 : size; // 0 significa 256
    entry[1] = size >= 256 ? 0 : size;
    entry.writeUInt16LE(1, 4); // piani di colore
    entry.writeUInt16LE(32, 6); // bit per pixel
    entry.writeUInt32LE(png.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += png.length;
    return entry;
  });
  return Buffer.concat([header, ...directory, ...entries.map((e) => e.png)]);
}

function renderVectorSizes() {
  const dir = mkdtempSync(join(tmpdir(), 'icons-'));
  try {
    const cli = join(ROOT, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
    execFileSync(process.execPath, [cli, 'icon', SOURCE, '-o', dir, '-p', VECTOR_SIZES.join(',')], {
      stdio: 'ignore',
    });
    return Object.fromEntries(
      VECTOR_SIZES.map((size) => [size, readFileSync(join(dir, `${size}x${size}.png`))]),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const pixel = Object.fromEntries(PIXEL_SPECS.map((spec) => [spec.size, drawPixelIcon(spec)]));
const vector = renderVectorSizes();

writeFileSync(join(OUT, '32x32.png'), pixel[32]);
writeFileSync(join(OUT, '128x128.png'), vector[128]);
writeFileSync(join(OUT, '128x128@2x.png'), vector[256]);
writeFileSync(
  join(OUT, 'icon.ico'),
  encodeIco([
    ...PIXEL_SPECS.map(({ size }) => ({ size, png: pixel[size] })),
    { size: 64, png: vector[64] },
    { size: 256, png: vector[256] },
  ]),
);
console.log('Icone generate in src-tauri/icons');
