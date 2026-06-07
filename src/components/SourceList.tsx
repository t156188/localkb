import { FolderOpen } from "lucide-react";
import { Source, openFile, revealFile } from "../lib/api";

export function SourceList({ sources }: { sources: Source[] }) {
  if (!sources.length) return null;
  return (
    <div
      className="mt-3 pt-2.5 border-t"
      style={{ borderColor: "var(--border)" }}
    >
      <div className="text-xs mb-1.5" style={{ color: "var(--muted)" }}>
        引用来源 · {sources.length}
      </div>
      <div className="flex flex-col gap-1">
        {sources.map((s) => (
          <div
            key={s.chunk_id}
            className="hoverable group flex items-start gap-2.5 rounded-lg px-2 py-1.5"
          >
            <span
              className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-[11px] font-medium"
              style={{
                background: "color-mix(in srgb, var(--accent) 18%, transparent)",
                color: "var(--accent)",
              }}
            >
              {s.index}
            </span>
            <button
              onClick={() => openFile(s.path)}
              title={`打开 ${s.path}`}
              className="min-w-0 flex-1 text-left"
            >
              <div className="truncate text-sm font-medium">
                {s.name}
                {s.heading && (
                  <span style={{ color: "var(--muted)" }}> · {s.heading}</span>
                )}
              </div>
              <div
                className="truncate text-xs"
                style={{ color: "var(--muted)" }}
              >
                {s.snippet}
              </div>
            </button>
            <button
              onClick={() => revealFile(s.path)}
              title="在文件夹中显示"
              className="hoverable mt-0.5 shrink-0 rounded-md p-1 opacity-0 group-hover:opacity-100"
              style={{ color: "var(--muted)" }}
            >
              <FolderOpen size={15} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
