import React from "react";
import type { SourceTy } from "./ir/ir";

export interface FileLoaderProps {
  onLoad: (mode: SourceTy, text: string) => void;
}

export const FileLoader: React.FC<FileLoaderProps> = ({ onLoad }) => {
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);

  const openFilePicker = React.useCallback(() => {
    if (!fileInputRef.current) return;
    fileInputRef.current.value = "";
    fileInputRef.current.click();
  }, []);

  const handleFile = React.useCallback((file: File) => {
    const name = file.name || "";
    const lower = name.toLowerCase();
    let mode: SourceTy;
    if (lower.endsWith(".ll") || lower.endsWith(".ir") || lower.endsWith(".remusys-ir")) {
      mode = "ir";
    } else if (lower.endsWith(".sy") || lower.endsWith(".sysy")) {
      mode = "sysy";
    } else {
      alert(`文件类型错误：期望 *.ll, *.ir, *.remusys-ir 或 *.sy, 得到的文件名是 ${name}`);
      return;
    }

    const reader = new FileReader();
    reader.onload = () => {
      const text = String(reader.result ?? "");
      onLoad(mode!, text);
    };
    reader.onerror = () => {
      alert(`无法读取文件 ${name}`);
    };
    reader.readAsText(file);
  }, [onLoad]);

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
        if (f) handleFile(f);
      }}
    >
      <input
        ref={fileInputRef}
        type="file"
        style={{ display: "none" }}
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) handleFile(f);
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
