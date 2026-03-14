import type { Node as RFNode, NodeProps } from "@xyflow/react";
import { Handle, Position } from "@xyflow/react";
import type { SourceTrackable } from "../../ir/ir";
import type React from "react";
import type { ReactNode } from "react";

export type FlowElemNodeData = {
  label: string | ReactNode;
  focused: boolean;
  irObjID: SourceTrackable | null;
  bgColor: string;
};
export type FlowGroupNodeData = FlowElemNodeData & {
  childIds: string[];
};

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
  // 圆角 3px，阴影使用 2px 的模糊半径（HTML 节点使用 CSS 样式）
  borderRadius: 3,
  boxShadow: "0 0 2px rgba(0,0,0,0.06)",
  overflow: "hidden",
};
const selectedNodeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "1px solid #404040",
  borderRadius: 3,
  overflow: "hidden",
  boxShadow: "0 0 2px rgba(0,0,0,0.12)",
  fontWeight: "bolder",
};

function nodeStyle(selected: boolean, color: string): React.CSSProperties {
  const node = selected ? selectedNodeStyle : baseNodeStyle;
  return Object.assign({ backgroundColor: color }, node);
}

export const FlowElemNodeComp: React.FC<FlowElemNodeProps> = (props) => {
  const selected = props.data?.focused ?? false;
  const color = props.data?.bgColor ?? "#e0e0e0";

  let labelElement: ReactNode = <div>unnamed node</div>;
  if (props.data) {
    const label = props.data.label;
    if (typeof label === "string") labelElement = <div>{label}</div>;
    else labelElement = label;
  }
  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <Handle type="source" position={Position.Top} />
      <div style={nodeStyle(selected, color)}>{labelElement}</div>
      <Handle type="target" position={Position.Bottom} />
    </div>
  );
};

const groupNodeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "2px dashed #c7c7c7",
  borderRadius: 6,
  backgroundColor: "rgba(240,240,240,0.6)",
  padding: 6,
  boxSizing: "border-box",
  overflow: "hidden",
};
export const FlowGroupNodeComp: React.FC<FlowGroupNodeProps> = (props) => {
  let labelElement: ReactNode | null = null;
  if (props.data) {
    const label = props.data.label;
    if (typeof label === "string") labelElement = <div>{label}</div>;
    else labelElement = label;
  }
  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <Handle type="source" position={Position.Top} />
      <div style={groupNodeStyle}>{labelElement}</div>
      <Handle type="target" position={Position.Bottom} />
    </div>
  );
};
export const FlowNodeTypes = {
  elemNode: FlowElemNodeComp,
  groupNode: FlowGroupNodeComp,
};
