import React, { useRef, useEffect } from 'react'
import Editor from '@monaco-editor/react'
import monarchLanguage from './llvmMonarch'
import { useIRStore } from '../ir/ir-state'
import './LensViewer.css'

function handleEditorMount(editor: any, monaco: any, editorRef: React.MutableRefObject<any>, monacoRef: React.MutableRefObject<any>) {
  try {
    const exists = monaco.languages.getLanguages().some((l: any) => l.id === 'llvm')
    if (!exists) monaco.languages.register({ id: 'llvm' })
    monaco.languages.setMonarchTokensProvider('llvm', monarchLanguage)
    // ensure current model uses the llvm language
    const model = editor.getModel()
    if (model) monaco.editor.setModelLanguage(model, 'llvm')
  } catch (e) {
    // swallow - editor still works without custom language
    console.warn('Failed to register llvm monarch', e)
  }
  editorRef.current = editor
  monacoRef.current = monaco
}

export type LensViewerProps = {
  irText: string
}

export default function LensViewer({ irText }: LensViewerProps) {
  const focusInfo = useIRStore(s => s.focusInfo)
  const moduleOverview = useIRStore(s => s.module?.brief.overview_src ?? s.sourceText)
  const editorRef = useRef<any>(null)
  const monacoRef = useRef<any>(null)
  const decRef = useRef<string[]>([])

  useEffect(() => {
    const editor = editorRef.current
    const monaco = monacoRef.current
    if (!editor || !monaco) return

    const model = editor.getModel()
    if (!focusInfo) {
      // clear decorations and restore module overview source
      if (decRef.current.length > 0) {
        decRef.current = editor.deltaDecorations(decRef.current, [])
      }
      if (model && model.getValue() !== moduleOverview) {
        model.setValue(moduleOverview)
      }
      return
    }

    // set model text to focus source
    if (model && model.getValue() !== focusInfo.sourceText) {
      model.setValue(focusInfo.sourceText)
    }

    // create decoration for highlightLoc
    const begin = focusInfo.highlightLoc.begin
    const end = focusInfo.highlightLoc.end
    try {
      const range = new monaco.Range(begin.line, begin.column, end.line, end.column)
      decRef.current = editor.deltaDecorations(decRef.current, [{
        range,
        options: {
          className: 'ir-focus-decoration',
        }
      }])
      editor.revealRange(range)
    } catch (e) {
      console.warn('LensViewer: failed to apply focus decoration', e)
    }
  }, [focusInfo, irText])

  return (
    <Editor
      height="100%" language="llvm"
      value={irText}
      onMount={(editor, monaco) => handleEditorMount(editor, monaco, editorRef, monacoRef)}
      options={{
        readOnly: true,
        minimap: { enabled: false },
        fontFamily: "Cascadia Code, ui-monospace, SFMono-Regular, Menlo, Monaco, 'Roboto Mono', 'Courier New', monospace"
      }}
    />
  )
}
