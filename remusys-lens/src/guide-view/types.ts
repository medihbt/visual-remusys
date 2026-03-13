import type { Node as RFNode, Edge as RFEdge, NodeProps } from "@xyflow/react";
import type { Exported, TreeNodeKind } from "./guide-view-tree";
import type { BlockID, GlobalID, ValueDt, SourceTrackable } from "../ir/ir";

export type GuideNodeData = Exported.NodeData;
export type GuideRFNode = RFNode<GuideNodeData, "guideNode">;
export type GuideRFNodeProp = NodeProps<GuideRFNode>;
export type GuideRFEdge = RFEdge;

/** Available on all IR nodes */
export interface FocusEvent {
  type: "Focus"; nodeRef: SourceTrackable; kind: TreeNodeKind; label: string;
};
// Available on all IR nodes
export interface ExpandOneEvent {
  type: "ExpandOne"; nodeRef: SourceTrackable; kind: TreeNodeKind;
}
// Available on all IR nodes
export interface ExpandAllEvent {
  type: "ExpandAll"; nodeRef: SourceTrackable; kind: TreeNodeKind;
}
// Available on all IR nodes
export interface CollapseEvent {
  type: "Collapse"; nodeRef: SourceTrackable; kind: TreeNodeKind;
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
export interface ShowValueDefUse {
  type: "ShowValueDefUse"; valueID: ValueDt;
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
  | ShowValueDefUse
  ;
export type NavEventKind = NavEvent["type"];

// GuideNodeComp 需要的回调 props
export interface GuideNodeCallbacks {
  onToggle: (ref: SourceTrackable) => void;
  onFocus: (ref: SourceTrackable, kind: TreeNodeKind, label: string) => void;
  onRequestMenu: (e: React.MouseEvent, ref: SourceTrackable, kind: TreeNodeKind) => void;
}