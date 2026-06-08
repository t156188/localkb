import { useCallback, useEffect, useRef, useState } from "react";
import { Plus, Settings as SettingsIcon, MessageSquare, Trash2, Database } from "lucide-react";
import {
  Settings,
  FolderInfo,
  IndexStatus,
  HistoryMeta,
  UiMessage,
  ChatMsg,
  getSettings,
  normalizeSettings,
  setSettings as saveSettingsApi,
  listFolders,
  indexStatus,
  historyList,
  historyRead,
  historyWrite,
  historyDelete,
  addFolder,
  removeFolder,
  reindex,
  onIndexEvent,
  ask,
  pickFolder,
  confirmRemoveFolder,
} from "./lib/api";
import { ChatView } from "./views/ChatView";
import { SettingsView, IndexingState } from "./views/SettingsView";

const NO_INDEXING: IndexingState = { active: false, done: 0, total: 0, file: "", phase: "", queued: 0 };

function genId() {
  return `c_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
}

function updateLast(msgs: UiMessage[], fn: (m: UiMessage) => UiMessage): UiMessage[] {
  if (msgs.length === 0) return msgs;
  const next = msgs.slice();
  next[next.length - 1] = fn(next[next.length - 1]);
  return next;
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [view, setView] = useState<"chat" | "settings">("chat");
  const [folders, setFolders] = useState<FolderInfo[]>([]);
  const [status, setStatus] = useState<IndexStatus>({ folders: 0, files: 0, chunks: 0, embeddings: 0 });
  const [convos, setConvos] = useState<HistoryMeta[]>([]);
  const [messages, setMessages] = useState<UiMessage[]>([]);
  const [asking, setAsking] = useState(false);
  const [indexing, setIndexing] = useState<IndexingState>(NO_INDEXING);
  const convoRef = useRef<{ id: string; title: string; createdAt: number } | null>(null);

  // ---- Loading & theme -----------------------------------------------------

  const refreshFolders = useCallback(async () => {
    setFolders(await listFolders());
    setStatus(await indexStatus());
  }, []);
  const refreshHistory = useCallback(async () => setConvos(await historyList()), []);

  useEffect(() => {
    (async () => {
      setSettings(normalizeSettings(await getSettings()));
      await refreshFolders();
      await refreshHistory();
    })();
  }, [refreshFolders, refreshHistory]);

  useEffect(() => {
    if (!settings) return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = settings.theme === "dark" || (settings.theme === "system" && mq.matches);
      document.documentElement.classList.toggle("dark", dark);
    };
    apply();
    if (settings.theme === "system") {
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  }, [settings]);

  const saveSettings = async (s: Settings) => {
    setSettings(s);
    await saveSettingsApi(s);
  };

  // ---- Conversations -------------------------------------------------------

  const newChat = () => {
    convoRef.current = null;
    setMessages([]);
    setView("chat");
  };

  const openConvo = async (id: string) => {
    try {
      const rec = await historyRead(id);
      convoRef.current = { id: rec.id, title: rec.title, createdAt: rec.createdAt };
      setMessages(rec.messages || []);
      setView("chat");
    } catch (e) {
      console.error(e);
    }
  };

  const deleteConvo = async (id: string) => {
    await historyDelete(id);
    if (convoRef.current?.id === id) newChat();
    refreshHistory();
  };

  const persist = useCallback(
    async (msgs: UiMessage[]) => {
      const meta = convoRef.current;
      if (!meta) return;
      await historyWrite(meta.id, {
        id: meta.id,
        title: meta.title,
        createdAt: meta.createdAt,
        updatedAt: Date.now(),
        messages: msgs.map((m) => ({ role: m.role, content: m.content, sources: m.sources ?? [] })),
      });
      refreshHistory();
    },
    [refreshHistory],
  );

  const send = useCallback(
    async (text: string) => {
      if (!convoRef.current) {
        convoRef.current = { id: genId(), title: text.slice(0, 30), createdAt: Date.now() };
      }
      const history: ChatMsg[] = messages.map((m) => ({ role: m.role, content: m.content }));
      setMessages((prev) => [
        ...prev,
        { role: "user", content: text },
        { role: "assistant", content: "", sources: [], streaming: true },
      ]);
      setAsking(true);

      await ask(text, history, (e) => {
        if (e.type === "status") {
          setMessages((prev) => updateLast(prev, (m) => ({ ...m, status: e.stage })));
        } else if (e.type === "sources") {
          setMessages((prev) => updateLast(prev, (m) => ({ ...m, sources: e.sources })));
        } else if (e.type === "delta") {
          setMessages((prev) => updateLast(prev, (m) => ({ ...m, content: m.content + e.text })));
        } else if (e.type === "done") {
          setMessages((prev) => {
            const next = updateLast(prev, (m) => ({ ...m, streaming: false }));
            persist(next);
            return next;
          });
          setAsking(false);
        } else if (e.type === "error") {
          setMessages((prev) =>
            updateLast(prev, (m) => ({ ...m, content: `⚠️ ${e.message}`, streaming: false })),
          );
          setAsking(false);
        }
      });
    },
    [messages, persist],
  );

  // ---- Indexing ------------------------------------------------------------
  // All index work (manual reindex AND filesystem auto-sync) is serialized by a
  // single backend worker and reported on one global `index-event`. We just
  // queue requests and reflect progress from that stream — it runs in the
  // background, so chat stays usable and you can keep adding folders.
  const enqueueReindex = useCallback((folderId: number | null) => {
    reindex(folderId);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onIndexEvent((e) => {
      if (e.type === "queued")
        setIndexing((s) => ({ ...s, active: true, queued: e.remaining, phase: s.phase || "正在准备…" }));
      else if (e.type === "status")
        setIndexing((s) => ({ ...s, active: true, phase: e.phase }));
      else if (e.type === "start")
        setIndexing((s) => ({ ...s, active: true, done: 0, total: e.total, file: "", phase: "" }));
      else if (e.type === "progress")
        setIndexing((s) => ({ ...s, active: true, done: e.done, total: e.total, file: e.file, phase: "" }));
      else if (e.type === "done") refreshFolders();
      else if (e.type === "error") {
        console.error("index error:", e.message);
        refreshFolders();
      } else if (e.type === "idle") setIndexing(NO_INDEXING);
    }).then((un) => {
      if (cancelled) un();
      else unlisten = un;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refreshFolders]);

  const onAddFolder = async () => {
    const path = await pickFolder();
    if (!path) return;
    const f = await addFolder(path);
    await refreshFolders();
    enqueueReindex(f.id);
  };

  const onRemoveFolder = async (id: number) => {
    const folder = folders.find((f) => f.id === id);
    const ok = await confirmRemoveFolder(folder?.path ?? "");
    if (!ok) return;
    await removeFolder(id);
    refreshFolders();
  };

  if (!settings) return null;

  // ---- Render --------------------------------------------------------------

  return (
    <div className="flex h-screen">
      {/* Sidebar */}
      <aside
        className="flex w-64 shrink-0 flex-col border-r"
        style={{ background: "var(--panel)", borderColor: "var(--border)" }}
      >
        <div className="px-4 pb-2 pt-4">
          <div className="flex items-center gap-2">
            <div
              className="flex h-7 w-7 items-center justify-center rounded-lg text-white"
              style={{ background: "var(--accent)" }}
            >
              <Database size={16} />
            </div>
            <div>
              <div className="text-sm font-semibold leading-tight">知索</div>
              <div className="text-[11px] leading-tight" style={{ color: "var(--muted)" }}>
                本地知识检索
              </div>
            </div>
          </div>
        </div>

        <div className="px-3 pb-2">
          <button
            onClick={newChat}
            className="flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-sm"
            style={{ borderColor: "var(--border)" }}
          >
            <Plus size={16} /> 新对话
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-2">
          {convos.length === 0 ? (
            <p className="px-2 py-4 text-xs" style={{ color: "var(--muted)" }}>
              暂无历史对话
            </p>
          ) : (
            convos.map((c) => (
              <div
                key={c.id}
                onClick={() => openConvo(c.id)}
                className="hoverable group flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-2 text-sm"
                style={
                  convoRef.current?.id === c.id
                    ? { background: "color-mix(in srgb, var(--accent) 12%, transparent)" }
                    : undefined
                }
              >
                <MessageSquare size={14} style={{ color: "var(--muted)" }} className="shrink-0" />
                <span className="min-w-0 flex-1 truncate">{c.title || "新对话"}</span>
                <button
                  onClick={(ev) => {
                    ev.stopPropagation();
                    deleteConvo(c.id);
                  }}
                  className="shrink-0 rounded p-0.5 opacity-0 group-hover:opacity-100"
                  style={{ color: "var(--muted)" }}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            ))
          )}
        </div>

        <div className="border-t p-2" style={{ borderColor: "var(--border)" }}>
          <button
            onClick={() => setView((v) => (v === "settings" ? "chat" : "settings"))}
            className="hoverable flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm"
            style={view === "settings" ? { color: "var(--accent)" } : undefined}
          >
            <SettingsIcon size={16} /> 设置
            <span className="ml-auto text-[11px]" style={{ color: "var(--muted)" }}>
              {status.chunks} 片段
            </span>
          </button>
        </div>
      </aside>

      {/* Main */}
      <main className="min-w-0 flex-1" style={{ background: "var(--bg)" }}>
        {view === "chat" ? (
          <ChatView
            messages={messages}
            asking={asking}
            chunkCount={status.chunks}
            providers={settings.providers}
            activeProviderId={settings.activeProviderId}
            onSelectProvider={(id) => saveSettings({ ...settings, activeProviderId: id })}
            onSend={send}
          />
        ) : (
          <SettingsView
            settings={settings}
            folders={folders}
            indexing={indexing}
            onSave={saveSettings}
            onAddFolder={onAddFolder}
            onRemoveFolder={onRemoveFolder}
            onReindex={enqueueReindex}
          />
        )}
      </main>
    </div>
  );
}
