import React from "react";
import type { IRTreeNodeClass } from "../ir/types";

type TypeIconStyle = {
    color: string;
    text: string;
    textColor: string;
}

const KIND_CONFIG: Record<IRTreeNodeClass, TypeIconStyle> = {
    Module: { color: "#ef4444", text: "M", textColor: "white" },
    GlobalVar: { color: "#242480", text: "Gv", textColor: "white" },
    Func: { color: "#fbbf24", text: "Fx", textColor: "black" },
    ExternFunc: { color: "#6b7280", text: "Fx", textColor: "white" },
    Block: { color: "#f97316", text: "B", textColor: "white" },
    NormalInst: { color: "#22c55e", text: "I", textColor: "black" },
    PhiInst: { color: "#38bdf8", text: "Φ", textColor: "black" },
    TerminatorInst: { color: "#f97316", text: "Ti", textColor: "white" },
    Use: { color: "#8b5cf6", text: "U", textColor: "white" },
    JumpTarget: { color: "#ec4899", text: "Jt", textColor: "white" },
    FuncArg: { color: "#fbbf24", text: "Arg", textColor: "black" },
};

export type TypeIconProps = {
    kind: IRTreeNodeClass;
    size?: number;
}

export const TypeIcon: React.FC<TypeIconProps> = ({ kind, size = 16 }) => {
    const config = KIND_CONFIG[kind] || { color: "#6b7280", text: "?", textColor: "white" };

    return (
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} style={{ flexShrink: 0 }}>
            <circle cx="8" cy="8" r="8" fill={config.color} />
            <text
                x="8" y="10"
                textAnchor="middle"
                fill={config.textColor}
                fontSize="9"
                fontFamily='"Cascadia Mono", monospace'
            >
                {config.text}
            </text>
        </svg>
    );
};