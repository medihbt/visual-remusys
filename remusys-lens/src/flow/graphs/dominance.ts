import { makeDominatorTree, type BlockID, type DomTreeDt, type GlobalID } from "../../ir/ir";
import { ModuleCache } from "../../ir/ir-state";
import type { FlowEdge } from "../components/Edge";
import type { FlowNode } from "../components/Node";
import { layoutFlow } from "./layout";

export async function renderDominatiorTree(
  module: ModuleCache, focusBB: BlockID | null, dominance: DomTreeDt
): Promise<[FlowNode[], FlowEdge[]]> {
  if (module === null) {
    throw new Error("No module loaded");
  }
  const { nodes, edges } = dominance;
  const flowNodes: FlowNode[] = nodes.map((node, idx) => {
    let block = module.loadBlock(node);
    return {
      id: node,
      position: { x: 0, y: idx * 100 },
      data: {
        label: block?.name || node,
        focused: node === focusBB,
        irObjID: { type: "Block", value: node },
        bgColor: "#ffffff",
      },
      type: "flowNode",
    }
  });
  const flowEdges: FlowEdge[] = edges.map(edge => ({
    id: `${edge[0]}->${edge[1]}`,
    source: edge[0],
    target: edge[1],
    data: {
      mainPaths: [],
      arrowPaths: [],
      labelX: 0,
      labelY: 0,
      label: "",
    }
  }));
  return await layoutFlow(flowNodes, flowEdges);
}

export async function renderDominanceOfFunc(
  module: ModuleCache,
  focusBB: BlockID | null,
  func: GlobalID
): Promise<[FlowNode[], FlowEdge[]] | null> {
  if (module === null) {
    throw new Error("No module loaded");
  }
  let dominance = makeDominatorTree(module.moduleId, func);
  if (!dominance) {
    return null;
  }
  return await renderDominatiorTree(module, focusBB, dominance);
}
