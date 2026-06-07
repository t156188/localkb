// 第二轮娱乐圈知识库扩容：更大规模、更多文件后缀、更多业务场景。
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

const ROOT = join(process.cwd(), "test-kb", "娱乐圈", "深度增补语料");
mkdirSync(ROOT, { recursive: true });

const people = ["胡歌", "刘亦菲", "孙俪", "赵丽颖", "周迅", "黄渤", "沈腾", "马丽", "张译", "雷佳音", "易烊千玺", "刘昊然", "王一博", "肖战", "李现", "倪妮", "汤唯", "周深", "毛不易", "李荣浩", "邓紫棋", "张韶涵", "何炅", "撒贝宁", "王凯", "杨幂", "陈坤", "章子怡", "贾玲", "檀健次"];
const projects = ["《城市光谱》", "《风起海岸》", "《星尘剧场》", "《长街烟火》", "《山河入梦》", "《冬日回响》", "《月光片场》", "《青云计划》", "《南方来信》", "《热浪之外》", "《剧场人生》", "《平行舞台》"];
const scenes = ["剧本围读", "定妆拍摄", "外景统筹", "宣发复盘", "舆情监测", "商务报价", "品牌联动", "粉丝运营", "红毯执行", "片单发布", "音乐企划", "综艺录制", "短剧投放", "平台排播", "数据周报", "法务审核", "物料归档", "奖项申报", "票房日报", "口碑分析"];
const platforms = ["央视综合频道", "东方卫视", "湖南卫视", "浙江卫视", "腾讯视频", "爱奇艺", "优酷", "芒果TV", "B站", "微博", "抖音", "小红书"];
const textExts = [
  "md", "markdown", "mdown", "mkd", "txt", "rst", "org", "tex", "log",
  "csv", "tsv", "json", "json5", "yaml", "yml", "toml", "ini", "cfg", "conf", "env", "properties",
  "xml", "html", "htm", "css", "scss", "less", "sql", "graphql",
  "rs", "py", "js", "jsx", "ts", "tsx", "mjs", "cjs", "vue", "svelte",
  "go", "java", "kt", "scala", "c", "h", "cpp", "cs", "rb", "php", "swift", "lua", "r", "dart",
  "sh", "bash", "zsh", "ps1", "bat", "cmd", "make", "cmake", "gradle", "proto", "tf", "hcl"
];

const pick = (arr, i) => arr[i % arr.length];
const pad = (n, w = 4) => String(n).padStart(w, "0");
const xml = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const pdfEsc = (s) => String(s).replace(/[\\()]/g, "\\$&").replace(/[^\x20-\x7e]/g, "?");
const q = (s) => String(s).replace(/\\/g, "\\\\").replace(/"/g, '\\"');

function mkdirWrite(sub, file, text, enc = "utf8") {
  const dir = join(ROOT, sub);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, file), text, enc);
}

function item(i) {
  const scene = pick(scenes, i);
  const project = pick(projects, i * 2);
  const a = pick(people, i * 3);
  const b = pick(people, i * 5 + 2);
  const platform = pick(platforms, i * 7);
  const year = 2021 + (i % 6);
  const code = `DEEP-${pad(i + 1)}`;
  return {
    code, scene, project, a, b, platform, year,
    title: `${scene}资料-${code}`,
    summary: `${project}围绕${scene}形成内部知识条目，记录公开可讨论的流程、指标、口径和复盘结论。`,
    bullets: [
      `${scene}通常需要主创、平台、经纪团队与宣发团队共同确认时间表。`,
      `${a}、${b}等公开人物只作为测试语料中的娱乐行业角色名出现。`,
      `${platform}侧重根据播放、评论、转发、会员转化或到场反馈判断传播效果。`,
      `物料包括海报、预告、花絮、采访、路演纪要、社媒文案和数据报表。`,
      `该文件用于本地知识库索引测试，内容保持中性、概括和原创。`,
    ],
    tags: [scene, project.replace(/[《》]/g, ""), a, b, platform, "娱乐行业", "知识库"],
  };
}

function plain(d) {
  return `${d.title}
编号：${d.code}
场景：${d.scene}
项目：${d.project}
人物：${d.a}、${d.b}
平台：${d.platform}
年份：${d.year}
摘要：${d.summary}

${d.bullets.map((x, i) => `${i + 1}. ${x}`).join("\n")}

标签：${d.tags.join(" ")}
`;
}

