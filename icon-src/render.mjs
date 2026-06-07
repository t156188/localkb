import sharp from "sharp";
import { readFileSync } from "fs";
const svg = readFileSync(new URL("./icon.svg", import.meta.url));
await sharp(svg, { density: 192 })
  .resize(1024, 1024)
  .png()
  .toFile(new URL("./icon-1024.png", import.meta.url).pathname);
console.log("rendered icon-1024.png");
