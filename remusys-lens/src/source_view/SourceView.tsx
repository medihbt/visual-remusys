import { useEffect, useRef } from "react";
import Editor, { type Monaco } from "@monaco-editor/react";
import { editor } from "monaco-editor";

import { useIRStore } from "../ir/state";
import llvmMonarch from "./llvmMonarch.ts";
import "./SourceView.css";
import { IRTreeCursor, ModuleInfo, type IRObjPath, type MonacoSrcRange } from "remusys-wasm";

function handleEditorMount(
  ed: editor.IStandaloneCodeEditor,
  monaco: Monaco,
  editorRef: React.RefObject<editor.IStandaloneCodeEditor | null>,
  monacoRef: React.RefObject<Monaco | null>,
) {
  try {
    const exists = monaco.languages
      .getLanguages()
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      .some((lang: any) => lang.id === "llvm");
    if (!exists) monaco.languages.register({ id: "llvm" });
    monaco.languages.setMonarchTokensProvider("llvm", llvmMonarch);

    const model = ed.getModel();
    if (model) monaco.editor.setModelLanguage(model, "llvm");
  } catch (error) {
    console.warn("SourceView: failed to register llvm monarch", error);
  }

  editorRef.current = ed;
  monacoRef.current = monaco;
}

function getReferenceSourceRanges(module: ModuleInfo, focus: IRObjPath): MonacoSrcRange[] {
  const cursor = IRTreeCursor.from_path(module, focus);
  let ranges: MonacoSrcRange[] = [];
  try {
    ranges = cursor.get_reference_source_ranges(module);
  } finally {
    cursor.free();
  }
  return ranges;
}

export default function SourceView() {
  const source = useIRStore((s) => s.source);
  const focus = useIRStore((s) => s.focus);
  const module = useIRStore((s) => s.module);
  const getFocusSrcRange = useIRStore((s) => s.getFocusSrcRange);

  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const decorationsRef = useRef<string[]>([]);

  useEffect(() => {
    const ed = editorRef.current;
    const monaco = monacoRef.current;
    if (!ed || !monaco || !module) {
      return;
    }

    try {
      if (focus.length > 1) {
        const rangeDt = getFocusSrcRange();
        const referenceRanges = getReferenceSourceRanges(module, focus);
        const focusRange = new monaco.Range(
          rangeDt.start.line,
          rangeDt.start.column,
          rangeDt.end.line,
          rangeDt.end.column,
        );
        const referenceDecorations = referenceRanges.map((referenceRange) => ({
          range: new monaco.Range(
            referenceRange.start.line,
            referenceRange.start.column,
            referenceRange.end.line,
            referenceRange.end.column,
          ),
          options: {
            className: "source-reference-decoration",
          },
        }));

        decorationsRef.current = ed.deltaDecorations(decorationsRef.current, [
          ...referenceDecorations,
          {
            range: focusRange,
            options: {
              className: "source-focus-decoration",
            },
          },
        ]);
        ed.revealRangeInCenterIfOutsideViewport(focusRange);
      } else {
        // 模块级焦点, 不高亮任何区域
        decorationsRef.current = ed.deltaDecorations(decorationsRef.current, []);
      }
    } catch {
      decorationsRef.current = ed.deltaDecorations(decorationsRef.current, []);
    }
  }, [focus, module, getFocusSrcRange]);

  return (
    <div className="source-view">
      <Editor
        height="100%"
        language="llvm"
        value={source}
        onMount={(ed, monaco) => handleEditorMount(ed, monaco, editorRef, monacoRef)}
        options={{
          readOnly: true,
          minimap: { enabled: false },
          fontFamily:
            "Cascadia Code, ui-monospace, SFMono-Regular, Menlo, Monaco, 'Roboto Mono', 'Courier New', monospace",
        }}
      />
    </div>
  );
}