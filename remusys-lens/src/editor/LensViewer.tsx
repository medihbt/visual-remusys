import Editor from '@monaco-editor/react'
import monarchLanguage from './llvmMonarch'

function handleEditorMount(editor: any, monaco: any) {
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
}

export type LensViewerProps = {
  irText: string
}

export default function LensViewer({ irText }: LensViewerProps) {
  return (
    <Editor
      height="100%" language="llvm"
      value={irText}
      onMount={handleEditorMount}
      options={{
        readOnly: true,
        minimap: { enabled: false },
        fontFamily: "Cascadia Code, ui-monospace, SFMono-Regular, Menlo, Monaco, 'Roboto Mono', 'Courier New', monospace"
      }}
    />
  )
}
