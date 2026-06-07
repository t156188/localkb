import { invoke, Channel } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

// ---- Types -----------------------------------------------------------------

export interface Source {
  index: number;
  chunk_id: number;
  path: string;
  name: string;
  heading: string;
  snippet: string;
}

export interface FolderInfo {
  id: number;
  path: string;
  addedAt: number;
  files: number;
  chunks: number;
}

export interface IndexStatus {
  folders: number;
  files: number;
  chunks: number;
  embeddings: number;
}

export type IndexEvent =
  | { type: "status"; phase: string }
  | { type: "queued"; remaining: number }
  | { type: "start"; total: number }
  | { type: "progress"; done: number; total: number; file: string }
  | { type: "done"; files: number; chunks: number }
  | { type: "error"; message: string }
  | { type: "idle" };

export type AskEvent =
  | { type: "status"; stage: string }
  | { type: "sources"; sources: Source[] }
  | { type: "delta"; text: string }
  | { type: "done"; answer: string }
  | { type: "error"; message: string };

export interface ChatMsg {
  role: "user" | "assistant";
  content: string;
}

export interface UiMessage {
  role: "user" | "assistant";
  content: string;
  sources?: Source[];
  streaming?: boolean;
  status?: string;
}

export interface ProviderCfg {
  id: string;
  name: string;
  preset: string;
  baseURL: string;
  apiKey: string;
  model: string;
}

export interface Settings {
  providers: ProviderCfg[];
  activeProviderId: string;
  theme: "system" | "light" | "dark";
  topN: number;
  /** Auto-reindex a folder when its files change on disk. */
  autoSync: boolean;
}

/**
 * Normalize whatever the backend returns into the multi-provider shape.
 * Migrates the legacy single-`provider` format and backfills missing
 * ids/active selection so the UI always has something valid to render.
 */
export function normalizeSettings(raw: any): Settings {
  let providers: ProviderCfg[] = Array.isArray(raw?.providers) ? raw.providers : [];

  // Legacy format: a single `provider` object, no list.
  if (providers.length === 0 && raw?.provider) {
    const p = raw.provider;
    providers = [
      {
        id: "default",
        name: p.preset || "默认",
        preset: p.preset || "openai",
        baseURL: p.baseURL || "https://api.openai.com/v1",
        apiKey: p.apiKey || "",
        model: p.model || "gpt-4o-mini",
      },
    ];
  }

  // Backfill any missing fields / ids.
  providers = providers.map((p, i) => ({
    id: p.id || `p_${i}`,
    name: p.name || p.preset || `服务商 ${i + 1}`,
    preset: p.preset || "custom",
    baseURL: p.baseURL || "",
    apiKey: p.apiKey || "",
    model: p.model || "",
  }));

  // "auto" (or any id not in the list) means: let the backend pick the first
  // provider. Keep it as-is rather than pinning to a specific provider.
  const activeProviderId =
    typeof raw?.activeProviderId === "string" && raw.activeProviderId ? raw.activeProviderId : "auto";

  return {
    providers,
    activeProviderId,
    theme: raw?.theme ?? "system",
    topN: typeof raw?.topN === "number" ? raw.topN : 8,
    autoSync: typeof raw?.autoSync === "boolean" ? raw.autoSync : true,
  };
}

export interface HistoryMeta {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
}

// ---- Folders / index -------------------------------------------------------

export const listFolders = () => invoke<FolderInfo[]>("list_folders");
export const addFolder = (path: string) =>
  invoke<{ id: number; path: string }>("add_folder", { path });
export const removeFolder = (id: number) => invoke("remove_folder", { id });
export const indexStatus = () => invoke<IndexStatus>("index_status");

/** Queue an index run. Progress arrives globally via `onIndexEvent`. */
export const reindex = (folderId: number | null) => invoke("reindex", { folderId });

/** Subscribe to index progress broadcast by the backend worker. */
export function onIndexEvent(cb: (e: IndexEvent) => void): Promise<UnlistenFn> {
  return listen<IndexEvent>("index-event", (e) => cb(e.payload));
}

// ---- Ask / search ----------------------------------------------------------

export function ask(
  question: string,
  history: ChatMsg[],
  onEvent: (e: AskEvent) => void,
) {
  const channel = new Channel<AskEvent>();
  channel.onmessage = onEvent;
  return invoke("ask", { question, history, onEvent: channel });
}

export const search = (query: string, topK = 10) =>
  invoke<Source[]>("search", { query, topK });

// ---- Settings --------------------------------------------------------------

export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (value: Settings) =>
  invoke("set_settings", { value });

export const listModels = (baseURL: string, apiKey: string) =>
  invoke<string[]>("list_models", { baseUrl: baseURL, apiKey });

// ---- History ---------------------------------------------------------------

export const historyList = () => invoke<HistoryMeta[]>("history_list");
export const historyRead = (id: string) => invoke<any>("history_read", { id });
export const historyWrite = (id: string, record: any) =>
  invoke("history_write", { id, record });
export const historyDelete = (id: string) => invoke("history_delete", { id });

// ---- OS integration --------------------------------------------------------

export async function pickFolder(): Promise<string | null> {
  const result = await openDialog({ directory: true, multiple: false });
  return typeof result === "string" ? result : null;
}

export const openFile = (path: string) => openPath(path);
export const revealFile = (path: string) => revealItemInDir(path);
