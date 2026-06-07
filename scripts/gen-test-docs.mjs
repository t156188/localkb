// 生成可被 parsers 解析的真实 .docx / .xlsx（手工拼 OOXML，再用系统 zip 打包）。
// docx: parsers 读 word/document.xml 剥标签；xlsx: calamine 读单元格（用 inlineStr）。
import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

const OUT = join(process.cwd(), "test-kb", "娱乐圈", "文档表格");
mkdirSync(OUT, { recursive: true });

const NS = {
  ct: "http://schemas.openxmlformats.org/package/2006/content-types",
  rel: "http://schemas.openxmlformats.org/package/2006/relationships",
  off: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
  wml: "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
  sml: "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
  rId: "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
};
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

// 把一组 {path: content} 写进临时目录并 zip 成目标文件
function pack(parts, outFile) {
  const stage = join(tmpdir(), "ooxml-" + outFile.replace(/[^\w]+/g, "_"));
  rmSync(stage, { recursive: true, force: true });
  for (const [p, content] of Object.entries(parts)) {
    const full = join(stage, p);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, content, "utf8");
  }
  const dest = join(OUT, outFile);
  rmSync(dest, { force: true });
  // -X 去掉额外属性；在 stage 内打包，路径相对
  execSync(`cd "${stage}" && zip -q -X -r "${dest}" "[Content_Types].xml" _rels word xl 2>/dev/null || true`, { shell: "/bin/zsh" });
  rmSync(stage, { recursive: true, force: true });
}

// ---------- DOCX ----------
function docx(outFile, paragraphs) {
  const body =
    paragraphs.map((t) => `<w:p><w:r><w:t xml:space="preserve">${esc(t)}</w:t></w:r></w:p>`).join("") +
    `<w:sectPr/>`;
  const parts = {
    "[Content_Types].xml":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Types xmlns="${NS.ct}">` +
      `<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
      `<Default Extension="xml" ContentType="application/xml"/>` +
      `<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>` +
      `</Types>`,
    "_rels/.rels":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Relationships xmlns="${NS.rel}">` +
      `<Relationship Id="rId1" Type="${NS.off}" Target="word/document.xml"/>` +
      `</Relationships>`,
    "word/document.xml":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<w:document xmlns:w="${NS.wml}"><w:body>${body}</w:body></w:document>`,
  };
  pack(parts, outFile);
}

// ---------- XLSX ----------
const colLetter = (n) => {
  let s = "";
  n++;
  while (n > 0) { const m = (n - 1) % 26; s = String.fromCharCode(65 + m) + s; n = Math.floor((n - 1) / 26); }
  return s;
};
function xlsx(outFile, rows) {
  const sheetRows = rows
    .map((row, ri) => {
      const cells = row
        .map((val, ci) => {
          const ref = colLetter(ci) + (ri + 1);
          if (typeof val === "number")
            return `<c r="${ref}"><v>${val}</v></c>`;
          return `<c r="${ref}" t="inlineStr"><is><t xml:space="preserve">${esc(val)}</t></is></c>`;
        })
        .join("");
      return `<row r="${ri + 1}">${cells}</row>`;
    })
    .join("");
  const parts = {
    "[Content_Types].xml":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Types xmlns="${NS.ct}">` +
      `<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
      `<Default Extension="xml" ContentType="application/xml"/>` +
      `<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>` +
      `<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>` +
      `</Types>`,
    "_rels/.rels":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Relationships xmlns="${NS.rel}">` +
      `<Relationship Id="rId1" Type="${NS.off}" Target="xl/workbook.xml"/>` +
      `</Relationships>`,
    "xl/workbook.xml":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<workbook xmlns="${NS.sml}" xmlns:r="${NS.rId}">` +
      `<sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>`,
    "xl/_rels/workbook.xml.rels":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<Relationships xmlns="${NS.rel}">` +
      `<Relationship Id="rId1" Type="${NS.rId}/worksheet" Target="worksheets/sheet1.xml"/>` +
      `</Relationships>`,
    "xl/worksheets/sheet1.xml":
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n` +
      `<worksheet xmlns="${NS.sml}"><sheetData>${sheetRows}</sheetData></worksheet>`,
  };
  pack(parts, outFile);
}

