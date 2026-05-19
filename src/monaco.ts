/**
 * Monaco 오프라인 번들 초기화 모듈.
 *
 * Tauri 데스크탑 앱은 CDN 접근이 불가하므로 @monaco-editor/react의 기본
 * CDN 로딩을 비활성화하고 Vite의 ?worker 번들링을 통해 로컬 워커를 제공한다.
 *
 * 반드시 Editor 컴포넌트보다 먼저 import되어야 한다 (side-effect import).
 */

import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import { loader } from "@monaco-editor/react";

// MonacoEnvironment는 monaco-editor의 editor.api.d.ts에서 Window 인터페이스에
// 선언되어 있다. 브라우저 메인 스레드에서는 self === window이므로 캐스팅.
(self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment =
  {
    getWorker(_: unknown, label: string): Worker {
      return label === "json" ? new jsonWorker() : new editorWorker();
    },
  };

// @monaco-editor/react 가 CDN 대신 로컬 monaco 인스턴스를 사용하도록 설정.
loader.config({ monaco });
