import type { Node as RFNode, Edge as RFEdge, NodeProps } from "@xyflow/react";
import type { Exported, TreeNodeRef, TreeNodeKind } from "./guide-view-tree";
import type { BlockID, GlobalID } from "../ir/ir";

export type GuideNodeData = Exported.NodeData;
export type GuideRFNode = RFNode<GuideNodeData, "guideNode">;
export type GuideRFNodeProp = NodeProps<GuideRFNode>;
export type GuideRFEdge = RFEdge;

/** Available on all IR nodes */
export interface FocusEvent {
  type: "Focus"; nodeRef: TreeNodeRef; kind: TreeNodeKind; label: string;
};
// Available on all IR nodes
export interface ExpandOneEvent {
  type: "ExpandOne"; nodeRef: TreeNodeRef; kind: TreeNodeKind;
}
// Available on all IR nodes
export interface ExpandAllEvent {
  type: "ExpandAll"; nodeRef: TreeNodeRef; kind: TreeNodeKind;
}
// Available on all IR nodes
export interface CollapseEvent {
  type: "Collapse"; nodeRef: TreeNodeRef; kind: TreeNodeKind;
};
// Available on nodes with type "Func"
export interface ShowCfgEvent {
  type: "ShowCfg"; funcDef: GlobalID;
};
// Available on nodes with type "Func"
export interface ShowDominanceEvent {
  type: "ShowDominance"; funcDef: GlobalID;
};
// Available on nodes with type "Block"
export interface ShowDfgEvent {
  type: "ShowDfg"; blockID: BlockID;
};

// 仅用于通知外部视图（如编辑器）的数据事件
export type NavEvent =
  | FocusEvent
  | ExpandOneEvent
  | ExpandAllEvent
  | CollapseEvent
  | ShowCfgEvent
  | ShowDominanceEvent
  | ShowDfgEvent
  ;
export type NavEventKind = NavEvent["type"];

// GuideNodeComp 需要的回调 props
export interface GuideNodeCallbacks {
  onToggle: (ref: TreeNodeRef) => void;
  onFocus: (ref: TreeNodeRef, kind: TreeNodeKind, label: string) => void;
  onRequestMenu: (e: React.MouseEvent, ref: TreeNodeRef, kind: TreeNodeKind) => void;
}