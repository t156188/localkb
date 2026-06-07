// 第三轮扩容：偏日常办公场景的 Word/Excel/PDF/PPT 文件。
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

const ROOT = join(process.cwd(), "test-kb", "娱乐圈", "办公文档增补");
mkdirSync(ROOT, { recursive: true });

const people = ["胡歌", "刘亦菲", "孙俪", "赵丽颖", "周迅", "黄渤", "沈腾", "马丽", "张译", "雷佳音", "易烊千玺", "刘昊然", "王一博", "肖战", "李现", "倪妮", "汤唯", "周深", "毛不易", "李荣浩", "邓紫棋", "张韶涵", "何炅", "撒贝宁"];
const projects = ["《城市光谱》", "《风起海岸》", "《星尘剧场》", "《长街烟火》", "《山河入梦》", "《冬日回响》", "《月光片场》", "《青云计划》", "《南方来信》", "《热浪之外》"];
const offices = ["会议纪要", "项目周报", "宣发日报", "合同备忘", "活动方案", "审批单", "预算说明", "排期计划", "报销清单", "舆情简报", "物料归档", "招商记录", "通告安排", "媒体邀约", "复盘报告"];
const platforms = ["央视综合频道", "东方卫视", "湖南卫视", "浙江卫视", "腾讯视频", "爱奇艺", "优酷", "芒果TV", "微博", "抖音"];

const pick = (arr, i) => arr[i % arr.length];
const pad = (n, w = 3) => String(n).padStart(w, "0");
const xml = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const pdfEsc = (s) => String(s).replace(/[\\()]/g, "\\$&").replace(/[^\x20-\x7e]/g, "?");

function doc(i) {
  const type = pick(offices, i);
  const project = pick(projects, i * 2);
  const a = pick(people, i * 3);
  const b = pick(people, i * 5 + 1);
  const platform = pick(platforms, i * 7);
  const code = `OFF-${pad(i + 1)}`;
  const year = 2022 + (i % 5);
  return {
    code, type, project, a, b, platform, year,
    title: `${project}${type}-${code}`,
    lines: [
      `文档编号：${code}`,
      `文档类型：${type}`,
      `关联项目：${project}`,
      `相关人员：${a}、${b}`,
      `涉及平台：${platform}`,
      `办公场景：用于娱乐项目日常协作、进度同步、预算登记和资料归档。`,
      `工作要点：确认物料口径、更新时间表、记录责任人、保留审批意见。`,
      `风险提示：宣传节奏、活动场地、艺人档期、预算上限和媒体排期需要提前核对。`,
      `复盘结论：本文件为本地知识库测试语料，内容中性原创，不包含私人信息。`,
      `标签：娱乐圈 办公文档 ${type} ${project.replace(/[《》]/g, "")} ${platform}`,
    ],
  };
}

function ensure(sub) {
  const dir = join(ROOT, sub);
  mkdirSync(dir, { recursive: true });
  return dir;
}

function pack(parts, outPath) {
  const stage = join(tmpdir(), "ent-office-" + outPath.replace(/[^\w]+/g, "_"));
  rmSync(stage, { recursive: true, force: true });
  for (const [p, text] of Object.entries(parts)) {
    const full = join(stage, p);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, text, "utf8");
  }
  rmSync(outPath, { force: true });
  execSync(`cd "${stage}" && zip -q -X -r "${outPath}" .`, { shell: "/bin/zsh" });
  rmSync(stage, { recursive: true, force: true });
}

