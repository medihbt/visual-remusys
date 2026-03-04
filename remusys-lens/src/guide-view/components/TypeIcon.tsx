import React from "react";
import type { TreeNodeKind } from "../guide-view-tree";

const KIND_CONFIG: Record<TreeNodeKind, { color: string; text: string; textColor: string }> = {
  Module: { color: "#ef4444", text: "M", textColor: "white" },
  GlobalVar: { color: "#242480", text: "Gv", textColor: "white" },
  ExternGlobalVar: { color: "#6b7280", text: "Gv", textColor: "white" },
  Func: { color: "#fbbf24", text: "Fx", textColor: "black" },
  ExternFunc: { color: "#6b7280", text: "Fx", textColor: "white" },
  Block: { color: "#f97316", text: "B", textColor: "white" },
  Inst: { color: "#22c55e", text: "I", textColor: "black" },
  Phi: { color: "#38bdf8", text: "Φ", textColor: "black" },
  Terminator: { color: "#f97316", text: "Ti", textColor: "white" },
};

export interface TypeIconProps {
  kind: TreeNodeKind;
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