// Maintainer-only asset generation. Runtime/builds use the committed ICO.
// npm install --no-save sharp; node tools/generate-icon.cjs
const fs = require('node:fs');
const path = require('node:path');
const sharp = require(process.env.OICHA_SHARP_PATH || 'sharp');
const root = path.resolve(__dirname, '..', 'crates/oicha-app/assets/branding');
const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
(async () => {
  const frames = await Promise.all(sizes.map(size => sharp(path.join(root, 'oicha.svg')).resize(size, size).png().toBuffer()));
  const header = Buffer.alloc(6 + 16 * sizes.length);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(sizes.length, 4);
  let offset = header.length;
  frames.forEach((png, i) => {
    const position = 6 + 16 * i;
    header[position] = header[position + 1] = sizes[i] === 256 ? 0 : sizes[i];
    header.writeUInt16LE(1, position + 4);
    header.writeUInt16LE(32, position + 6);
    header.writeUInt32LE(png.length, position + 8);
    header.writeUInt32LE(offset, position + 12);
    offset += png.length;
  });
  fs.writeFileSync(path.join(root, 'oicha.ico'), Buffer.concat([header, ...frames]));
  await sharp(path.join(root, 'oicha.svg')).resize(512, 512).png().toFile(path.join(root, 'oicha.png'));
  async function bitmap(svg, width, height, filename) {
    const pixels = await sharp(Buffer.from(svg)).resize(width, height).flatten({ background: '#ffffff' }).removeAlpha().raw().toBuffer();
    const stride = Math.ceil(width * 3 / 4) * 4;
    const bmp = Buffer.alloc(54 + stride * height);
    bmp.write('BM'); bmp.writeUInt32LE(bmp.length, 2); bmp.writeUInt32LE(54, 10);
    bmp.writeUInt32LE(40, 14); bmp.writeInt32LE(width, 18); bmp.writeInt32LE(height, 22);
    bmp.writeUInt16LE(1, 26); bmp.writeUInt16LE(24, 28); bmp.writeUInt32LE(stride * height, 34);
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
      const source = (y * width + x) * 3, target = 54 + (height - 1 - y) * stride + x * 3;
      bmp[target] = pixels[source + 2]; bmp[target + 1] = pixels[source + 1]; bmp[target + 2] = pixels[source];
    }
    fs.writeFileSync(path.join(root, filename), bmp);
  }
  const symbol = fs.readFileSync(path.join(root, 'oicha.svg'), 'utf8').replace('<svg ', '<svg x="38" y="77" width="252" height="252" ');
  const wizard = `<svg xmlns="http://www.w3.org/2000/svg" width="328" height="628" viewBox="0 0 328 628"><rect width="328" height="628" fill="#315d49"/>${symbol}<text x="164" y="388" text-anchor="middle" font-family="Georgia,serif" font-size="48" fill="#fff9e9">oicha.</text><path d="M116 426h96" stroke="#dcb979" stroke-width="3"/><text x="164" y="470" text-anchor="middle" font-family="Segoe UI,sans-serif" font-size="18" fill="#dce8da">Viết nên câu chuyện của bạn</text></svg>`;
  fs.writeFileSync(path.join(root, 'installer-wizard.svg'), wizard);
  await bitmap(wizard, 328, 628, 'installer-wizard.bmp');
  console.log(`Generated ${sizes.length} icon sizes and preview`);
})().catch(error => { console.error(error); process.exitCode = 1; });
