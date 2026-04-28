import { TypeIcon } from "./TypeIcon";
import "./ChildRow.css";
import type { GuideNodeData } from "remusys-wasm";
import { isGuideNodeExpand } from "./guide-view-tree";

export type ChildRowProps = {
  child: GuideNodeData;
  onToggle: (node: GuideNodeData) => void;
  onContextMenu?: (
    event: React.MouseEvent<HTMLDivElement>,
    node: GuideNodeData,
  ) => void;
};

export function ChildRow(props: ChildRowProps) {
  const { child, onToggle, onContextMenu } = props;
  const kind = child.kind;
  const isExpanded = isGuideNodeExpand(child);
  let label = child.label;

  if (label.trim() === "") {
    label = "(no name)";
  }
  const insideFocusPath = child.focusClass !== "NotFocused";
  return (
    <div
      onClick={(e) => {
        e.stopPropagation();
        onToggle(child);
      }}
      onContextMenu={(e) => {
        e.stopPropagation();
        e.preventDefault();
        if (onContextMenu) onContextMenu(e, child);
      }}
      className={`guide-child-row${isExpanded ? " expanded" : ""}`}
    >
      <div className="guide-child-row__icon">
        <TypeIcon kind={kind} size={16} focused={insideFocusPath} />
      </div>
      <div
        className={
          child.focusClass === "NotFocused"
            ? "guide-child-row__label"
            : "guide-child-row__label_focused"
        }
      >
        {label}
      </div>

      {/* 简单的展开指示器 */}
      <div className="guide-child-row__indicator">
        {isExpanded && <div className="guide-child-row__indicator-inner" />}
      </div>
    </div>
  );
}