function render(ext, d) {
  if (["md", "markdown", "mdown", "mkd"].includes(ext)) return `# ${d.title}\n\n- 编号：${d.code}\n- 场景：${d.scene}\n- 项目：${d.project}\n- 人物：${d.a}、${d.b}\n- 平台：${d.platform}\n\n## 摘要\n\n${d.summary}\n\n## 记录\n\n${d.bullets.map((x) => `- ${x}`).join("\n")}\n\n标签: ${d.tags.join(" ")}\n`;
  if (ext === "json" || ext === "json5") return JSON.stringify(d, null, 2) + "\n";
  if (ext === "yaml" || ext === "yml") return `code: ${d.code}\ntitle: ${d.title}\nscene: ${d.scene}\nproject: ${d.project}\npeople:\n  - ${d.a}\n  - ${d.b}\nplatform: ${d.platform}\nyear: ${d.year}\nsummary: ${d.summary}\ntags: ${d.tags.join(", ")}\n`;
  if (ext === "toml") return `code = "${d.code}"\ntitle = "${d.title}"\nscene = "${d.scene}"\nproject = "${d.project}"\npeople = ["${d.a}", "${d.b}"]\nplatform = "${d.platform}"\nyear = ${d.year}\nsummary = "${d.summary}"\n`;
  if (ext === "csv") return `编号,标题,场景,项目,人物,平台,年份,摘要\n${d.code},${d.title},${d.scene},${d.project},${d.a}/${d.b},${d.platform},${d.year},${d.summary}\n`;
  if (ext === "tsv") return `编号\t标题\t场景\t项目\t人物\t平台\t年份\t摘要\n${d.code}\t${d.title}\t${d.scene}\t${d.project}\t${d.a}/${d.b}\t${d.platform}\t${d.year}\t${d.summary}\n`;
  if (ext === "xml") return `<?xml version="1.0"?><entry><code>${d.code}</code><title>${xml(d.title)}</title><scene>${xml(d.scene)}</scene><project>${xml(d.project)}</project><summary>${xml(d.summary)}</summary></entry>\n`;
  if (ext === "html" || ext === "htm") return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><title>${d.title}</title></head><body><article><h1>${d.title}</h1><p>${d.summary}</p>${d.bullets.map((x) => `<p>${x}</p>`).join("")}<footer>${d.tags.join(" ")}</footer></article></body></html>\n`;
  if (ext === "tex") return `\\section{${d.title}}\n${plain(d)}\n`;
  if (ext === "rst") return `${d.title}\n${"=".repeat(d.title.length)}\n\n${plain(d)}`;
  if (ext === "org") return `* ${d.title}\n${plain(d)}`;
  if (ext === "ini" || ext === "cfg" || ext === "conf" || ext === "properties") return `[record]\ncode=${d.code}\ntitle=${d.title}\nscene=${d.scene}\nproject=${d.project}\npeople=${d.a},${d.b}\nplatform=${d.platform}\nsummary=${d.summary}\n`;
  if (ext === "env") return `ENT_CODE=${d.code}\nENT_SCENE="${d.scene}"\nENT_PROJECT="${d.project}"\nENT_PLATFORM="${d.platform}"\nENT_SUMMARY="${d.summary}"\n`;
  if (ext === "sql") return `insert into entertainment_knowledge(code, scene, project, people, platform, summary) values ('${d.code}', '${d.scene}', '${d.project}', '${d.a}/${d.b}', '${d.platform}', '${d.summary}');\n`;
  if (ext === "graphql") return `type EntertainmentRecord { code: String scene: String project: String people: [String] platform: String summary: String }\n# ${plain(d)}\n`;
  if (["js", "jsx", "ts", "tsx", "mjs", "cjs"].includes(ext)) return `export const entertainmentRecord = ${JSON.stringify(d, null, 2)};\n`;
  if (ext === "py") return `record = ${JSON.stringify(d, null, 2)}\n`;
  if (["rs", "go", "java", "kt", "scala", "c", "h", "cpp", "cs", "rb", "php", "swift", "lua", "r", "dart"].includes(ext)) return `// ${plain(d).replace(/\n/g, "\n// ")}\n`;
  if (["sh", "bash", "zsh", "ps1", "bat", "cmd", "make", "cmake", "gradle", "tf", "hcl", "proto"].includes(ext)) return `# ${plain(d).replace(/\n/g, "\n# ")}\n`;
  if (["css", "scss", "less"].includes(ext)) return `/* ${plain(d).replace(/\*\//g, "* /")} */\n.ent-${d.code.toLowerCase()} { content: "${q(d.scene)}"; }\n`;
  if (ext === "vue") return `<template><section><h1>${d.title}</h1><p>${d.summary}</p></section></template>\n<script setup>\nconst record = ${JSON.stringify(d, null, 2)}\n</script>\n`;
  if (ext === "svelte") return `<script>export let record = ${JSON.stringify(d)};</script>\n<h1>${d.title}</h1><p>${d.summary}</p>\n`;
  if (ext === "log") return `[${d.year}-09-01 09:30] ${d.code} ${d.scene} ${d.summary}\n[${d.year}-09-01 18:00] ${d.platform} ${d.project} 复盘完成\n`;
  return plain(d);
}

