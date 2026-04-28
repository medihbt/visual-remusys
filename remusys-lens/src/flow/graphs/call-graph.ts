import type {
  CallGraphDt,
  CallGraphNodeDt,
  GlobalID,
  IRObjPath,
} from "remusys-wasm";

import type { IRState } from "../../ir/state";
import type { FlowEdge } from "../Edge";
import type { FlowElemNode } from "../Node";
import { dagreLayoutFlow, type FlowGraph } from "./layout";

function isFocusedGlobal(focusPath: IRObjPath, globalID: GlobalID): boolean {
  if (focusPath.length === 0) return false;
  const current = focusPath[focusPath.length - 1];
  return current.type === "Global" && current.value === globalID;
}

function nodeColorByRole(node: CallGraphNodeDt): string {
  switch (node.role) {
    case "Public":
      return "#dbeafe";
    case "Private":
      return "#f3f4f6";
    case "Extern":
      return "#dcfce7";
  }
}

function makeNodes(
  callGraph: CallGraphDt,
  focusPath: IRObjPath,
): FlowElemNode[] {
  return callGraph.nodes.map((node) => ({
    id: node.id,
    type: "elemNode",
    position: { x: 0, y: 0 },
    width: 180,
    height: 50,
    data: {
      label: node.label,
      focused: isFocusedGlobal(focusPath, node.id),
      irObjID: { type: "Global", value: node.id },
      bgColor: nodeColorByRole(node),
    },
  }));
}

function makeEdges(callGraph: CallGraphDt): FlowEdge[] {
  return callGraph.edges.map((edge, index) => ({
    id: `cg:${edge.from}->${edge.to}:${index}`,
    source: edge.from,
    target: edge.to,
    type: "FlowEdge",
    label: "",
    style: {
      stroke: "#334155",
      strokeWidth: 1.2,
    },
    data: {
      path: "",
      labelPosition: { x: 0, y: 0 },
      isFocused: false,
      // 对调用边双击时直接聚焦到被调函数。
      irObjID: { type: "Global", value: edge.to },
    },
  }));
}

export function getCallGraph(irState: IRState): FlowGraph {
  const callGraph = irState.getModule().get_call_graph();
  const nodes = makeNodes(callGraph, irState.focus);
  const edges = makeEdges(callGraph);
  dagreLayoutFlow(nodes, edges);
  return { nodes, edges };
}
