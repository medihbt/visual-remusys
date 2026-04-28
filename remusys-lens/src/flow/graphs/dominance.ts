import type {
  BlockID,
  DomTreeEdge,
  DomTreeNode,
  GlobalID,
  IRObjPath,
} from "remusys-wasm";

import type { IRState } from "../../ir/state";
import type { FlowEdge } from "../Edge";
import type { FlowElemNode } from "../Node";
import { dagreLayoutFlow, type FlowGraph } from "./layout";

function focusedBlock(focusPath: IRObjPath): BlockID | null {
  const current = focusPath[focusPath.length - 1];
  if (!current || current.type !== "Block") return null;
  return current.value;
}

function makeNodes(
  nodes: DomTreeNode[],
  focusBlock: BlockID | null,
): FlowElemNode[] {
  return nodes.map((node) => ({
    id: node.id,
    type: "elemNode",
    position: { x: 0, y: 0 },
    width: 160,
    height: 50,
    data: {
      label: node.label,
      focused: node.id === focusBlock,
      irObjID: { type: "Block", value: node.id },
      bgColor: node.id === focusBlock ? "#dbeafe" : "#ffffff",
    },
  }));
}

function makeEdges(edges: DomTreeEdge[]): FlowEdge[] {
  return edges.map((edge, index) => ({
    id: `dom:${edge.from}->${edge.to}:${index}`,
    source: edge.from,
    target: edge.to,
    type: "FlowEdge",
    label: "",
    style: {
      stroke: "#475569",
      strokeWidth: 1.2,
    },
    data: {
      path: "",
      labelPosition: { x: 0, y: 0 },
      isFocused: false,
    },
  }));
}

export function getFuncDominance(
  irState: IRState,
  funcID: GlobalID,
): FlowGraph {
  const dom = irState.getModule().get_func_dom_tree(funcID);
  const focusBlockID = focusedBlock(irState.focus);
  const nodes = makeNodes(dom.nodes, focusBlockID);
  const edges = makeEdges(dom.edges);
  dagreLayoutFlow(nodes, edges);
  return { nodes, edges };
}
