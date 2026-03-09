import { makeDominatorTree, type DomTreeDt, type GlobalID } from "../ir/ir";
import { useIRStore } from "../ir/ir-state";
import type { FlowEdge } from "./components/Edge";
import type { FlowNode } from "./components/Node";
import { layoutFlow } from "./layout";

export async function renderDominatiorTree(dominance: DomTreeDt): Promise<[FlowNode[], FlowEdge[]]> {
  const module = useIRStore(state => state.module);
  const focusedBlock = useIRStore(state => {
    let focusInfo = state.focusInfo;
    let id = focusInfo?.id;
    if (!id)
      return null;
    if ("Block" in id)
      return id.Block;
    if ("Inst" in id) {
      let inst = state.module?.loadInst(id.Inst);
      if (!inst)
        return null;
      return inst.parent;
    }
    return null;
  });
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
        focused: node === focusedBlock,
        irObjID: { Block: node },
        bgColor: "#e0e0e0",
      },
      type: "flowNode",
    }
  });
  const flowEdges: FlowEdge[] = edges.map(edge => ({
    id: `${edge[0]}->${edge[1]}`,
    source: edge[0],
    target: edge[1],
  }));
  return await layoutFlow(flowNodes, flowEdges);
}

export async function renderDominanceOfFunc(func: GlobalID): Promise<[FlowNode[], FlowEdge[]] | null> {
  const module = useIRStore(state => state.module);
  if (module === null) {
    throw new Error("No module loaded");
  }
  let dominance = makeDominatorTree(module.moduleId, func);
  if (!dominance) {
    return null;
  }
  return await renderDominatiorTree(dominance);
}
