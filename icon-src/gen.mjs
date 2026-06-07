import sharp from "sharp";

const defs = `
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1024" y2="1024" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#7c6cff"/>
      <stop offset="0.55" stop-color="#4f46e5"/>
      <stop offset="1" stop-color="#3b1d8f"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.5" cy="0.30" r="0.78">
      <stop offset="0" stop-color="#ffffff" stop-opacity="0.20"/>
      <stop offset="1" stop-color="#ffffff" stop-opacity="0"/>
    </radialGradient>
    <filter id="sh" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="12" stdDeviation="16" flood-color="#170d38" flood-opacity="0.40"/>
    </filter>
  </defs>`;

const bg = `
  <rect width="1024" height="1024" rx="232" fill="url(#bg)"/>
  <rect width="1024" height="1024" rx="232" fill="url(#glow)"/>`;

// 1) Magnifier over a document (search knowledge)
const opt1 = `<svg width="1024" height="1024" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">${defs}${bg}
  <g filter="url(#sh)">
    <g stroke="#c9c4ff" stroke-width="26" stroke-linecap="round">
      <line x1="352" y1="372" x2="516" y2="372"/>
      <line x1="352" y1="424" x2="540" y2="424"/>
      <line x1="352" y1="476" x2="500" y2="476"/>
    </g>
    <circle cx="442" cy="424" r="196" fill="none" stroke="#ffffff" stroke-width="66"/>
    <line x1="585" y1="567" x2="742" y2="724" stroke="#ffffff" stroke-width="76" stroke-linecap="round"/>
  </g>
</svg>`;

// 2) Knowledge graph (connected nodes)
const opt2 = `<svg width="1024" height="1024" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">${defs}${bg}
  <g filter="url(#sh)">
    <g stroke="#ffffff" stroke-width="20" stroke-opacity="0.8" stroke-linecap="round">
      <line x1="512" y1="500" x2="358" y2="350"/>
      <line x1="512" y1="500" x2="676" y2="332"/>
      <line x1="512" y1="500" x2="700" y2="612"/>
      <line x1="512" y1="500" x2="372" y2="664"/>
      <line x1="358" y1="350" x2="676" y2="332"/>
    </g>
    <circle cx="358" cy="350" r="46" fill="#ffffff"/>
    <circle cx="676" cy="332" r="38" fill="#ffffff"/>
    <circle cx="700" cy="612" r="42" fill="#ffffff"/>
    <circle cx="372" cy="664" r="38" fill="#ffffff"/>
    <circle cx="512" cy="500" r="70" fill="#ffffff"/>
    <circle cx="512" cy="500" r="34" fill="#4f46e5"/>
  </g>
</svg>`;

// 3) Magnifier with a spark inside (AI search)
const spark = (cx, cy, ro, wi) =>
  `M ${cx},${cy-ro} Q ${cx+wi},${cy-wi} ${cx+ro},${cy} Q ${cx+wi},${cy+wi} ${cx},${cy+ro} Q ${cx-wi},${cy+wi} ${cx-ro},${cy} Q ${cx-wi},${cy-wi} ${cx},${cy-ro} Z`;
const opt3 = `<svg width="1024" height="1024" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">${defs}${bg}
  <g filter="url(#sh)">
    <circle cx="446" cy="420" r="194" fill="none" stroke="#ffffff" stroke-width="66"/>
    <line x1="588" y1="562" x2="744" y2="718" stroke="#ffffff" stroke-width="76" stroke-linecap="round"/>
    <path d="${spark(446,420,128,30)}" fill="#ffffff"/>
    <path d="${spark(556,322,46,11)}" fill="#d7d2ff"/>
  </g>
</svg>`;

for (const [name, svg] of [["opt1", opt1], ["opt2", opt2], ["opt3", opt3]]) {
  await sharp(Buffer.from(svg), { density: 192 }).resize(1024, 1024).png()
    .toFile(`icon-src/${name}.png`);
}
console.log("rendered opt1/opt2/opt3");
