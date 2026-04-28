import { useContext } from "react";
import * as XYFlow from "@xyflow/react";
import { ChildRow } from "./ChildRow";
import { TypeIcon } from "./TypeIcon";
import {
  GuideHandlersContext,
  type GuideRFNodeData,
  type GuideNodeProps,
} from "./GuideContext";

// ---------------------------------------------------------------------------
// GuideViewNode — React Flow 自定义节点（本文件唯一导出的组件）
// ---------------------------------------------------------------------------

export default function GuideViewNode(props: GuideNodeProps) {
  const data = props.data as GuideRFNodeData;
  const handlers = useContext(GuideHandlersContext);
  const isFocused = data.focusClass === "FocusNode";

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <XYFlow.Handle
        type="target"
        position={XYFlow.Position.Left}
        style={{ opacity: 0.5 }}
      />

      <div
        style={{
          width: "100%",
          height: "100%",
          border: "1px solid #d1d5db",
          borderRadius: "4px",
          backgroundColor: "#fff",
          boxShadow: "0 2px 4px rgba(0,0,0,0.05)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        <div
          onDoubleClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handlers.onFocus(data);
          }}
          style={{
            display: "flex",
            alignItems: "center",
            padding: "8px 12px",
            backgroundColor: isFocused ? "#eef2ff" : "#f9fafb",
            borderBottom: "1px solid #e5e7eb",
            cursor: "pointer",
            userSelect: "none",
          }}
        >
          <div
            style={{
              marginRight: "8px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            {isFocused ? (
              <div
                style={{
                  borderRadius: 9999,
                  padding: 4,
                  border: "2px solid #60a5fa",
                  display: "inline-flex",
                }}
              >
                <TypeIcon kind={data.kind} />
              </div>
            ) : (
              <TypeIcon kind={data.kind} />
            )}
          </div>
          <div
            style={{
              flex: 1,
              fontSize: "13px",
              fontWeight: 600,
              color: "#1f2937",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {data.label.trim() === "" ? "(no name)" : data.label}
          </div>
        </div>

        <div style={{ overflowY: "auto", height: "100%" }}>
          {data.children.map((child) => (
            <ChildRow
              key={child.id}
              child={child}
              onToggle={handlers.onToggle}
              onContextMenu={handlers.onRowContextMenu}
            />
          ))}
          {data.children.length === 0 && (
            <div
              style={{
                padding: "8px",
                fontSize: "11px",
                color: "#9ca3af",
                textAlign: "center",
              }}
            >
              (无子节点)
            </div>
          )}
        </div>
      </div>

      <XYFlow.Handle
        type="source"
        position={XYFlow.Position.Right}
        style={{ opacity: 0.5 }}
      />
    </div>
  );
}

