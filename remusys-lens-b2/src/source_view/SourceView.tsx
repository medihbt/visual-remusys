import { useEffect, useRef } from "react";
import Editor, { type Monaco } from "@monaco-editor/react";
import { editor } from "monaco-editor";

import { useIRStore } from "../ir/state";
import llvmMonarch from "./llvmMonarch.ts";
import "./SourceView.css";

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
      const rangeDt = getFocusSrcRange();
      const range = new monaco.Range(
        rangeDt.start.line,
        rangeDt.start.column,
        rangeDt.end.line,
        rangeDt.end.column,
      );
      decorationsRef.current = ed.deltaDecorations(decorationsRef.current, [
        {
          range,
          options: {
            className: "source-focus-decoration",
          },
        },
      ]);
      ed.revealRangeInCenterIfOutsideViewport(range);
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