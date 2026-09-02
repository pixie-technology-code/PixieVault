const fs = require('fs');
const path = require('path');

const iconsDir = path.join(__dirname, '..', 'src-tauri', 'icons');
const png32 = fs.readFileSync(path.join(iconsDir, '32x32.png'));
const png128 = fs.readFileSync(path.join(iconsDir, '128x128.png'));
const png256 = fs.readFileSync(path.join(iconsDir, '128x128@2x.png'));
const png512 = fs.readFileSync(path.join(iconsDir, 'icon.png'));

function makeChunk(tag, data) {
  const header = Buffer.alloc(8);
  header.write(tag, 0, 4, 'ascii');
  header.writeUInt32BE(data.length + 8, 4);
  return Buffer.concat([header, data]);
}

const chunks = [
  makeChunk('icp5', png32),
  makeChunk('ic07', png128),
  makeChunk('ic08', png256),
  makeChunk('ic09', png512),
];

const totalBody = Buffer.concat(chunks);
const icnsHeader = Buffer.alloc(8);
icnsHeader.write('icns', 0, 4, 'ascii');
icnsHeader.writeUInt32BE(totalBody.length + 8, 4);

const icnsFile = Buffer.concat([icnsHeader, totalBody]);
fs.writeFileSync(path.join(iconsDir, 'icon.icns'), icnsFile);

console.log(`✓ Generated valid Apple ICNS file: ${icnsFile.length} bytes`);
