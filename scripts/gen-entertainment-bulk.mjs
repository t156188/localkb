// 批量扩充娱乐圈测试知识库：多主题、多格式、原创中性测试语料。
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

const ROOT = join(process.cwd(), "test-kb", "娱乐圈");
const OUT = "增补语料";
const base = join(ROOT, OUT);
mkdirSync(base, { recursive: true });

const topics = [
  "院线电影", "网络电影", "年代剧", "都市剧", "悬疑剧", "古装剧", "综艺节目", "音乐现场",
  "唱片发行", "艺人经纪", "商务代言", "公益活动", "电影节", "红毯造型", "平台排播", "数据运营",
  "短剧赛道", "剧本开发", "项目招商", "宣发节奏", "后期制作", "服化道设计", "选角流程", "粉丝社群",
];
const people = [
  "胡歌", "刘亦菲", "孙俪", "赵丽颖", "周迅", "黄渤", "沈腾", "马丽", "张译", "雷佳音",
  "易烊千玺", "刘昊然", "王一博", "肖战", "李现", "倪妮", "汤唯", "周深", "毛不易", "李荣浩",
  "邓紫棋", "张韶涵", "何炅", "撒贝宁",
];
const works = [
  "《山海回声》", "《春日来信》", "《长夜灯火》", "《城市坐标》", "《锦绣新章》", "《逆风航线》",
  "《烟火人间》", "《星河入梦》", "《旧梦新声》", "《白昼剧场》", "《云上乐章》", "《盛夏未完》",
];
const platforms = ["央视综合频道", "东方卫视", "湖南卫视", "浙江卫视", "腾讯视频", "爱奇艺", "优酷", "芒果TV", "B站"];
const formats = ["md", "txt", "json", "yaml", "yml", "toml", "csv", "tsv", "html", "xml", "rst", "org", "log", "ini", "sql", "js"];

const escXml = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const escPdf = (s) => String(s).replace(/[\\()]/g, "\\$&");
const pad = (n) => String(n).padStart(3, "0");
const pick = (arr, i) => arr[i % arr.length];
const write = (sub, name, text) => {
  const dir = join(base, sub);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, name), text, "utf8");
};

function paragraphs(i) {
  const topic = pick(topics, i);
  const person = pick(people, i * 3 + 1);
  const person2 = pick(people, i * 5 + 4);
  const work = pick(works, i);
  const platform = pick(platforms, i * 2);
  const year = 2020 + (i % 6);
  return {
    id: `ENT-${pad(i + 1)}`,
    title: `${topic}观察 ${pad(i + 1)}`,
    topic,
    person,
    person2,
    work,
    platform,
    year,
    summary: `${work}围绕${topic}展开，记录项目从筹备、制作到上线传播的关键节点。`,
    body: [
      `${topic}是娱乐产业中的常见板块，涉及内容生产、平台分发、宣发协同与观众反馈。`,
      `${person}与${person2}在公开活动中多次提到作品表达、职业训练和团队协作的重要性。`,
      `${work}计划于${year}年前后进入主要宣传周期，物料会覆盖预告、海报、访谈和线下活动。`,
      `${platform}侧重以热度指数、会员转化、互动评论和长尾播放评估项目表现。`,
      `该文档为本地知识库测试语料，内容采用中性概括写法，不涉及未经证实的私人信息。`,
    ],
    tags: [topic, person, person2, work.replace(/[《》]/g, ""), platform, "娱乐圈", "测试语料"],
  };
}

function renderText(doc) {
  return `${doc.title}

编号：${doc.id}
主题：${doc.topic}
人物：${doc.person}、${doc.person2}
作品：${doc.work}
平台：${doc.platform}
年份：${doc.year}

摘要：${doc.summary}

${doc.body.map((p, idx) => `${idx + 1}. ${p}`).join("\n")}

标签：${doc.tags.join(" ")}
`;
}