for (let i = 0; i < 700; i++) {
  const ext = pick(textExts, i);
  const d = item(i);
  const group = `文本深水区/${ext}`;
  mkdirWrite(group, `${d.code}-${d.scene}.${ext}`, render(ext, d));
}

function pack(parts, outFile) {
  const stage = join(tmpdir(), "ent-more-" + outFile.replace(/[^\w]+/g, "_"));
  rmSync(stage, { recursive: true, force: true });
  for (const [p, text] of Object.entries(parts)) {
    const full = join(stage, p);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, text, "utf8");
  }
  const destDir = join(ROOT, "复杂文档包");
  mkdirSync(destDir, { recursive: true });
  const dest = join(destDir, outFile);
  rmSync(dest, { force: true });
  execSync(`cd "${stage}" && zip -q -X -r "${dest}" .`, { shell: "/bin/zsh" });
  rmSync(stage, { recursive: true, force: true });
}

function docx(file, docs) {
  const ps = docs.flatMap((d) => [d.title, d.summary, ...d.bullets, `标签：${d.tags.join(" ")}`]).map((t) => `<w:p><w:r><w:t xml:space="preserve">${xml(t)}</w:t></w:r></w:p>`).join("");
  pack({
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>`,
    "word/document.xml": `<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>${ps}<w:sectPr/></w:body></w:document>`,
  }, file);
}

const col = (n) => {
  let s = "";
  for (n++; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s;
  return s;
};
function xlsx(file, docs) {
  const rows = [["编号", "场景", "项目", "人物", "平台", "年份", "摘要"], ...docs.map((d) => [d.code, d.scene, d.project, `${d.a}/${d.b}`, d.platform, d.year, d.summary])];
  const sheet = rows.map((row, r) => `<row r="${r + 1}">${row.map((v, c) => typeof v === "number" ? `<c r="${col(c)}${r + 1}"><v>${v}</v></c>` : `<c r="${col(c)}${r + 1}" t="inlineStr"><is><t>${xml(v)}</t></is></c>`).join("")}</row>`).join("");
  pack({
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
    "xl/workbook.xml": `<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="深度语料" sheetId="1" r:id="rId1"/></sheets></workbook>`,
    "xl/_rels/workbook.xml.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>`,
    "xl/worksheets/sheet1.xml": `<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>${sheet}</sheetData></worksheet>`,
  }, file);
}

function pptx(file, docs) {
  const parts = {
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>`,
    "ppt/presentation.xml": `<?xml version="1.0"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
  };
  docs.forEach((d, i) => {
    parts[`ppt/slides/slide${i + 1}.xml`] = `<?xml version="1.0"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>${xml(d.title)}</a:t></a:r></a:p><a:p><a:r><a:t>${xml(d.summary)}</a:t></a:r></a:p><a:p><a:r><a:t>${xml(d.tags.join(" "))}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>`;
  });
  pack(parts, file);
}

function pdf(file, docs) {
  const dir = join(ROOT, "复杂文档包");
  mkdirSync(dir, { recursive: true });
  const lines = docs.flatMap((d) => [d.title, d.summary, `${d.scene} ${d.platform}`]).slice(0, 28);
  const stream = ["BT", "/F1 11 Tf", "50 780 Td", ...lines.flatMap((line) => [`(${pdfEsc(line)}) Tj`, "0 -22 Td"]), "ET"].join("\n");
  const objs = [
    "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n",
    "2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n",
    "3 0 obj << /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /MediaBox [0 0 595 842] /Contents 5 0 R >> endobj\n",
    "4 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n",
    `5 0 obj << /Length ${Buffer.byteLength(stream)} >> stream\n${stream}\nendstream endobj\n`,
  ];
  let out = "%PDF-1.4\n";
  const offsets = [0];
  for (const obj of objs) { offsets.push(Buffer.byteLength(out)); out += obj; }
  const xref = Buffer.byteLength(out);
  out += `xref\n0 6\n0000000000 65535 f \n${offsets.slice(1).map((o) => String(o).padStart(10, "0") + " 00000 n ").join("\n")}\ntrailer << /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  writeFileSync(join(dir, file), out, "binary");
}

for (let i = 0; i < 20; i++) docx(`深度纪要-${pad(i + 1, 2)}.docx`, [item(i), item(i + 60), item(i + 120)]);
for (let i = 0; i < 20; i++) xlsx(`深度数据-${pad(i + 1, 2)}.xlsx`, Array.from({ length: 12 }, (_, j) => item(i * 12 + j)));
for (let i = 0; i < 20; i++) pptx(`深度简报-${pad(i + 1, 2)}.pptx`, [item(i), item(i + 20), item(i + 40), item(i + 80)]);
for (let i = 0; i < 20; i++) pdf(`深度报告-${pad(i + 1, 2)}.pdf`, [item(i), item(i + 40), item(i + 100), item(i + 160)]);

console.log("generated 780 more entertainment documents in test-kb/娱乐圈/深度增补语料");
