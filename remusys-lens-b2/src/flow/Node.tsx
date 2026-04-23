import { type Node as RFNode, type NodeProps, Handle, Position } from "@xyflow/react";

import type { IRTreeObjID } from "remusys-wasm-b2";

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

export const GROUP_NODE_TARGET_HANDLE_ID = "group-target";
export const GROUP_NODE_SOURCE_HANDLE_ID = "group-source";

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
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <Handle
        id={GROUP_NODE_TARGET_HANDLE_ID}
        type="target"
        position={Position.Left}
        style={{ opacity: 0, pointerEvents: "none" }}
      />
      <div style={nodeStyle(selected, color)}>
        {data?.label ?? "unnamed group"}
      </div>
      <Handle
        id={GROUP_NODE_SOURCE_HANDLE_ID}
        type="source"
        position={Position.Right}
        style={{ opacity: 0, pointerEvents: "none" }}
      />
    </div>
  );
}

export const FlowNodeTypes = {
  elemNode: ElemNode,
  groupNode: GroupNode,
}
