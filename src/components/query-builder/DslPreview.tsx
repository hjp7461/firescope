import { useMemo } from "react";
import Editor from "@monaco-editor/react";
import "@/monaco";
import { useQueryStore } from "@/stores/queryStore";

// 빌더 드래프트를 실제 전송될 QueryDsl JSON으로 미리보기 (Monaco 읽기전용).
// 빌드 실패(파싱 에러)면 사유를 그대로 보여줘 사용자가 교정하게 한다.
export function DslPreview() {
  // 드래프트 필드를 구독 → 변경 시 재렌더 → buildDsl 재계산.
  const targetKind = useQueryStore((s) => s.targetKind);
  const target = useQueryStore((s) => s.target);
  const wheres = useQueryStore((s) => s.wheres);
  const orderBys = useQueryStore((s) => s.orderBys);
  const limit = useQueryStore((s) => s.limit);
  const build = useQueryStore((s) => s.build);

  const text = useMemo(() => {
    const r = build();
    return r.ok
      ? JSON.stringify(r.dsl, null, 2)
      : `// 빌드 불가: ${r.error}`;
  }, [build, targetKind, target, wheres, orderBys, limit]);

  return (
    <Editor
      height="100%"
      language="json"
      value={text}
      theme="vs"
      loading={
        <div className="p-3 text-xs text-muted-foreground">에디터 로딩…</div>
      }
      options={{
        readOnly: true,
        domReadOnly: true,
        minimap: { enabled: false },
        lineNumbers: "off",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        fontSize: 11,
        renderLineHighlight: "none",
        folding: false,
      }}
    />
  );
}
