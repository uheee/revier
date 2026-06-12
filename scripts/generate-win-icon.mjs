/* global Buffer, console */

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

const outputPath = 'build/icon.ico';
const sizes = [16, 32, 48, 256];

function uint16(value) {
  const buffer = Buffer.alloc(2);
  buffer.writeUInt16LE(value);
  return buffer;
}

function uint32(value) {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(value);
  return buffer;
}

function bitmapInfoHeader(width, height) {
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(width, 4);
  header.writeInt32LE(height * 2, 8);
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  header.writeUInt32LE(0, 16);
  header.writeUInt32LE(width * height * 4, 20);
  header.writeInt32LE(0, 24);
  header.writeInt32LE(0, 28);
  header.writeUInt32LE(0, 32);
  header.writeUInt32LE(0, 36);
  return header;
}

function isInsideLogoMark(x, y, size) {
  const normalizedX = x / size;
  const normalizedY = y / size;
  const stem = normalizedX > 0.25
    && normalizedX < 0.38
    && normalizedY > 0.22
    && normalizedY < 0.78;
  const bowl = normalizedX > 0.34
    && normalizedX < 0.68
    && normalizedY > 0.22
    && normalizedY < 0.48
    && (normalizedY < 0.34 || normalizedX > 0.58);
  const leg = Math.abs(normalizedY - 0.48 - 1.15 * (normalizedX - 0.37)) < 0.055
    && normalizedX > 0.37
    && normalizedX < 0.72
    && normalizedY > 0.48
    && normalizedY < 0.78;
  return stem || bowl || leg;
}

function pixelColor(x, y, size) {
  const center = (size - 1) / 2;
  const radius = size * 0.48;
  const dx = x - center;
  const dy = y - center;
  const distance = Math.sqrt(dx * dx + dy * dy);

  if (distance > radius) {
    return [0, 0, 0, 0];
  }

  if (isInsideLogoMark(x, y, size)) {
    return [255, 255, 255, 255];
  }

  const gradient = (x + y) / (2 * (size - 1));
  const red = Math.round(38 + 70 * gradient);
  const green = Math.round(83 + 40 * gradient);
  const blue = Math.round(160 + 65 * gradient);
  return [blue, green, red, 255];
}

function createDib(size) {
  const pixels = [];

  for (let row = size - 1; row >= 0; row -= 1) {
    for (let column = 0; column < size; column += 1) {
      pixels.push(...pixelColor(column, size - 1 - row, size));
    }
  }

  const maskRowBytes = Math.ceil(size / 32) * 4;
  const transparentMask = Buffer.alloc(maskRowBytes * size);
  return Buffer.concat([
    bitmapInfoHeader(size, size),
    Buffer.from(pixels),
    transparentMask,
  ]);
}

function createIcon() {
  const images = sizes.map(createDib);
  const headerSize = 6;
  const directoryEntrySize = 16;
  let imageOffset = headerSize + directoryEntrySize * images.length;

  const header = Buffer.concat([uint16(0), uint16(1), uint16(images.length)]);
  const directoryEntries = images.map((image, index) => {
    const size = sizes[index];
    const entry = Buffer.concat([
      Buffer.from([size === 256 ? 0 : size, size === 256 ? 0 : size, 0, 0]),
      uint16(1),
      uint16(32),
      uint32(image.length),
      uint32(imageOffset),
    ]);
    imageOffset += image.length;
    return entry;
  });

  return Buffer.concat([header, ...directoryEntries, ...images]);
}

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, createIcon());
console.log(`Generated ${outputPath}`);
