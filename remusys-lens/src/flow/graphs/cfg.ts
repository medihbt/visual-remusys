import type { BlockID, GlobalID, JTKind, JumpTargetID } from "../../ir/ir";
import { ModuleCache } from "../../ir/ir-state";
import type { FlowEdge } from "../components/Edge";
import type { FlowElemNode, FlowNode } from "../components/Node";
import { layoutSimpleFlow } from "./layout";

export type CfgNodeKind = "Entry" | "Control" | "Exit";
export type CfgNode = {
  id: BlockID;
  label: string;
  kind: CfgNodeKind;
};
export type CfgEdge = {
  id: JumpTargetID;
  from: BlockID;
  to: BlockID;
  kind: JTKind;
};

const strokeColors = {
  Jump: "#222",
  BrThen: "#16a34a",
  BrElse: "#dc2626",
  SwitchDefault: "#2563eb",
  SwitchCase: "#d97706",
};
function getStrokeColor(kind: JTKind): string {
  const kindSeg0 = kind.split(":")[0];
  return strokeColors[kindSeg0 as keyof typeof strokeColors] ?? "#222";
}

export function makeCfg(
  module: ModuleCache,
  func: GlobalID,
): [CfgNode[], CfgEdge[]] | null {
  const funcDt = module.loadGlobal(func);
  if (funcDt.typeid !== "Func") return null;
  if (!funcDt.blocks) return null;
  const entryNode = funcDt.blocks[0];
  const nodes: CfgNode[] = [
    {
      id: entryNode.id,
      label: entryNode.name ?? entryNode.id,
      kind: "Entry",
    },
  ];
  const edges: CfgEdge[] = module.getBlockSuccessors(entryNode).map((jt) => {
    return { id: jt.id, from: entryNode.id, to: jt.target, kind: jt.kind };
  });

  for (let i = 1; i < funcDt.blocks.length; i++) {
    const block = funcDt.blocks[i];
    const succs = module.getBlockSuccessors(block);
    const kind: CfgNodeKind = succs.length === 0 ? "Exit" : "Control";
    nodes.push({
      id: block.id,
      label: block.name ?? block.id,
      kind,
    });
    for (const jt of succs) {
      const edge: CfgEdge = {
        id: jt.id,
        from: block.id,
        to: jt.target,
        kind: jt.kind,
      };
      edges.push(edge);
    }
  }
  return [nodes, edges];
}

export async function renderCfgToFlow(
  nodes: CfgNode[],
  edges: CfgEdge[],
  focusBlock: BlockID | null,
  focusEdge: JumpTargetID | null,
): Promise<[FlowNode[], FlowEdge[]]> {
  const flowNodes: FlowElemNode[] = nodes.map((n) => {
    let bgColor: string;
    switch (n.kind) {
      case "Entry":
        bgColor = "#d1fae5";
        break;
      case "Exit":
        bgColor = "#fee2e2";
        break;
      default:
        bgColor = "#ffffff";
        break;
    }
    return {
      id: n.id as string,
      position: { x: 0, y: 0 },
      type: "elemNode",
      data: {
        label: n.label,
        focused: n.id === focusBlock,
        irObjID: { type: "Block", value: n.id },
        bgColor: bgColor,
      },
      width: 120,
      height: 45,
    };
  });
  const flowEdges: FlowEdge[] = edges.map((e) => {
    const isSelected = e.id === focusEdge;
    return {
      id: e.id as string,
      source: e.from as string,
      target: e.to as string,
      type: "flowEdge",
      data: {
        mainPaths: [],
        arrowPaths: [],
        labelX: 0,
        labelY: 0,
        label: e.kind,
        irObjID: { type: "JumpTarget", value: e.id },
        strokeColor: getStrokeColor(e.kind),
        isFocused: isSelected,
      },
    };
  });
  return layoutSimpleFlow(flowNodes, flowEdges);
}

export async function renderCfgOfFunc(
  module: ModuleCache,
  func: GlobalID,
  focusBlock: BlockID | null,
  focusEdge: JumpTargetID | null,
): Promise<[FlowNode[], FlowEdge[]] | null> {
  const cfg = makeCfg(module, func);
  if (!cfg) return null;
  const [nodes, edges] = cfg;
  return await renderCfgToFlow(nodes, edges, focusBlock, focusEdge);
}
