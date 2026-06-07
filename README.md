# LOCALKB · 本地知识检索

把本地文件夹索引成可检索的知识库，用聊天的方式提问，回答带可点击的引用文件。完全本地索引，问答走可配置的远程模型。Tauri 2 跨平台（macOS / Windows），用户安装包零依赖。

## 功能

- **混合检索**：SQLite FTS5 关键词（中文 trigram 分词）+ 本地向量语义检索（fastembed / multilingual-e5-small），RRF 融合。
- **引用来源**：回答下方列出引用的文件，点击用系统默认程序打开，或在文件夹中显示。
- **多格式**：Markdown / 文本 / 代码、PDF、Word(.docx)、Excel(.xlsx)、PPT(.pptx)。
- **历史记录**：多会话，自动保存，可恢复 / 删除。
- **可配置**：多个知识库文件夹；模型预设（OpenAI / DeepSeek / 智谱 GLM / 通义千问 / 本地 Ollama / 自定义）；浅色 / 深色主题。

## 开发环境

需要 **Node 20+** 和 **Rust**（已通过 rustup 安装）。

```sh
npm install
export PATH="$HOME/.cargo/bin:$PATH"   # 确保 cargo 在 PATH
npm run tauri dev                      # 启动开发版（首次会编译 Rust 依赖，较慢）
```

## 打包

```sh
npm run tauri build                    # macOS 产出 .dmg / .app；Windows 产出 .exe
```

## 数据位置

应用数据在系统数据目录的 `LOCALKB/` 下（macOS：`~/Library/Application Support/com.localkb.app/LOCALKB/`）：

- `index.db` — SQLite 索引（folders / files / chunks / FTS / embeddings）
- `settings.json` — 模型配置、主题、topN
- `history/` — 一会话一个 JSON + `index.json` 元数据
- `models/` — 本地向量模型缓存（首次索引时自动下载，约 100MB）

## 架构

- 后端 `src-tauri/src/`：`commands.rs`（Tauri 命令）、`index.rs`（扫描/分块/写库/向量化）、`search.rs`（混合检索+RRF）、`chat.rs`（远程模型 SSE 流式）、`parsers.rs`（各格式抽文本）、`db.rs`、`embed.rs`、`settings.rs`、`history.rs`。
- 前端 `src/`：React + Vite + Tailwind v4。`views/ChatView`、`views/SettingsView`、`components/`，`lib/api.ts` 封装 `invoke` 与流式 `Channel`。
- IPC：原生 `#[tauri::command]` + `invoke()`；索引进度与问答流式通过 `tauri::ipc::Channel` 推送。

## 快速验证

1. `npm run tauri dev` 启动。
2. 进入「设置」→「添加文件夹」，选择本仓库的 `sample-kb/`，等待索引完成。
3. 在「设置」→「问答模型」选一个预设并填入 API Key（本地 Ollama 可留空，需先 `ollama serve`）。
4. 回到对话，问「默认端口是多少？」——应给出答案并在下方列出 `产品说明.md`，点击可打开。
