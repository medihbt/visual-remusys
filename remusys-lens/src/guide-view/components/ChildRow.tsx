import React from "react";
import { TypeIcon } from "./TypeIcon";
import type { Exported } from "../guide-view-tree";
import "./ChildRow.css";

export interface ChildRowProps {
  child: Exported.NodeData;
  onToggle: (ref: Exported.NodeData["treeNode"]) => void;
}

export const ChildRow: React.FC<ChildRowProps> = ({ child, onToggle }) => {
  // eslint-disable-next-line prefer-const
  let { expanded: isExpanded, kind, label } = child;

  if (!label || label.trim() === "") {
    label = "(no name)";
  }
  if (!kind) throw new Error(`Child node ${label} is missing kind information`);

  return (
    <div
      onClick={(e) => {
        e.stopPropagation();
        onToggle(child.treeNode);
      }}
      className={`guide-child-row${isExpanded ? " expanded" : ""}`}
    >
      <div className="guide-child-row__icon">
        <TypeIcon kind={kind} size={16} />
      </div>
      <div className="guide-child-row__label">{label}</div>

      {/* 简单的展开指示器 */}
      <div className="guide-child-row__indicator">
        {isExpanded && <div className="guide-child-row__indicator-inner" />}
      </div>
    </div>
  );
};
