import React from "react";
import type { IRTreeNodeClass } from "remusys-wasm-b2";

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
  focused?: boolean;
}

export const TypeIcon: React.FC<TypeIconProps> = ({ kind, size = 16, focused = false }) => {
  const config = KIND_CONFIG[kind] || { color: "#6b7280", text: "?", textColor: "white" };

  let circles;
  if (focused) {
    circles = (
      <>
        <circle cx="8" cy="8" r="7" fill="none" stroke="#000" strokeWidth="1" />
        <circle cx="8" cy="8" r="6" fill={config.color} />
      </>
    );
  } else {
    circles = <circle cx="8" cy="8" r="8" fill={config.color} />;
  }

  return (
    <svg width={size} height={size} viewBox="0 0 16 16" style={{ flexShrink: 0 }}>
      {circles}
      <text
        x="8" y="10"
        textAnchor="middle"
        fill={config.textColor}
        fontSize="9"
        fontFamily='"Cascadia Mono", monospace'
        fontWeight={focused? "bold" : "normal"}
      >
        {config.text}
      </text>
    </svg>
  );
};