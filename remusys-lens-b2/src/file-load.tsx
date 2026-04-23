import type { SourceTy } from "remusys-wasm-b2";

export function handleFileLoad(file: File, onLoad: (mode: SourceTy, text: string, filename: string) => void) {
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
        onLoad(mode!, text, name);
    };
    reader.onerror = () => {
        alert(`无法读取文件 ${name}`);
    };
    reader.readAsText(file);
}