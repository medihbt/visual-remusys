import React from "react";
import type { SourceTy } from "remusys-wasm";
import { handleFileLoad } from "./file-load";

export interface FileLoaderProps {
  onLoad: (mode: SourceTy, text: string, filename: string) => void;
}

export const FileLoader: React.FC<FileLoaderProps> = ({ onLoad }) => {
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);

  const openFilePicker = React.useCallback(() => {
    if (!fileInputRef.current) return;
    fileInputRef.current.value = "";
    fileInputRef.current.click();
  }, []);

  return (
    <div
      style={{
        width: "100vw",
        height: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "#fafafa",
        boxSizing: "border-box",
      }}
      onDragOver={(e) => { e.preventDefault(); }}
      onDrop={(e) => {
        e.preventDefault();
        const f = e.dataTransfer?.files?.[0];
        if (f) handleFileLoad(f, onLoad);
      }}
    >
      <input
        ref={fileInputRef}
        type="file"
        style={{ display: "none" }}
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) handleFileLoad(f, onLoad);
        }}
      />

      <div
        onClick={openFilePicker}
        style={{
          width: 520,
          maxWidth: "90%",
          minHeight: 220,
          borderRadius: 12,
          border: "2px dashed #cbd5e1",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          textAlign: "center",
          padding: 24,
          cursor: "pointer",
          background: "#fff",
        }}
      >
        <div style={{ color: "#374151", fontSize: 16 }}>
          拖动上传源文件：<br />
          <span style={{ color: "#6b7280", fontSize: 13 }}>*remusys-ir 或者 *.sysy</span>
        </div>
      </div>
    </div>
  );
};

export default FileLoader;
