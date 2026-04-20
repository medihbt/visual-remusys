import { TypeIcon } from "./TypeIcon";
import "./ChildRow.css";
import { guideNodeExpanded } from "./Node";
import type { GuideNodeData } from "remusys-wasm-b2";

export type ChildRowProps = {
    child: GuideNodeData;
    onToggle: (node: GuideNodeData) => void;
}

export function ChildRow({ child, onToggle }: ChildRowProps) {
    let { label, kind } = child;
    let isExpanded = guideNodeExpanded(child);

    if (label.trim() === "") {
        label = "(no name)";
    }
    return (
        <div
            onClick={(e) => {
                e.stopPropagation();
                onToggle(child);
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
    )
}
