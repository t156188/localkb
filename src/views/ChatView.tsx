import { useEffect, useRef, useState } from "react";
import { ArrowUp, Loader2, Database, Sparkles } from "lucide-react";
import { ProviderCfg, UiMessage } from "../lib/api";
import { Markdown } from "../components/Markdown";
import { SourceList } from "../components/SourceList";

interface Props {
  messages: UiMessage[];
  asking: boolean;
  chunkCount: number;
  providers: ProviderCfg[];
  activeProviderId: string;
  onSelectProvider: (id: string) => void;
  onSend: (text: string) => void;
}

const SUGGESTIONS = [
  "总结一下这些文档的主要内容",
  "关于……有哪些关键信息？",
  "帮我找到提到……的文件",
];

export function ChatView({
  messages,
  asking,
  chunkCount,
  providers,
  activeProviderId,
  onSelectProvider,
  onSend,
}: Props) {
  const [input, setInput] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [messages]);

  const submit = () => {
    const t = input.trim();
    if (!t || asking) return;
    onSend(t);
    setInput("");
    if (taRef.current) taRef.current.style.height = "auto";
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  const autosize = () => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = Math.min(ta.scrollHeight, 260) + "px";
  };

  return (
    <div className="flex h-full flex-col">
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        {messages.length === 0 ? (
          <Empty chunkCount={chunkCount} onPick={(s) => setInput(s)} />
        ) : (
          <div className="mx-auto flex max-w-3xl flex-col gap-5 px-5 py-6">
            {messages.map((m, i) => (
              <Bubble key={i} msg={m} />
            ))}
          </div>
        )}
      </div>

      {/* Composer */}
      <div className="px-5 pb-5">
        <div
          className="mx-auto flex max-w-3xl flex-col gap-2 rounded-2xl border p-2.5 shadow-sm"
          style={{ background: "var(--panel)", borderColor: "var(--border)" }}
        >
          <textarea
            ref={taRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              autosize();
            }}
            onKeyDown={onKeyDown}
            rows={3}
            placeholder="向你的知识库提问…（Enter 发送，Shift+Enter 换行）"
            className="max-h-[260px] min-h-[88px] w-full resize-none bg-transparent px-1.5 py-1 text-[15px] outline-none"
            style={{ color: "var(--text)" }}
          />
          <div className="flex items-center justify-end gap-2">
            <select
              value={providers.some((p) => p.id === activeProviderId) ? activeProviderId : "auto"}
              onChange={(e) => onSelectProvider(e.target.value)}
              className="rounded-lg border bg-transparent px-2 py-1.5 text-xs outline-none"
              style={{ borderColor: "var(--border)", color: "var(--text)" }}
              title="选择问答使用的服务商"
            >
              <option value="auto">自动选择</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} · {p.model}
                </option>
              ))}
            </select>
            <button
              onClick={submit}
              disabled={!input.trim() || asking}
              className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl text-white disabled:opacity-40"
              style={{ background: "var(--accent)" }}
            >
              {asking ? <Loader2 size={18} className="animate-spin" /> : <ArrowUp size={18} />}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function Bubble({ msg }: { msg: UiMessage }) {
  if (msg.role === "user") {
    return (
      <div className="flex justify-end">
        <div
          className="max-w-[80%] whitespace-pre-wrap rounded-2xl px-4 py-2.5 text-[15px] text-white"
          style={{ background: "var(--accent)" }}
        >
          {msg.content}
        </div>
      </div>
    );
  }
  return (
    <div
      className="rounded-2xl border px-4 py-3"
      style={{ background: "var(--panel)", borderColor: "var(--border)" }}
    >
      {msg.content ? (
        <Markdown text={msg.content} />
      ) : msg.streaming ? (
        <div className="flex items-center gap-2 text-sm" style={{ color: "var(--muted)" }}>
          <Loader2 size={14} className="animate-spin" /> {msg.status || "正在思考…"}
        </div>
      ) : null}
      {msg.sources && msg.sources.length > 0 && <SourceList sources={msg.sources} />}
    </div>
  );
}

function Empty({ chunkCount, onPick }: { chunkCount: number; onPick: (s: string) => void }) {
  return (
    <div className="flex h-full flex-col items-center justify-center px-6 text-center">
      <div
        className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl"
        style={{ background: "color-mix(in srgb, var(--accent) 14%, transparent)", color: "var(--accent)" }}
      >
        <Sparkles size={26} />
      </div>
      <h2 className="mb-1.5 text-lg font-semibold">本地知识检索</h2>
      <p className="mb-5 max-w-sm text-sm" style={{ color: "var(--muted)" }}>
        {chunkCount > 0 ? (
          <>知识库已就绪，已索引 {chunkCount} 个文本片段。开始提问吧。</>
        ) : (
          <span className="inline-flex items-center gap-1.5">
            <Database size={14} /> 还没有索引内容 — 先在设置里添加文件夹并构建索引。
          </span>
        )}
      </p>
      {chunkCount > 0 && (
        <div className="flex flex-col items-stretch gap-2">
          {SUGGESTIONS.map((s) => (
            <button
              key={s}
              onClick={() => onPick(s)}
              className="hoverable rounded-xl border px-4 py-2 text-sm"
              style={{ borderColor: "var(--border)" }}
            >
              {s}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
