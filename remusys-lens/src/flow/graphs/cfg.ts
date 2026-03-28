import { irMakeCfg } from "../../ir/ir";
import type { BlockID, CfgEdge, CfgNode, GlobalID, JTKind, JumpTargetID } from "../../ir/ir";
import { ModuleCache } from "../../ir/ir-state";
import type { FlowEdge } from "../components/Edge";
import type { FlowElemNode, FlowNode } from "../components/Node";
import { layoutSimpleFlow } from "./layout";

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
  const { nodes, edges } = irMakeCfg(module.moduleId, func);
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
    let dashPattern: `${number} ${number}` | "none" = "none";
    let dashAndLine = false;
    switch (e.edge_class) {
      case "Unreachable":
        dashPattern = "2 2";
        break;
      case "Cross":
        dashPattern = "4 4";
        break;
      case "Back":
        dashPattern = "6 3";
        dashAndLine = true;
        break;
      case "Forward":
        dashPattern = "4 2";
        dashAndLine = true;
        break;
      default:
        dashPattern = "none";
        break;
    }
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
        strokeColor: e.edge_class === "Unreachable" ? "#9ca3af" : getStrokeColor(e.kind),
        dashPattern,
        dashAndLine,
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