function render(ext, doc) {
  const plain = renderText(doc);
  switch (ext) {
    case "md":
      return `# ${doc.title}\n\n- 编号：${doc.id}\n- 主题：${doc.topic}\n- 相关人物：${doc.person}、${doc.person2}\n- 作品：${doc.work}\n- 平台：${doc.platform}\n\n## 摘要\n\n${doc.summary}\n\n## 要点\n\n${doc.body.map((p) => `- ${p}`).join("\n")}\n\n标签: ${doc.tags.join(" ")}\n`;
    case "json":
      return JSON.stringify(doc, null, 2) + "\n";
    case "yaml":
    case "yml":
      return `id: ${doc.id}\ntitle: ${doc.title}\ntopic: ${doc.topic}\npeople:\n  - ${doc.person}\n  - ${doc.person2}\nwork: ${doc.work}\nplatform: ${doc.platform}\nyear: ${doc.year}\nsummary: ${doc.summary}\ntags: ${doc.tags.join(", ")}\n`;
    case "toml":
      return `id = "${doc.id}"\ntitle = "${doc.title}"\ntopic = "${doc.topic}"\npeople = ["${doc.person}", "${doc.person2}"]\nwork = "${doc.work}"\nplatform = "${doc.platform}"\nyear = ${doc.year}\nsummary = "${doc.summary}"\ntags = ["${doc.tags.join('", "')}"]\n`;
    case "csv":
      return `编号,标题,主题,人物,作品,平台,年份,摘要\n${doc.id},${doc.title},${doc.topic},${doc.person}/${doc.person2},${doc.work},${doc.platform},${doc.year},${doc.summary}\n`;
    case "tsv":
      return `编号\t标题\t主题\t人物\t作品\t平台\t年份\t摘要\n${doc.id}\t${doc.title}\t${doc.topic}\t${doc.person}/${doc.person2}\t${doc.work}\t${doc.platform}\t${doc.year}\t${doc.summary}\n`;
    case "html":
      return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><title>${doc.title}</title></head><body><h1>${doc.title}</h1><p>${doc.summary}</p><ul>${doc.body.map((p) => `<li>${p}</li>`).join("")}</ul><p>标签：${doc.tags.join(" ")}</p></body></html>\n`;
    case "xml":
      return `<?xml version="1.0" encoding="UTF-8"?><document><id>${doc.id}</id><title>${escXml(doc.title)}</title><topic>${escXml(doc.topic)}</topic><people><person>${escXml(doc.person)}</person><person>${escXml(doc.person2)}</person></people><work>${escXml(doc.work)}</work><platform>${escXml(doc.platform)}</platform><summary>${escXml(doc.summary)}</summary></document>\n`;
    case "rst":
      return `${doc.title}\n${"=".repeat(doc.title.length)}\n\n${plain}`;
    case "org":
      return `* ${doc.title}\n\n${plain}`;
    case "log":
      return `[${doc.year}-06-01 10:00] ${doc.id} ${doc.topic} ${doc.summary}\n[${doc.year}-06-01 10:30] ${doc.person} ${doc.person2} ${doc.platform} 宣发节点记录\n`;
    case "ini":
      return `[entertainment]\nid=${doc.id}\ntitle=${doc.title}\ntopic=${doc.topic}\npeople=${doc.person},${doc.person2}\nwork=${doc.work}\nplatform=${doc.platform}\nyear=${doc.year}\nsummary=${doc.summary}\n`;
    case "sql":
      return `insert into entertainment_docs(id,title,topic,people,work,platform,year,summary) values ('${doc.id}','${doc.title}','${doc.topic}','${doc.person}/${doc.person2}','${doc.work}','${doc.platform}',${doc.year},'${doc.summary}');\n`;
    case "js":
      return `export default ${JSON.stringify(doc, null, 2)};\n`;
    default:
      return plain;
  }
}

// 336 个文本类文件：覆盖所有受支持的常见文本/代码格式。
for (let i = 0; i < 336; i++) {
  const ext = formats[i % formats.length];
  const doc = paragraphs(i);
  write(`文本合集/${ext}`, `${doc.id}-${doc.topic}.${ext}`, render(ext, doc));
}

function pack(parts, outFile) {
  const stage = join(tmpdir(), "ent-bulk-" + outFile.replace(/[^\w]+/g, "_"));
  rmSync(stage, { recursive: true, force: true });
  for (const [p, content] of Object.entries(parts)) {
    const full = join(stage, p);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, content, "utf8");
  }
  const dest = join(base, "Office与PDF", outFile);
  mkdirSync(join(base, "Office与PDF"), { recursive: true });
  rmSync(dest, { force: true });
  execSync(`cd "${stage}" && zip -q -X -r "${dest}" .`, { shell: "/bin/zsh" });
  rmSync(stage, { recursive: true, force: true });
}

function docx(outFile, docs) {
  const body = docs.flatMap((d) => [d.title, ...d.body, `标签：${d.tags.join(" ")}`])
    .map((t) => `<w:p><w:r><w:t xml:space="preserve">${escXml(t)}</w:t></w:r></w:p>`).join("");
  pack({
    "[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>`,
    "word/document.xml": `<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>${body}<w:sectPr/></w:body></w:document>`,
  }, outFile);
}

