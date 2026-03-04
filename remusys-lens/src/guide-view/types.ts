import type { Node as RFNode, Edge as RFEdge, NodeProps } from "@xyflow/react";
import type { Exported, TreeNodeRef, TreeNodeKind } from "./guide-view-tree";

export type GuideNodeData = Exported.NodeData;
export type GuideRFNode = RFNode<GuideNodeData, "guideNode">;
export type GuideRFNodeProp = NodeProps<GuideRFNode>;
export type GuideRFEdge = RFEdge;

// 仅用于通知外部视图（如编辑器）的数据事件
export type NavEvent = 
  | { type: 'FOCUS'; nodeRef: TreeNodeRef; kind: TreeNodeKind; label: string };

// GuideNodeComp 需要的回调 props
export interface GuideNodeCallbacks {
  onToggle: (ref: TreeNodeRef) => void;
  onFocus: (ref: TreeNodeRef, kind: TreeNodeKind, label: string) => void;
  onRequestMenu: (e: React.MouseEvent, ref: TreeNodeRef, kind: TreeNodeKind) => void;
}