// ====== 内容（真实艺人 / 中性）======
docx("艺人通告单-2024.docx", [
  "艺人通告单（2024 年度）",
  "本通告单仅用于内部行程协调，记录公开活动安排，不含任何私人信息。",
  "一、影视拍摄",
  "胡歌：年代剧《山海长歌》第二阶段拍摄，外景地为横店影视城，预计为期六周。",
  "刘亦菲：都市情感剧《春风渡》定妆与读本会，随后进入棚内拍摄。",
  "二、品牌与活动",
  "靳东：出席品牌秋季发布会并担任分享嘉宾。",
  "赵丽颖：参与公益阅读推广活动，现场捐赠图书。",
  "三、备注",
  "所有通告以最终通知为准，如遇档期冲突由经纪团队统一协调。",
]);

docx("剧组拍摄计划-山海长歌.docx", [
  "《山海长歌》剧组拍摄计划",
  "类型：年代剧　　主演：胡歌、孙俪　　播出平台：央视综合频道",
  "项目概述：本剧讲述普通人在时代变迁中的成长与坚守，全剧共 40 集。",
  "拍摄周期：分三个阶段，覆盖横店、上海、青岛三地外景。",
  "第一阶段：人物建置与主线铺陈，重点完成主角群像戏份。",
  "第二阶段：情感线与冲突推进，安排重场戏与群演大场面。",
  "第三阶段：收尾与补拍，完成转场镜头与空镜素材。",
  "后期计划：剪辑、调色与配乐同步推进，力争年内完成送审。",
]);

docx("活动策划案-电影节红毯.docx", [
  "电影节红毯活动策划案",
  "活动名称：第 24 届白玉兰奖红毯暨颁奖盛典",
  "活动目标：呈现行业风采，致敬优秀创作者，营造积极正向的舆论氛围。",
  "流程安排：红毯入场、嘉宾采访、颁奖典礼、获奖感言、合影留念。",
  "拟邀嘉宾：胡歌、周迅、雷佳音、章子怡等影视音乐从业者。",
  "媒体安排：设置统一采访区，规范报道口径，倡导文明追星。",
  "应急预案：现场设医疗与安保点位，确保活动安全有序。",
]);

docx("经纪合作备忘.docx", [
  "经纪合作备忘录（示例）",
  "本备忘录为测试用示例文本，所涉条款均为通用表述，不构成任何真实协议。",
  "合作范围：影视拍摄、综艺通告、品牌代言、公益活动等公开演艺工作。",
  "工作原则：以作品为核心，遵守行业规范，维护良好公众形象。",
  "权益保障：明确档期协调、署名与酬劳结算等通用约定。",
  "保密条款：双方对未公开的工作信息负有保密义务。",
]);

// ====== XLSX ======
xlsx("艺人档案汇总表.xlsx", [
  ["姓名", "领域", "代表作", "代表奖项", "活跃起始年"],
  ["胡歌", "影视演员", "山海长歌", "白玉兰奖", 2005],
  ["刘亦菲", "影视演员", "春风渡", "金鸡奖", 2003],
  ["孙俪", "影视演员", "归途列车", "飞天奖", 2003],
  ["雷佳音", "影视演员", "破晓之城", "金鹰奖", 2007],
  ["周深", "音乐人", "星辰之间", "金曲奖提名", 2014],
  ["何炅", "主持人", "你好星期六", "金鹰奖最佳主持", 1998],
]);

xlsx("综艺收视统计表.xlsx", [
  ["节目名称", "播出平台", "首播年份", "集数", "平均热度指数"],
  ["披荆斩棘", "芒果TV", 2023, 12, 86],
  ["奔跑吧", "浙江卫视", 2024, 13, 82],
  ["声生不息", "湖南卫视", 2023, 11, 79],
  ["王牌对王牌", "浙江卫视", 2024, 12, 80],
  ["向往的生活", "湖南卫视", 2022, 12, 77],
]);

xlsx("专辑销量与上线表.xlsx", [
  ["专辑", "歌手", "风格", "发行年份", "数字销量(万)"],
  ["星辰之间", "周深", "流行", 2022, 120],
  ["回声", "毛不易", "民谣", 2021, 88],
  ["未完成", "李荣浩", "R&B", 2023, 95],
  ["昼夜", "邓紫棋", "电子流行", 2024, 110],
  ["远方来信", "张韶涵", "抒情", 2020, 70],
]);

xlsx("颁奖入围名单.xlsx", [
  ["奖项", "类别", "入围者", "入围作品", "届次"],
  ["白玉兰奖", "最佳男主角", "胡歌", "山海长歌", 24],
  ["白玉兰奖", "最佳女主角", "孙俪", "归途列车", 24],
  ["金鸡奖", "最佳男配角", "张译", "时代回声", 36],
  ["飞天奖", "优秀电视剧", "—", "春风渡", 34],
  ["金鹰奖", "观众喜爱的演员", "赵丽颖", "锦绣前程", 32],
]);

console.log("done");
