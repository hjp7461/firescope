/**
 * Monaco 오프라인 번들 초기화 모듈.
 *
 * Tauri 데스크탑 앱은 CDN 접근이 불가하므로 @monaco-editor/react의 기본
 * CDN 로딩을 비활성화하고 Vite의 ?worker 번들링을 통해 로컬 워커를 제공한다.
 *
 * 반드시 Editor 컴포넌트보다 먼저 import되어야 한다 (side-effect import).
 */

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import "monaco-editor/esm/vs/language/json/monaco.contribution";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import { loader } from "@monaco-editor/react";

// monaco-editor/esm/vs/editor/editor.api.d.ts가 Window 인터페이스에
// MonacoEnvironment를 전역 선언하므로 직접 할당 가능.
window.MonacoEnvironment = {
  getWorker(_: unknown, label: string): Worker {
    return label === "json" ? new jsonWorker() : new editorWorker();
  },
};

// @monaco-editor/react 가 CDN 대신 로컬 monaco 인스턴스를 사용하도록 설정.
loader.config({ monaco });
