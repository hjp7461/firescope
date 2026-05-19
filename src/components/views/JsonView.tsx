import { useMemo } from "react";
import Editor from "@monaco-editor/react";
import "@/monaco";
import { useResultStore } from "@/stores/resultStore";
import { toDisplayJson } from "@/lib/json";

export function JsonView() {
  const rows = useResultStore((s) => s.rows);
  const status = useResultStore((s) => s.status);
  const text = useMemo(() => toDisplayJson(rows), [rows]);

  if (rows.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {status === "streaming" ? "불러오는 중…" : "결과 없음"}
      </div>
    );
  }
  return (
    <Editor
      height="100%"
      language="json"
      value={text}
      theme="vs"
      loading={
        <div className="p-4 text-sm text-muted-foreground">에디터 로딩…</div>
      }
      options={{
        readOnly: true,
        domReadOnly: true,
        minimap: { enabled: false },
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        wordWrap: "off",
        fontSize: 12,
        renderLineHighlight: "none",
      }}
    />
  );
}
