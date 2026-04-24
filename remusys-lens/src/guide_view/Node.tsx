import * as XYFlow from "@xyflow/react";
import * as Dagre from "dagre";
import { ChildRow } from "./ChildRow";
import { TypeIcon } from "./TypeIcon";
import type { GuideNodeData, GuideNodeExpand } from "remusys-wasm";

export function guideNodeExpanded(data: GuideNodeData): boolean {
  return data.children !== undefined;
}
export function guideNodeSize(data: GuideNodeExpand): { width: number; height: number } {
  const header_height = 52;
  const item_height = 40;
  const width = 240;
  const max_height = 300;
  const min_height = header_height + item_height;
  const estimated = header_height + data.children.length * item_height;
  const height = Math.max(min_height, Math.min(estimated, max_height));
  return { width, height };
}

export type GuideNodeHandlers = {
  onFocus(node: GuideNodeData): void;
  onToggle(node: GuideNodeData): void;
  onRowContextMenu(event: React.MouseEvent<HTMLDivElement>, rowNode: GuideNodeData): void;
};

export type GuideNodeVMData = GuideNodeExpand & {
  onFocus(node: GuideNodeData): void;
  onToggle(node: GuideNodeData): void;
  onRowContextMenu(event: React.MouseEvent<HTMLDivElement>, rowNode: GuideNodeData): void;
};

export function collectGuideTree(root: GuideNodeData, handlers: GuideNodeHandlers): [GuideRFNode[], XYFlow.Edge[]] {
  const nodes: GuideRFNode[] = [];
  const edges: XYFlow.Edge[] = [];

  function dfs(node: GuideNodeData) {
    if (!node.children) {
      // node that is not expanded, do not add to nodes list.
      return;
    }
    const { width, height } = guideNodeSize(node);
    nodes.push({
      id: node.id,
      type: "GuideNode",
      data: {
        ...node,
        onFocus: handlers.onFocus,
        onToggle: handlers.onToggle,
        onRowContextMenu: handlers.onRowContextMenu,
      },
      position: { x: 0, y: 0 },
      width,
      height,
    });
    for (const child of node.children) {
      if (!child.children) {
        // Collapsed/menu child items are rendered inside the card, not as flow nodes.
        continue;
      }
      const edgeId = `${node.id}_${child.id}`;
      let dashed: boolean;
      switch (child.focusClass) {
        case "FocusNode":
        case "FocusParent":
        case "FocusScope":
          dashed = true;
          break;
        default:
          dashed = false;
          break;
      }

      edges.push({
        id: edgeId,
        source: node.id,
        target: child.id,
        type: "default",
        // 这样就可以指出来一条指向焦点的路径了
        animated: dashed,
        style: {
          stroke: dashed ? "#60a5fa" : "#d1d5db",
          strokeDasharray: dashed ? "4 2" : undefined,
        },
        markerEnd: { type: "arrowclosed" },
      });
      dfs(child);
    }
  }
  dfs(root);
  dagreLayoutGuideTree(nodes, edges);

  return [nodes, edges];
}

function dagreLayoutGuideTree(nodes: GuideRFNode[], edges: XYFlow.Edge[]) {
  const g = new Dagre.graphlib.Graph();
  g.setGraph({ rankdir: "LR", nodesep: 24, ranksep: 56 });
  g.setDefaultEdgeLabel(() => ({}));

  for (const node of nodes) {
    g.setNode(node.id, {
      width: node.width || 240,
      height: node.height || 52
    });
  }
  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }
  Dagre.layout(g);

  for (const node of nodes) {
    const { x, y } = g.node(node.id);
    const width = node.width || 240;
    const height = node.height || 52;
    node.position = {
      x: x - width / 2,
      y: y - height / 2,
    };
  }
}

export type GuideRFNodeData = GuideNodeVMData;
export type GuideRFNode = XYFlow.Node<GuideRFNodeData, "GuideNode">;
export type GuideNodeProps = XYFlow.NodeProps<GuideRFNode>;

export default function GuideViewNode(props: GuideNodeProps) {
  const data = props.data as GuideNodeVMData;

  function tryFocusNode(node: GuideNodeData) {
    data.onFocus(node);
  }

  function handleToggle(child: GuideNodeData) {
    data.onToggle(child);
  }

  function onRowContextMenu(e: React.MouseEvent<HTMLDivElement>, rowNode: GuideNodeData) {
    data.onRowContextMenu(e, rowNode);
  }

  const isFocused = data.focusClass === "FocusNode";

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <XYFlow.Handle type="target" position={XYFlow.Position.Left} style={{ opacity: 0.5 }} />

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
            tryFocusNode(data);
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
              key={child.id} child={child}
              onToggle={handleToggle}
              onContextMenu={onRowContextMenu}
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

export const guideNodeTypes = { GuideNode: GuideViewNode };