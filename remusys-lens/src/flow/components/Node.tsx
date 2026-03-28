import type { Node as RFNode, NodeProps } from "@xyflow/react";
import { Handle, Position } from "@xyflow/react";
import type { SourceTrackable } from "../../ir/ir";
import type React from "react";
import type { ReactElement, ReactNode } from "react";

// Base data type for all flow nodes
export type FlowNodeBaseData = {
  label: string | ReactElement;
  focused: boolean;
  dashed?: boolean;
  irObjID: SourceTrackable | null;
  bgColor: string;
};

// Element node data (for leaf nodes)
export type FlowElemNodeData = FlowNodeBaseData;

// Group node data (for section containers)
// Note: child relationships are managed via React Flow's parentId mechanism
export type FlowGroupNodeData = FlowNodeBaseData;

// Node type definitions
export type FlowElemNode = RFNode<FlowElemNodeData, "elemNode">;
export type FlowGroupNode = RFNode<FlowGroupNodeData, "groupNode">;
export type FlowNode = FlowElemNode | FlowGroupNode;

// Props type definitions
export type FlowElemNodeProps = NodeProps<FlowElemNode>;
export type FlowGroupNodeProps = NodeProps<FlowGroupNode>;
export type FlowNodeProps = NodeProps<FlowNode>;

// Style constants
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

/**
 * Element node component for leaf nodes in the flow.
 * Can be used as a child node within a group when parentId is set.
 */
export const FlowElemNodeComp: React.FC<FlowElemNodeProps> = (props) => {
  const selected = props.data?.focused ?? false;
  const color = props.data?.bgColor ?? "#e0e0e0";

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      {/* Top handle for incoming connections */}
      <Handle type="target" position={Position.Top} />
      <div style={nodeStyle(selected, color)}>
        {props.data?.label ?? "unnamed node"}
      </div>
      {/* Bottom handle for outgoing connections */}
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
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
      {/* Group nodes typically don't have handles since they're containers */}
      {labelElement}
    </div>
  );
};

/**
 * Node type registry for React Flow
 */
export const FlowNodeTypes = {
  elemNode: FlowElemNodeComp,
  groupNode: FlowGroupNodeComp,
};
