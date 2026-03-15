import type { GlobalID, UseID, UseKind, CallNodeRole } from "../../ir/ir";
import { ModuleCache } from "../../ir/ir-state";
import type { FlowEdge } from "../components/Edge";
import type { FlowElemNode, FlowNode } from "../components/Node";
import { layoutSimpleFlow } from "./layout";
import React from "react";

export type CallGraphNode = {
  id: GlobalID;
  role: CallNodeRole;
  name: string; // 节点的显示名称，格式为 @${globalObj.name}
};
export type CallGraphEdge = {
  id: UseID;
  caller: GlobalID;
  callee: GlobalID;
  kind?: UseKind; // 可选的 kind 字段
};

// 根据 role 获取节点背景色
function getNodeBgColor(role: CallNodeRole): string {
  switch (role) {
    case "Root":
      return "#ffeef0"; // 浅粉红色
    case "Live":
      return "#e8f7e8"; // 浅粉绿色
    case "Indirect":
      return "#e0f7fb"; // 浅粉青色
    case "Unreachable":
      return "#f0f0f0"; // 浅灰色
    default:
      return "#ffffff";
  }
}

// 根据 role 获取文字颜色
function getTextColor(role: CallNodeRole): string {
  if (role === "Unreachable") {
    return "#9e9e9e"; // 灰色文字
  }
  return "#000000"; // 黑色文字
}

export function makeCallGraph(
  module: ModuleCache,
): [CallGraphNode[], CallGraphEdge[]] {
  const cg = module.makeCallGraph();

  // 转换节点：为每个节点获取名称
  const nodes: CallGraphNode[] = cg.nodes.map((node) => {
    const globalObj = module.loadGlobal(node.id);
    const name = globalObj.name ? `@${globalObj.name}` : node.id;
    return {
      id: node.id,
      role: node.role,
      name,
    };
  });

  // 转换边：为每条边获取 use kind 信息
  const edges: CallGraphEdge[] = cg.edges.map((edge) => {
    // 尝试加载 use 对象以获取 kind
    try {
      const useObj = module.loadUse(edge.id);
      return {
        ...edge,
        kind: useObj.kind,
      };
    } catch {
      return edge; // 如果无法加载，返回原始边
    }
  });

  return [nodes, edges];
}

export async function renderCallGraphToFlow(
  nodes: CallGraphNode[],
  edges: CallGraphEdge[],
  focusNode: GlobalID | null,
  focusEdge: UseID | null,
): Promise<[FlowNode[], FlowEdge[]]> {
  const flowNodes: FlowElemNode[] = nodes.map((node) => {
    const isFocused = node.id === focusNode;
    const bgColor = getNodeBgColor(node.role);
    const textColor = getTextColor(node.role);

    // 创建带颜色的标签
    const label = React.createElement(
      "div",
      {
        style: {
          color: textColor,
          textAlign: "center",
          fontWeight: "normal",
        },
      },
      node.name,
    );

    return {
      id: node.id as string,
      position: { x: 0, y: 0 },
      type: "elemNode",
      data: {
        label,
        focused: isFocused,
        irObjID: { type: "Global", value: node.id },
        bgColor,
      },
      width: 120,
      height: 45,
    };
  });

  const flowEdges: FlowEdge[] = edges.map((edge) => {
    const isFocused = edge.id === focusEdge;
    return {
      id: edge.id as string,
      source: edge.caller as string,
      target: edge.callee as string,
      type: "flowEdge",
      data: {
        mainPaths: [],
        arrowPaths: [],
        labelX: 0,
        labelY: 0,
        label: edge.kind || "", // 显示 use kind（如果有）
        irObjID: { type: "Use", value: edge.id },
        strokeColor: "#222",
        isFocused,
      },
    };
  });

  return layoutSimpleFlow(flowNodes, flowEdges);
}

export async function renderCallGraph(
  module: ModuleCache,
  focusNode: GlobalID | null,
  focusEdge: UseID | null,
): Promise<[FlowNode[], FlowEdge[]]> {
  const [nodes, edges] = makeCallGraph(module);
  return await renderCallGraphToFlow(nodes, edges, focusNode, focusEdge);
}