function makeDocx(file, docs) {
  const body = docs.flatMap((d) => [d.title, ...d.lines]).map((t) => `<w:p><w:r><w:t xml:space="preserve">${xml(t)}</w:t></w:r></w:p>`).join("");
  pack({
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>`,
    "word/document.xml": `<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>${body}<w:sectPr/></w:body></w:document>`,
  }, join(ensure("docx-Word文档"), file));
}

const col = (n) => {
  let s = "";
  for (n++; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s;
  return s;
};

function makeXlsx(file, docs) {
  const rows = [["编号", "类型", "项目", "人员", "平台", "年份", "状态", "预算"], ...docs.map((d, i) => [d.code, d.type, d.project, `${d.a}/${d.b}`, d.platform, d.year, i % 3 === 0 ? "待确认" : "已归档", 30000 + i * 2500])];
  const sheet = rows.map((row, r) => `<row r="${r + 1}">${row.map((v, c) => typeof v === "number" ? `<c r="${col(c)}${r + 1}"><v>${v}</v></c>` : `<c r="${col(c)}${r + 1}" t="inlineStr"><is><t>${xml(v)}</t></is></c>`).join("")}</row>`).join("");
  pack({
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
    "xl/workbook.xml": `<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="办公表格" sheetId="1" r:id="rId1"/></sheets></workbook>`,
    "xl/_rels/workbook.xml.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>`,
    "xl/worksheets/sheet1.xml": `<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>${sheet}</sheetData></worksheet>`,
  }, join(ensure("xlsx-Excel表格"), file));
}

function makePptx(file, docs) {
  const parts = {
    "[Content_Types].xml": `<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>`,
    "_rels/.rels": `<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>`,
    "ppt/presentation.xml": `<?xml version="1.0"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
  };
  docs.forEach((d, i) => {
    parts[`ppt/slides/slide${i + 1}.xml`] = `<?xml version="1.0"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>${xml(d.title)}</a:t></a:r></a:p><a:p><a:r><a:t>${xml(d.lines[5])}</a:t></a:r></a:p><a:p><a:r><a:t>${xml(d.lines[6])}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>`;
  });
  pack(parts, join(ensure("pptx-汇报简报"), file));
}

function makePdf(file, docs) {
  const lines = docs.flatMap((d) => [d.title, ...d.lines.slice(0, 8)]).slice(0, 34);
  const stream = ["BT", "/F1 11 Tf", "50 790 Td", ...lines.flatMap((line) => [`(${pdfEsc(line)}) Tj`, "0 -21 Td"]), "ET"].join("\n");
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
  writeFileSync(join(ensure("pdf-报告归档"), file), out, "binary");
}

function makeLegacyDoc(file, d) {
  const html = `<!doctype html><html><head><meta charset="utf-8"><title>${d.title}</title></head><body><h1>${d.title}</h1>${d.lines.map((x) => `<p>${xml(x)}</p>`).join("")}</body></html>`;
  writeFileSync(join(ensure("doc-兼容Word老格式"), file), html, "utf8");
}

function makeLegacyXls(file, docs) {
  const rows = [["编号", "类型", "项目", "人员", "平台"], ...docs.map((d) => [d.code, d.type, d.project, `${d.a}/${d.b}`, d.platform])];
  const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><table>${rows.map((r) => `<tr>${r.map((c) => `<td>${xml(c)}</td>`).join("")}</tr>`).join("")}</table></body></html>`;
  writeFileSync(join(ensure("xls-兼容Excel老格式"), file), html, "utf8");
}

for (let i = 0; i < 80; i++) makeDocx(`办公文档-${pad(i + 1)}.docx`, [doc(i), doc(i + 80)]);
for (let i = 0; i < 80; i++) makeXlsx(`办公表格-${pad(i + 1)}.xlsx`, Array.from({ length: 8 }, (_, j) => doc(i * 4 + j)));
for (let i = 0; i < 60; i++) makePdf(`办公报告-${pad(i + 1)}.pdf`, [doc(i), doc(i + 60), doc(i + 120)]);
for (let i = 0; i < 40; i++) makePptx(`办公简报-${pad(i + 1)}.pptx`, [doc(i), doc(i + 40), doc(i + 80)]);
for (let i = 0; i < 60; i++) makeLegacyDoc(`兼容Word-${pad(i + 1)}.doc`, doc(i));
for (let i = 0; i < 60; i++) makeLegacyXls(`兼容Excel-${pad(i + 1)}.xls`, Array.from({ length: 10 }, (_, j) => doc(i * 3 + j)));

console.log("generated 380 office-style documents in test-kb/娱乐圈/办公文档增补");
