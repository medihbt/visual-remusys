import { useCallback, useEffect, useRef } from "react";
import Editor, { type Monaco } from "@monaco-editor/react";
import { editor, KeyCode } from "monaco-editor";

import { useIRStore } from "../ir/state";
import llvmMonarch from "./llvmMonarch.ts";
import "./SourceView.css";
import {
  IRTreeCursor,
  ModuleInfo,
  type IRObjPath,
  type MonacoSrcRange,
} from "remusys-wasm";
import { RenameInputWidget, type RenameInputState } from "./RenameInput.tsx";

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

function getReferenceSourceRanges(
  module: ModuleInfo,
  focus: IRObjPath,
): MonacoSrcRange[] {
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
  const irStore = useIRStore();

  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const decorationsRef = useRef<string[]>([]);
  const renameWidgetRef = useRef<RenameInputWidget | null>(null);

  const clearDecorations = useCallback(() => {
    const ed = editorRef.current;
    if (ed) {
      decorationsRef.current = ed.deltaDecorations(decorationsRef.current, []);
    }
  }, [editorRef, decorationsRef]);

  /// 通知整个 App 把焦点放在选中的位置. 如果没有选中的对象，则放在光标位置.
  /// 该函数不需要直接调整编辑器的高亮等信息，只需要通知 App 进行调整即可, 剩下的
  /// 工作由 React 的数据驱动组件来完成.
  const focusOnSelection = useCallback(() => {
    const ed = editorRef.current;
    const module = irStore.getModule();
    if (!ed || !module) {
      return;
    }

    const selection = ed.getSelection();
    const position = selection?.getStartPosition() ?? ed.getPosition();
    if (!position) {
      return;
    }
    console.log("SourceView: focusing on position", position);

    try {
      const path = module.path_of_srcpos({
        line: position.lineNumber,
        column: position.column,
      });
      console.log("SourceView: focusing on path", path);
      irStore.setFocus(path);
    } catch (error) {
      console.warn("SourceView: failed to focus from selection", error);
    }
  }, [editorRef, irStore]);

  const openRenameInput = useCallback(() => {
    const ed = editorRef.current;
    const module = irStore.getModule();
    if (!ed || !module) {
      return;
    }

    const selection = ed.getSelection();
    const position = selection?.getStartPosition() ?? ed.getPosition();
    if (!position) {
      return;
    }

    try {
      const path = module.path_of_srcpos({
        line: position.lineNumber,
        column: position.column,
      });
      const node = module.path_get_node(path);
      const target = path[path.length - 1];
      const identityName = module.get_object_identity_name(target);
      let initialValue: string;
      switch (identityName.type) {
        case "Module":
        case "Global":
        case "Local":
        case "UseGlobal":
        case "UseLocal":
          initialValue = identityName.value;
          break;
        case "NotNameable":
          alert("无法重命名该对象");
          return;
        default:
          initialValue = "";
          break;
      }

      renameWidgetRef.current?.show(
        {
          lineNumber: node.src_range.start.line,
          column: node.src_range.start.column,
        },
        path,
        initialValue,
      );
    } catch (error) {
      console.warn("SourceView: failed to open rename input", error);
    }
  }, [irStore, editorRef]);

  const addActions = useCallback(
    (ed: editor.IStandaloneCodeEditor) => {
      const focusAction = ed.addAction({
        id: "remusys.focus-on-selection",
        label: "聚焦选区",
        contextMenuGroupId: "navigation",
        contextMenuOrder: 1.5,
        run: () => {
          focusOnSelection();
        },
      });

      const renameAction = ed.addAction({
        id: "remusys.rename-selection",
        label: "重命名",
        contextMenuGroupId: "modification",
        contextMenuOrder: 1.5,
        keybindings: [KeyCode.F2],
        run: () => {
          openRenameInput();
        },
      });

      return () => {
        renameAction.dispose();
        focusAction.dispose();
      };
    },
    [focusOnSelection, openRenameInput],
  );

  const handleRenameSubmit = useCallback(
    (renameState: RenameInputState) => {
      const { path, value } = renameState;
      const res = irStore.rename(path, value);
      switch (res.type) {
        case "Renamed":
        case "NoChange":
          break;
        case "GlobalNameConflict":
        case "LocalNameConflict":
          alert(`重命名失败: 已存在同名对象"`);
          break;
        case "UnnamedObject":
          alert(`重命名失败: 该对象没有名称, 无法重命名`);
          break;
        default:
          alert(`重命名失败: ${res}`);
      }
    },
    [irStore],
  );

  const handleRenameCancel = useCallback(() => {
    // no-op; widget handles its own teardown.
  }, []);

  const handleMount = useCallback(
    (ed: editor.IStandaloneCodeEditor, monaco: Monaco) => {
      handleEditorMount(ed, monaco, editorRef, monacoRef);
      renameWidgetRef.current?.dispose();
      renameWidgetRef.current = new RenameInputWidget(
        ed,
        handleRenameSubmit,
        handleRenameCancel,
      );
      return addActions(ed);
    },
    [addActions, handleRenameCancel, handleRenameSubmit],
  );

  useEffect(() => {
    return () => {
      renameWidgetRef.current?.dispose();
      renameWidgetRef.current = null;
    };
  }, []);

  useEffect(() => {
    const ed = editorRef.current;
    const monaco = monacoRef.current;
    const module = irStore.getModule();
    const focus = irStore.focus;
    if (!ed || !monaco || !module) {
      return;
    }

    try {
      if (focus.length > 1) {
        const rangeDt = irStore.getFocusSrcRange();
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
        clearDecorations();
      }
    } catch {
      clearDecorations();
    }
  }, [irStore, clearDecorations]);

  return (
    <div className="source-view">
      <Editor
        height="100%"
        language="llvm"
        value={irStore.source}
        onMount={handleMount}
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
