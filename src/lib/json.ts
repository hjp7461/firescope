import type { FirestoreDocument, FirestoreValue } from "@/types";

function plain(v: FirestoreValue): unknown {
  switch (v.type) {
    case "null": return null;
    case "bool": return v.value;
    case "double": return v.value;
    case "int": return Number(v.value);
    case "string": return v.value;
    case "timestamp": return v.value;
    case "reference": return v.value;
    case "bytes": return { __type__: "bytes", base64: v.value };
    case "geo": return { __type__: "geopoint", lat: v.lat, lng: v.lng };
    case "array": return v.value.map(plain);
    case "map":
      return Object.fromEntries(
        Object.entries(v.value).map(([k, cv]) => [k, plain(cv)]),
      );
  }
}

/** 결과를 [{<docId>: {필드}}] 형태의 들여쓴 JSON 문자열로. */
export function toDisplayJson(rows: readonly FirestoreDocument[]): string {
  const arr = rows.map((d) => ({
    [d.id]: Object.fromEntries(
      Object.entries(d.data).map(([k, v]) => [k, plain(v)]),
    ),
  }));
  return JSON.stringify(arr, null, 2);
}
