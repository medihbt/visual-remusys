import { type Node as RFNode, type NodeProps, Handle, Position } from "@xyflow/react";

import type { IRTreeObjID } from "remusys-wasm-b2";
import type { FlowEdge } from "./Edge";

export type FlowNodeBase = {
  label: string | React.ReactNode;
  focused: boolean;
  irObjID: IRTreeObjID | null;
  bgColor: string;
}
export type FlowElemNodeData = FlowNodeBase;
export type FlowGroupNodeData = FlowNodeBase;

export type FlowElemNode = RFNode<FlowElemNodeData, "elemNode">;
export type FlowGroupNode = RFNode<FlowGroupNodeData, "groupNode">;
export type FlowNode = FlowElemNode | FlowGroupNode;

export type FlowElemNodeProps = NodeProps<FlowElemNode>;
export type FlowGroupNodeProps = NodeProps<FlowGroupNode>;
export type FlowNodeProps = NodeProps<FlowNode>;

const baseNodeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "1px solid #e5e7eb",
  borderRadius: 3,
  boxShadow: "0 0 2px rgba(0,0,0,0.06)",
  overflow: "hidden",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const selectedNodeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "1px solid #404040",
  borderRadius: 3,
  overflow: "hidden",
  boxShadow: "0 0 2px rgba(0,0,0,0.12)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

function nodeStyle(selected: boolean, color: string): React.CSSProperties {
  const node = selected ? selectedNodeStyle : baseNodeStyle;
  return Object.assign({ backgroundColor: color }, node);
}

const ElemNode: React.FC<FlowElemNodeProps> = ({ data }) => {
  const selected = data?.focused || false;
  const color = data?.bgColor || "#e0e0e0";

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      {/* Top handle for incoming connections */}
      <Handle type="target" position={Position.Top} />
      <div style={nodeStyle(selected, color)}>
        {data?.label ?? "unnamed node"}
      </div>
      {/* Bottom handle for outgoing connections */}
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}
const GroupNode: React.FC<FlowGroupNodeProps> = ({ data }) => {
  const selected = data?.focused || false;
  const color = data?.bgColor || "#e0e0e0";

  return (
    <div style={nodeStyle(selected, color)}>
      {data?.label ?? "unnamed group"}
    </div>
  );
}

export const FlowNodeTypes = {
  elemNode: ElemNode,
  groupNode: GroupNode,
}

/** Creates a graph representation of an error for visualization purposes */
export function makeErrorGraph(error: Error): [FlowElemNode[], FlowEdge[]] {
  const backtrace: string[] = error.stack ? error.stack.split("\n") : [];
  const label = (
    <div style={{ padding: "8px", fontFamily: "system-ui, sans-serif" }}>
      <div style={{ fontWeight: "bold", marginBottom: "4px" }}>Error: {error.message}</div>
      <div style={{ fontSize: "12px", color: "#6b7280" }}>
        {backtrace.map((line, idx) => (<div key={idx}>{line}</div>))}
      </div>
    </div>
  );

  const node: FlowElemNode = {
    id: `error-${Date.now()}`,
    type: "elemNode",
    data: {
      label,
      focused: false,
      irObjID: null,
      bgColor: "#ffe5e5",
    },
    position: { x: 0, y: 0 },
    width: 240,
    height: 52,
  };
  return [[node], []];
}