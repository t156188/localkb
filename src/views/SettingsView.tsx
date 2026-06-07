import { useEffect, useRef, useState } from "react";
import {
  FolderPlus,
  RefreshCw,
  Trash2,
  FileText,
  Check,
  Plus,
  Search,
  Loader2,
  ChevronDown,
  Eye,
  EyeOff,
} from "lucide-react";
import { FolderInfo, ProviderCfg, Settings, listModels } from "../lib/api";

export interface IndexingState {
  active: boolean;
  done: number;
  total: number;
  file: string;
  /** Coarse phase label shown before per-file progress (scan / model load). */
  phase: string;
  /** Folders still waiting in the index queue behind the current one. */
  queued: number;
}

const PRESETS = [
  { id: "deepseek", label: "DeepSeek", baseURL: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  { id: "openai", label: "OpenAI", baseURL: "https://api.openai.com/v1", model: "gpt-4o-mini" },
  { id: "zhipu", label: "智谱 GLM", baseURL: "https://open.bigmodel.cn/api/paas/v4", model: "glm-4-flash" },
  { id: "qwen", label: "通义千问", baseURL: "https://dashscope.aliyuncs.com/compatible-mode/v1", model: "qwen-plus" },
  { id: "ollama", label: "本地 Ollama", baseURL: "http://localhost:11434/v1", model: "qwen2.5:7b-instruct" },
  { id: "custom", label: "自定义", baseURL: "", model: "" },
];

const PRESET_LABELS = new Set(PRESETS.map((p) => p.label));

const emptyDraft = (): Omit<ProviderCfg, "id"> => {
  const p = PRESETS[0];
  return { name: p.label, preset: p.id, baseURL: p.baseURL, model: p.model, apiKey: "" };
};

const newId = () => `p_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;

interface Props {
  settings: Settings;
  folders: FolderInfo[];
  indexing: IndexingState;
  onSave: (s: Settings) => void;
  onAddFolder: () => void;
  onRemoveFolder: (id: number) => void;
  onReindex: (folderId: number | null) => void;
}

export function SettingsView({
  settings,
  folders,
  indexing,
  onSave,
  onAddFolder,
  onRemoveFolder,
  onReindex,
}: Props) {
  const { providers, activeProviderId } = settings;
  const [topN, setTopN] = useState(settings.topN);
  const [saved, setSaved] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  // Show the add form by default only until the first provider exists.
  const [showAdd, setShowAdd] = useState(providers.length === 0);

  const card = {
    background: "var(--panel)",
    borderColor: "var(--border)",
  };

  const flash = () => {
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  const addProvider = (vals: Omit<ProviderCfg, "id">) => {
    const entry: ProviderCfg = { ...vals, id: newId(), name: vals.name.trim() || vals.preset };
    onSave({ ...settings, providers: [...providers, entry], activeProviderId, topN });
    setShowAdd(false);
  };

  const updateProvider = (id: string, vals: Omit<ProviderCfg, "id">) => {
    onSave({
      ...settings,
      providers: providers.map((p) =>
        p.id === id ? { ...vals, id, name: vals.name.trim() || vals.preset } : p,
      ),
      topN,
    });
    setEditingId(null);
  };

  const removeProvider = (id: string) => {
    const next = providers.filter((p) => p.id !== id);
    onSave({
      ...settings,
      providers: next,
      activeProviderId: id === activeProviderId ? "auto" : activeProviderId,
      topN,
    });
    setEditingId(null);
    if (next.length === 0) setShowAdd(true);
  };

  const saveTopN = () => {
    onSave({ ...settings, topN });
    flash();
  };

  const setTheme = (theme: Settings["theme"]) => onSave({ ...settings, theme, topN });

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-2xl px-6 py-8">
        <h1 className="mb-6 text-xl font-semibold">设置</h1>

        {/* Folders -------------------------------------------------------- */}
        <section className="mb-6 rounded-xl border p-4" style={card}>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="font-medium">知识库文件夹</h2>
            <div className="flex gap-2">
              <button
                onClick={() => onReindex(null)}
                disabled={indexing.active || folders.length === 0}
                className="hoverable flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-sm disabled:opacity-40"
                style={{ borderColor: "var(--border)" }}
              >
                <RefreshCw size={14} className={indexing.active ? "animate-spin" : ""} />
                重建全部
              </button>
              <button
                onClick={onAddFolder}
                className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-sm text-white"
                style={{ background: "var(--accent)" }}
              >
                <FolderPlus size={14} />
                添加文件夹
              </button>
            </div>
          </div>

          {indexing.active && (
            <div className="mb-3">
              <div className="mb-1 flex justify-between text-xs" style={{ color: "var(--muted)" }}>
                {/* Before per-file progress starts, show the phase label so the
                    first-run model download / tree scan doesn't read as a hang. */}
                <span className="truncate">
                  {indexing.file
                    ? `正在索引 ${indexing.file.split("/").pop()}`
                    : indexing.phase || "正在准备…"}
                  {indexing.queued > 0 && ` · 队列中还有 ${indexing.queued} 个`}
                </span>
                {indexing.total > 0 && (
                  <span>
                    {indexing.done}/{indexing.total}
                  </span>
                )}
              </div>
              <div className="h-1.5 w-full overflow-hidden rounded-full" style={{ background: "var(--border)" }}>
                <div
                  className={`h-full rounded-full transition-all ${
                    indexing.total ? "" : "animate-pulse"
                  }`}
                  style={{
                    background: "var(--accent)",
                    width: `${indexing.total ? (indexing.done / indexing.total) * 100 : 100}%`,
                  }}
                />
              </div>
            </div>
          )}

          {folders.length === 0 ? (
            <p className="py-4 text-center text-sm" style={{ color: "var(--muted)" }}>
              还没有文件夹。添加一个本地文件夹开始构建知识库。
            </p>
          ) : (
            <div className="flex flex-col gap-1.5">
              {folders.map((f) => (
                <div
                  key={f.id}
                  className="flex items-center gap-3 rounded-lg border px-3 py-2"
                  style={{ borderColor: "var(--border)" }}
                >
                  <FileText size={16} style={{ color: "var(--muted)" }} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm" title={f.path}>
                      {f.path}
                    </div>
                    <div className="text-xs" style={{ color: "var(--muted)" }}>
                      {f.files} 个文件 · {f.chunks} 个片段
                    </div>
                  </div>
                  <button
                    onClick={() => onReindex(f.id)}
                    disabled={indexing.active}
                    className="hoverable rounded-md p-1.5 disabled:opacity-40"
                    title="重建此文件夹"
                    style={{ color: "var(--muted)" }}
                  >
                    <RefreshCw size={15} />
                  </button>
                  <button
                    onClick={() => onRemoveFolder(f.id)}
                    className="hoverable rounded-md p-1.5"
                    title="移除"
                    style={{ color: "var(--muted)" }}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Auto-sync toggle */}
          <div className="mt-3 flex items-center justify-between border-t pt-3" style={{ borderColor: "var(--border)" }}>
            <div className="min-w-0">
              <div className="text-sm">自动同步</div>
              <div className="text-xs" style={{ color: "var(--muted)" }}>
                文件夹内容变化时自动更新索引，无需手动重建。
              </div>
            </div>
            <button
              role="switch"
              aria-checked={settings.autoSync}
              onClick={() => onSave({ ...settings, autoSync: !settings.autoSync })}
              className="relative h-6 w-10 shrink-0 rounded-full transition-colors"
              style={{ background: settings.autoSync ? "var(--accent)" : "var(--border)" }}
              title={settings.autoSync ? "点击关闭自动同步" : "点击开启自动同步"}
            >
              <span
                className="absolute top-0.5 h-5 w-5 rounded-full bg-white transition-all"
                style={{ left: settings.autoSync ? "1.125rem" : "0.125rem" }}
              />
            </button>
          </div>
        </section>

        {/* Model ---------------------------------------------------------- */}
        <section className="mb-6 rounded-xl border p-4" style={card}>
          <h2 className="mb-3 font-medium">问答模型</h2>

          <p className="mb-3 text-xs" style={{ color: "var(--muted)" }}>
            在这里管理服务商；具体用哪个在聊天框里选择（默认「自动」）。
          </p>

          {/* Saved providers — click a row to expand & edit */}
          {providers.length > 0 && (
            <div className="mb-4 flex flex-col gap-1.5">
              {providers.map((p) => {
                const expanded = editingId === p.id;
                return (
                  <div
                    key={p.id}
                    className="rounded-lg border"
                    style={{ borderColor: expanded ? "var(--accent)" : "var(--border)" }}
                  >
                    <button
                      onClick={() => setEditingId(expanded ? null : p.id)}
                      className="hoverable flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-sm font-medium">{p.name}</div>
                        <div className="truncate text-xs" style={{ color: "var(--muted)" }}>
                          {p.model} · {p.baseURL}
                        </div>
                      </div>
                      <ChevronDown
                        size={16}
                        className="shrink-0"
                        style={{
                          color: "var(--muted)",
                          transition: "transform .15s",
                          transform: expanded ? "rotate(180deg)" : "none",
                        }}
                      />
                    </button>
                    {expanded && (
                      <div className="border-t px-3 py-3" style={{ borderColor: "var(--border)" }}>
                        <ProviderForm
                          initial={p}
                          submitLabel="保存"
                          onSubmit={(vals) => updateProvider(p.id, vals)}
                          onCancel={() => setEditingId(null)}
                          onDelete={() => removeProvider(p.id)}
                        />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}

          {/* Add provider — collapsed behind a button once one exists */}
          {showAdd ? (
            <div className="rounded-lg border border-dashed p-3" style={{ borderColor: "var(--border)" }}>
              <div className="mb-2 text-xs font-medium" style={{ color: "var(--muted)" }}>
                添加服务商
              </div>
              <ProviderForm
                initial={emptyDraft()}
                submitLabel="添加"
                onSubmit={addProvider}
                onCancel={providers.length > 0 ? () => setShowAdd(false) : undefined}
              />
            </div>
          ) : (
            <button
              onClick={() => setShowAdd(true)}
              className="hoverable flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed py-2 text-sm"
              style={{ borderColor: "var(--border)", color: "var(--muted)" }}
            >
              <Plus size={15} />
              添加服务商
            </button>
          )}

          {/* Retrieval */}
          <div className="mt-4">
            <Field label={`检索片段数（topN）：${topN}`}>
              <input
                type="range"
                min={1}
                max={15}
                value={topN}
                onChange={(e) => setTopN(Number(e.target.value))}
                className="w-full"
              />
            </Field>
            <button
              onClick={saveTopN}
              className="mt-2 flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-sm"
              style={{ borderColor: "var(--border)" }}
            >
              {saved ? <Check size={15} /> : null}
              {saved ? "已保存" : "保存检索设置"}
            </button>
          </div>
        </section>

        {/* Appearance ----------------------------------------------------- */}
        <section className="rounded-xl border p-4" style={card}>
          <h2 className="mb-3 font-medium">外观</h2>
          <div className="flex gap-1.5">
            {(["system", "light", "dark"] as const).map((t) => (
              <button
                key={t}
                onClick={() => setTheme(t)}
                className="rounded-lg border px-3 py-1.5 text-sm"
                style={{
                  borderColor: settings.theme === t ? "var(--accent)" : "var(--border)",
                  color: settings.theme === t ? "var(--accent)" : "var(--text)",
                }}
              >
                {t === "system" ? "跟随系统" : t === "light" ? "浅色" : "深色"}
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="mb-1.5 block text-xs" style={{ color: "var(--muted)" }}>
        {label}
      </label>
      {children}
    </div>
  );
}

/**
 * Add/edit form for a single provider. Self-contained: owns its draft plus the
 * model-search state, so it works the same whether adding a new provider or
 * editing an existing one.
 */
function ProviderForm({
  initial,
  submitLabel,
  onSubmit,
  onCancel,
  onDelete,
}: {
  initial: Omit<ProviderCfg, "id">;
  submitLabel: string;
  onSubmit: (vals: Omit<ProviderCfg, "id">) => void;
  onCancel?: () => void;
  onDelete?: () => void;
}) {
  const [draft, setDraft] = useState(initial);
  const [showKey, setShowKey] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsError, setModelsError] = useState("");

  const fetchModels = async (baseURL: string, apiKey: string) => {
    if (!baseURL.trim()) return;
    setModelsLoading(true);
    setModelsError("");
    setModels([]);
    try {
      const list = await listModels(baseURL.trim(), apiKey.trim());
      setModels(list);
      if (list.length === 0) setModelsError("未返回任何模型");
    } catch (e) {
      setModelsError(String(e));
    } finally {
      setModelsLoading(false);
    }
  };

  // Pick a preset template; preserve a custom name, then fetch its model list.
  const applyPreset = (id: string) => {
    const p = PRESETS.find((x) => x.id === id)!;
    const next = {
      ...draft,
      preset: id,
      name: !draft.name || PRESET_LABELS.has(draft.name) ? p.label : draft.name,
      baseURL: id === "custom" ? draft.baseURL : p.baseURL,
      model: id === "custom" ? draft.model : p.model,
    };
    setDraft(next);
    setModels([]);
    setModelsError("");
    if (next.baseURL.trim()) fetchModels(next.baseURL, next.apiKey);
  };

  const canSubmit = !!draft.baseURL.trim() && !!draft.model.trim();

  return (
    <div className="grid grid-cols-1 gap-3">
      <Field label="预设模板">
        <select value={draft.preset} onChange={(e) => applyPreset(e.target.value)} className="input">
          {PRESETS.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
      </Field>
      <Field label="名称">
        <input
          value={draft.name}
          onChange={(e) => setDraft({ ...draft, name: e.target.value })}
          placeholder="例如：我的 DeepSeek"
          className="input"
        />
      </Field>
      <Field label="Base URL">
        <input
          value={draft.baseURL}
          onChange={(e) => setDraft({ ...draft, baseURL: e.target.value, preset: "custom" })}
          placeholder="https://api.openai.com/v1"
          className="input"
        />
      </Field>
      <Field label="API Key（本地 Ollama 可留空）">
        <div className="relative">
          <input
            type={showKey ? "text" : "password"}
            value={draft.apiKey}
            onChange={(e) => setDraft({ ...draft, apiKey: e.target.value })}
            placeholder="sk-..."
            className="input pr-9"
          />
          <button
            type="button"
            tabIndex={-1}
            onClick={() => setShowKey((s) => !s)}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-0.5"
            style={{ color: "var(--muted)" }}
            title={showKey ? "隐藏" : "显示"}
          >
            {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        </div>
      </Field>
      <Field label="模型名称（可搜索后选择，或直接填写）">
        <div className="flex gap-2">
          <ModelCombobox
            value={draft.model}
            options={models}
            onChange={(v) => setDraft({ ...draft, model: v })}
            placeholder="deepseek-chat"
          />
          <button
            onClick={() => fetchModels(draft.baseURL, draft.apiKey)}
            disabled={!draft.baseURL.trim() || modelsLoading}
            className="hoverable flex shrink-0 items-center gap-1.5 rounded-lg border px-2.5 text-sm disabled:opacity-40"
            style={{ borderColor: "var(--border)" }}
            title="从该服务商获取模型列表"
          >
            {modelsLoading ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />}
            搜索模型
          </button>
        </div>
        {modelsError && (
          <p className="mt-1 text-xs" style={{ color: "#ef4444" }}>
            {modelsError}
          </p>
        )}
        {!modelsError && models.length > 0 && (
          <p className="mt-1 text-xs" style={{ color: "var(--muted)" }}>
            已获取 {models.length} 个模型，点开输入框可选择。
          </p>
        )}
      </Field>

      <div className="flex items-center gap-2">
        <button
          onClick={() => canSubmit && onSubmit(draft)}
          disabled={!canSubmit}
          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm text-white disabled:opacity-40"
          style={{ background: "var(--accent)" }}
        >
          {submitLabel === "添加" ? <Plus size={15} /> : <Check size={15} />}
          {submitLabel}
        </button>
        {onCancel && (
          <button
            onClick={onCancel}
            className="hoverable rounded-lg border px-3 py-1.5 text-sm"
            style={{ borderColor: "var(--border)" }}
          >
            取消
          </button>
        )}
        {onDelete && (
          <button
            onClick={onDelete}
            className="hoverable ml-auto flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm"
            style={{ color: "#ef4444" }}
          >
            <Trash2 size={15} />
            删除
          </button>
        )}
      </div>
    </div>
  );
}

/**
 * Searchable model picker: a single input that opens a scrollable list of all
 * fetched models. Typing narrows the list (and is also accepted as a custom
 * value); clicking an item fills it.
 */
function ModelCombobox({
  value,
  options,
  onChange,
  placeholder,
}: {
  value: string;
  options: string[];
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const q = value.trim().toLowerCase();
  const matches = q ? options.filter((o) => o.toLowerCase().includes(q)) : options;
  // Keep the full list reachable even when the typed value matches nothing.
  const list = matches.length > 0 ? matches : options;
  const hasOptions = options.length > 0;

  return (
    <div ref={ref} className="relative flex-1">
      <input
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
        placeholder={placeholder}
        className="input w-full pr-8"
      />
      <button
        type="button"
        tabIndex={-1}
        onClick={() => hasOptions && setOpen((o) => !o)}
        className="absolute right-2 top-1/2 -translate-y-1/2 p-0.5"
        style={{ color: "var(--muted)", cursor: hasOptions ? "pointer" : "default" }}
        title={hasOptions ? "展开模型列表" : "先点「搜索模型」获取列表"}
      >
        <ChevronDown size={16} />
      </button>

      {open && hasOptions && (
        <div
          className="absolute left-0 right-0 z-10 mt-1 max-h-56 overflow-y-auto rounded-lg border shadow-lg"
          style={{ background: "var(--panel)", borderColor: "var(--border)" }}
        >
          {list.map((o) => (
            <button
              key={o}
              type="button"
              onClick={() => {
                onChange(o);
                setOpen(false);
              }}
              className="hoverable flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-sm"
              style={{ color: "var(--text)" }}
            >
              <span className="truncate">{o}</span>
              {o === value && <Check size={14} className="shrink-0" style={{ color: "var(--accent)" }} />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
