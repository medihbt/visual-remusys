import React from "react";
import { Handle, Position } from "@xyflow/react";
import { TypeIcon } from "./TypeIcon";
import { useIRStore } from "../../ir/ir-state";
import type * as gv from "../guide-view-tree";
import { ChildRow } from "./ChildRow";
import type { GuideNodeCallbacks, GuideRFNodeProp } from "../types";

type GuideNodeProps = GuideRFNodeProp & GuideNodeCallbacks;

export const GuideNodeComp: React.FC<GuideNodeProps> = (props) => {
  let { data, onToggle, onFocus, onRequestMenu } = props;
  if (!data.expanded) return null;

  const nodeTree = data.treeNode;

  const handleFocus = () => {
    console.debug('GuideNodeComp: handleFocus called for', nodeTree.selfId, data.kind, nodeTree.label);
    onFocus(nodeTree.selfId, data.kind, nodeTree.label);
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onRequestMenu(e, nodeTree.selfId, data.kind);
  };

  const focusedId = useIRStore((s) => s.focusedId);
  const focusInfo = useIRStore((s) => s.focusInfo);
  const isFocused = (() => {
    const ref = nodeTree.selfId as gv.TreeNodeRef;
    if (!focusedId && !focusInfo) return false;
    // module-level focus
    if (focusInfo && (focusInfo.id as any).Module && ref.type === "Module") return true;

    // direct id match
    if (focusedId) {
      if ((focusedId as any).Module) {
        if (ref.type === "Module") return true;
      }
      if (ref.type === "GlobalObj" && "Global" in focusedId) {
        if (focusedId.Global === ref.global_id) return true;
      }
      if (ref.type === "Block" && "Block" in focusedId) {
        if (focusedId.Block === ref.block_id) return true;
      }
      if (ref.type === "Inst" && "Inst" in focusedId) {
        if (focusedId.Inst === ref.inst_id) return true;
      }
    }

    // scope-based match: if focused item belongs to a function, highlight the function node
    if (focusInfo && focusInfo.scopeId) {
      if (ref.type === "GlobalObj") {
        // highlight function node when scope matches
        if (focusInfo.scopeId === ref.global_id) return true;
      }
    }

    return false;
  })();

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <Handle type="target" position={Position.Left} style={{ opacity: 0.5 }} />

      <div style={{
        width: "100%", height: "100%",
        border: "1px solid #d1d5db", borderRadius: "4px",
        backgroundColor: "#fff",
        boxShadow: "0 2px 4px rgba(0,0,0,0.05)",
        display: "flex", flexDirection: "column",
        overflow: "hidden", fontFamily: "system-ui, sans-serif"
      }}>
        {/* 顶栏 */}
        <div
          onDoubleClick={handleFocus}
          onContextMenu={handleContextMenu}
          // 单击顶栏也可以聚焦，看需求，这里保留双击
          style={{
            display: "flex", alignItems: "center", padding: "8px 12px",
            backgroundColor: isFocused ? "#eef2ff" : "#f9fafb",
            borderBottom: "1px solid #e5e7eb",
            cursor: "pointer", userSelect: "none"
          }}
        >
          <div style={{ marginRight: "8px", display: "flex", alignItems: "center", justifyContent: "center" }}>
            {isFocused ? (
              <div style={{ borderRadius: 9999, padding: 4, border: "2px solid #60a5fa", display: "inline-flex" }}>
                <TypeIcon kind={data.kind} />
              </div>
            ) : (
              <TypeIcon kind={data.kind} />
            )}
          </div>
          <div style={{ flex: 1, fontSize: "13px", fontWeight: 600, color: "#1f2937", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {nodeTree.label}
          </div>
          <div style={{ fontSize: "16px", color: "#9ca3af", marginLeft: "4px" }}>⋯</div>
        </div>

        {/* 子节点列表 */}
        <div style={{ overflowY: "auto", height: "100%" }}>
          {data.children.map((child, idx) => (
            <ChildRow
              key={child.expanded ? JSON.stringify(child.treeNode.selfId) : `collapsed-${idx}`}
              child={child}
              onToggle={(r) => onToggle("type" in r ? r : r.selfId)}
            />
          ))}
          {data.children.length === 0 && (
            <div style={{ padding: "8px", fontSize: "11px", color: "#9ca3af", textAlign: "center" }}>
              (无子节点)
            </div>
          )}
        </div>
      </div>

      <Handle type="source" position={Position.Right} style={{ opacity: 0.5 }} />
    </div>
  );
};