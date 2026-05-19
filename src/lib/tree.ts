import type { FirestoreDocument, FirestoreValue } from "@/types";

export type TreeNode = {
  id: string;
  k: string;
  valuePreview: string;
  typeLabel: string;
  children?: TreeNode[];
};

const LABEL: Record<FirestoreValue["type"], string> = {
  null: "Null", bool: "Boolean", int: "Int", double: "Double",
  string: "String", bytes: "Bytes", timestamp: "Timestamp",
  reference: "Reference", geo: "Geopoint", array: "Array", map: "Map",
};

function valueNode(id: string, k: string, v: FirestoreValue): TreeNode {
  const typeLabel = LABEL[v.type];
  if (v.type === "map") {
    const entries = Object.entries(v.value);
    return {
      id, k, typeLabel, valuePreview: `{${entries.length}}`,
      children: entries.map(([ck, cv]) => valueNode(`${id}.${ck}`, ck, cv)),
    };
  }
  if (v.type === "array") {
    return {
      id, k, typeLabel, valuePreview: `[${v.value.length}]`,
      children: v.value.map((cv, i) => valueNode(`${id}.${i}`, String(i), cv)),
    };
  }
  const preview =
    v.type === "null" ? "null"
    : v.type === "bool" ? String(v.value)
    : v.type === "double" ? String(v.value)
    : v.type === "geo" ? `(${v.lat}, ${v.lng})`
    : v.value;
  return { id, k, typeLabel, valuePreview: preview };
}

/** 결과 문서들을 react-arborist용 트리로. 문서=최상위 노드. */
export function buildTree(rows: readonly FirestoreDocument[]): TreeNode[] {
  return rows.map((d) => ({
    id: d.id,
    k: d.id,
    typeLabel: "Document",
    valuePreview: "",
    children: Object.entries(d.data).map(([k, v]) =>
      valueNode(`${d.id}.${k}`, k, v),
    ),
  }));
}