const col = (n) => {
  let s = "";
  for (n++; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s;
  return s;
};
function xlsx(outFile, docs) {
  const rows = [["编号", "标题", "主题", "人物", "作品", "平台", "年份"], ...docs.map((d) => [d.id, d.title, d.topic, `${d.person}/${d.person2}`, d.work, d.platform, d.year])];
  const sheet = rows.map((row, r) => `<row r="${r + 1}">${row.map((v, c) => typeof v === "number" ? `<c r="${col(c)}${r + 1}"><v>${v}</v></c>` : `<c r="${col(c)}${r + 1}" t="inlineStr"><is><t>${escXml(v)}</t></is></c>`).join("")}</row>`).join("");
  pack({
    "[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
    "xl/workbook.xml": `<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="娱乐语料" sheetId="1" r:id="rId1"/></sheets></workbook>`,
    "xl/_rels/workbook.xml.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>`,
    "xl/worksheets/sheet1.xml": `<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>${sheet}</sheetData></worksheet>`,
  }, outFile);
}

function pptx(outFile, docs) {
  const parts = {
    "[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>`,
    "ppt/presentation.xml": `<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
  };
  docs.forEach((d, i) => {
    parts[`ppt/slides/slide${i + 1}.xml`] = `<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>${escXml(d.title)}</a:t></a:r></a:p><a:p><a:r><a:t>${escXml(d.summary)}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>`;
  });
  pack(parts, outFile);
}

function pdf(outFile, docs) {
  const lines = docs.flatMap((d) => [d.title, d.summary, `${d.topic} ${d.person} ${d.platform}`]).slice(0, 24);
  const stream = ["BT", "/F1 12 Tf", "50 760 Td", ...lines.flatMap((line) => [`(${escPdf(line)}) Tj`, "0 -24 Td"]), "ET"].join("\n");
  const objects = [
    "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n",
    "2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n",
    "3 0 obj << /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /MediaBox [0 0 595 842] /Contents 5 0 R >> endobj\n",
    "4 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n",
    `5 0 obj << /Length ${Buffer.byteLength(stream)} >> stream\n${stream}\nendstream endobj\n`,
  ];
  let pdfText = "%PDF-1.4\n";
  const offsets = [0];
  for (const obj of objects) {
    offsets.push(Buffer.byteLength(pdfText));
    pdfText += obj;
  }
  const xref = Buffer.byteLength(pdfText);
  pdfText += `xref\n0 6\n0000000000 65535 f \n${offsets.slice(1).map((o) => String(o).padStart(10, "0") + " 00000 n ").join("\n")}\ntrailer << /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  writeFileSync(join(base, "Office与PDF", outFile), pdfText, "binary");
}

for (let i = 0; i < 8; i++) docx(`娱乐项目备忘-${i + 1}.docx`, [paragraphs(i), paragraphs(i + 24), paragraphs(i + 48)]);
for (let i = 0; i < 8; i++) xlsx(`娱乐数据表-${i + 1}.xlsx`, Array.from({ length: 8 }, (_, j) => paragraphs(i * 8 + j)));
for (let i = 0; i < 8; i++) pptx(`宣发简报-${i + 1}.pptx`, [paragraphs(i), paragraphs(i + 8), paragraphs(i + 16)]);
for (let i = 0; i < 8; i++) pdf(`行业报告-${i + 1}.pdf`, [paragraphs(i), paragraphs(i + 12), paragraphs(i + 36)]);

console.log("generated 368 entertainment documents in test-kb/娱乐圈/增补语料");
