import { createContext } from "react";
import type { GuideNodeData, GuideNodeExpand } from "remusys-wasm";
import { isGuideNodeExpand } from "./guide-view-tree";
import * as XYFlow from "@xyflow/react";
import * as Dagre from "dagre";

// ---------------------------------------------------------------------------
// Handler Context — 回调不再塞进 React Flow node data，改为通过 Context 传递
// ---------------------------------------------------------------------------

export type GuideNodeHandlers = {
  onFocus(node: GuideNodeData): void;
  onToggle(node: GuideNodeData): void;
  onRowContextMenu(
    event: React.MouseEvent<HTMLDivElement>,
    rowNode: GuideNodeData,
  ): void;
};

const noopHandlers: GuideNodeHandlers = {
  onFocus: () => {},
  onToggle: () => {},
  onRowContextMenu: () => {},
};

export const GuideHandlersContext =
  createContext<GuideNodeHandlers>(noopHandlers);

// ---------------------------------------------------------------------------
// React Flow 节点类型（纯 WASM 数据，可序列化）
// ---------------------------------------------------------------------------

export type GuideRFNodeData = GuideNodeExpand;
export type GuideRFNode = XYFlow.Node<GuideRFNodeData, "GuideNode">;
export type GuideNodeProps = XYFlow.NodeProps<GuideRFNode>;

// ---------------------------------------------------------------------------
// 尺寸计算
// ---------------------------------------------------------------------------

function guideNodeSize(data: GuideNodeExpand): {
  width: number;
  height: number;
} {
  const header_height = 52;
  const item_height = 40;
  const width = 240;
  const max_height = 300;
  const min_height = header_height + item_height;
  const estimated = header_height + data.children.length * item_height;
  const height = Math.max(min_height, Math.min(estimated, max_height));
  return { width, height };
}

// ---------------------------------------------------------------------------
// collectGuideTree — 纯计算
// ---------------------------------------------------------------------------

/**
 * 从 WASM 返回的展开树根递归构建 React Flow nodes 和 edges。
 *
 * 这是纯函数——不访问 ref、不调用 WASM、不修改任何外部状态。
 * 回调通过 Context 传递给 GuideViewNode 组件，而非在此处塞入 data。
 */
export function collectGuideTree(
  root: GuideNodeData,
): [GuideRFNode[], XYFlow.Edge[]] {
  const nodes: GuideRFNode[] = [];
  const edges: XYFlow.Edge[] = [];

  function dfs(node: GuideNodeData) {
    if (!isGuideNodeExpand(node)) {
      // 收起态子项渲染在父卡片内部，不作为独立 React Flow node
      return;
    }
    const { width, height } = guideNodeSize(node);
    nodes.push({
      id: node.id,
      type: "GuideNode",
      data: node, // 纯 WASM 数据，不含回调
      position: { x: 0, y: 0 },
      width,
      height,
    });
    for (const child of node.children) {
      if (!child.children) {
        // 收起态子项 —— 留在父卡片内渲染
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

// ---------------------------------------------------------------------------
// dagre 布局（collectGuideTree 内部使用）
// ---------------------------------------------------------------------------

function dagreLayoutGuideTree(nodes: GuideRFNode[], edges: XYFlow.Edge[]) {
  const g = new Dagre.graphlib.Graph();
  g.setGraph({ rankdir: "LR", nodesep: 24, ranksep: 56 });
  g.setDefaultEdgeLabel(() => ({}));

  for (const node of nodes) {
    g.setNode(node.id, {
      width: node.width || 240,
      height: node.height || 52,
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
