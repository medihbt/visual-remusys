import type { Node as RFNode, NodeProps } from "@xyflow/react";
import { Handle, Position } from "@xyflow/react";
import type { SourceTrackable } from "../../ir/ir";
import type React from "react";

export type FlowNodeData = {
  label: string;
  focused: boolean;
  irObjID: SourceTrackable | null;
  bgColor: string;
}

export type FlowNode = RFNode<FlowNodeData, "flowNode">;
export type FlowNodeProps = NodeProps<FlowNode>;

const baseNodeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "1px solid #e5e7eb",
  // 圆角 3px，阴影使用 2px 的模糊半径（HTML 节点使用 CSS 样式）
  borderRadius: 3,
  boxShadow: '0 0 2px rgba(0,0,0,0.06)',
  overflow: 'hidden',
};
const selectedNodeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "1px solid #404040",
  borderRadius: 3,
  overflow: 'hidden',
  boxShadow: '0 0 2px rgba(0,0,0,0.12)',
};

export function nodeStyle(selected: boolean, color: string): React.CSSProperties {
  const node = selected ? selectedNodeStyle : baseNodeStyle;
  return Object.assign({ backgroundColor: color }, node);
}
export const FlowNodeComp: React.FC<FlowNodeProps> = (props) => {
  let selected = props.data?.focused ?? false;
  let color = props.data?.bgColor ?? "#e0e0e0";
  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <Handle type="target" position={Position.Top} style={{ opacity: 0.5 }} />
      <div style={nodeStyle(selected, color)}>
        <div style={{ padding: 8, fontSize: 12, color: "#111", textAlign: "center", overflow: "hidden" }}>
          {props.data?.label}
        </div>
      </div>
      <Handle type="source" position={Position.Bottom} style={{ visibility: "hidden" }} />
    </div>
  );
}
export const FlowNodeTypes = {
  flowNode: FlowNodeComp,
}
