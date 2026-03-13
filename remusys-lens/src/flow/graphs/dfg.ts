import { type FlowNode } from "../components/Node";
import { type FlowEdge } from "../components/Edge";
import type { BlockID, UseID, UseKind, ValueDt } from "../../ir/ir";
import type { ModuleCache } from "../../ir/ir-state";
import { layoutFlow } from "./layout";

export type DfgNodeKind = "Income" | "Focused" | "Outcome";

export type DfgNode = {
  nodeID: string;
  value: ValueDt;
  kind: DfgNodeKind;
};
export type DfgEdge = {
  operandID: string;
  userID: string;
  useKind: UseKind;
  useID: UseID;
};
export type Dfg = {
  nodes: DfgNode[];
  edges: DfgEdge[];
};

export function valueIsTraceable(value: ValueDt): boolean {
  return /^(Inst|Global|FuncArg|Block|Expr)$/.test(value.type);
}

export class DfgBuilder {
  private readonly nodes: Map<string, DfgNode> = new Map();
  private readonly edges: Map<UseID, DfgEdge> = new Map();
  readonly module: ModuleCache;

  constructor(module: ModuleCache) {
    this.module = module;
  }
  extractDfg(): Dfg {
    return {
      nodes: Array.from(this.nodes.values()),
      edges: Array.from(this.edges.values())
    }
  }
  nodeGetID(fallback: UseID | null, value: ValueDt): string {
    switch (value.type) {
      case "Inst": case "Global": case "Block": case "Expr":
        return value.value;
      case "FuncArg":
        return `Arg(${value.value[0]},${value.value[1]})`;
      default:
        if (fallback)
          return fallback;
        else
          throw new Error("Cannot generate node ID for value without fallback");
    }
  }
  addNode(value: ValueDt, fallbackID: UseID | null, kind: DfgNodeKind): DfgNode {
    const nodeID = this.nodeGetID(fallbackID, value);
    const node = this.nodes.get(nodeID);
    if (node) {
      return node;
    } else {
      const newNode = { nodeID, value, kind };
      this.nodes.set(nodeID, newNode);
      return newNode;
    }
  }
  addEdgeWithNodes(edge: UseID, userKind: DfgNodeKind, operandKind: DfgNodeKind): DfgEdge {
    const useObj = this.module.loadUse(edge);
    const operandNode = this.addNode(useObj.value, edge, operandKind);
    const userNode = this.addNode(useObj.user, edge, userKind);
    if (this.edges.has(edge)) {
      return this.edges.get(edge)!;
    } else {
      const newEdge = {
        operandID: operandNode.nodeID,
        userID: userNode.nodeID,
        useKind: useObj.kind,
        useID: edge
      };
      this.edges.set(edge, newEdge);
      return newEdge;
    }
  }

  static buildFromCentered(centerValue: ValueDt, module: ModuleCache): Dfg {
    const builder = new DfgBuilder(module);
    builder.addNode(centerValue, null, "Focused");
    for (const useDt of module.getValueOperands(centerValue)) {
      builder.addEdgeWithNodes(useDt.id, "Focused", "Income");
    }
    for (const useDt of module.getValueUsers(centerValue)) {
      builder.addEdgeWithNodes(useDt.id, "Outcome", "Focused");
    }
    return builder.extractDfg();
  }
  static buildInsideBlock(blockID: BlockID, module: ModuleCache): Dfg {
    const builder = new DfgBuilder(module);
    const blockObj = module.loadBlock(blockID);
    const instsSet = new Set(blockObj.insts.map(inst => inst.id));
    for (const instObj of blockObj.insts) {
      const instValue: ValueDt = { type: "Inst", value: instObj.id };
      builder.addNode(instValue, null, "Focused");
      for (const useDt of module.getValueOperands(instValue)) {
        let operandNodeKind: DfgNodeKind;
        if (useDt.value.type === "Inst" && instsSet.has(useDt.value.value)) {
          operandNodeKind = "Focused";
        } else {
          operandNodeKind = "Income";
        }
        builder.addEdgeWithNodes(useDt.id, "Focused", operandNodeKind);
      }
      for (const useDt of module.getValueUsers(instValue)) {
        let userNodeKind: DfgNodeKind;
        if (useDt.user.type === "Inst" && instsSet.has(useDt.user.value)) {
          userNodeKind = "Focused";
        } else {
          userNodeKind = "Outcome";
        }
        builder.addEdgeWithNodes(useDt.id, userNodeKind, "Focused");
      }
    }
    return builder.extractDfg();
  }
}

export async function renderDfg(module: ModuleCache, dfg: Dfg): Promise<[FlowNode[], FlowEdge[]]> {
  const flowNodes: FlowNode[] = dfg.nodes.map(node => ({
    id: node.nodeID,
    position: { x: 0, y: 0 },
    width: 120,
    height: 45,
    type: "flowNode",
    data: {
      label: module.valueGetName(node.value) ?? node.nodeID,
      focused: false,
      irObjID: null,
      bgColor: node.kind === "Focused" ? "lightblue" : (node.kind === "Income" ? "lightgreen" : "lightcoral")
    }
  }));
  const flowEdges: FlowEdge[] = dfg.edges.map(edge => ({
    id: edge.useID,
    source: edge.operandID,
    target: edge.userID,
    type: "flowEdge",
    data: {
      label: edge.useKind,
      mainPaths: [],
      arrowPaths: [],
      labelX: 0,
      labelY: 0
    }
  }));
  return await layoutFlow(flowNodes, flowEdges);
}

export async function renderDfgFromCentered(
  centerValue: ValueDt,
  module: ModuleCache
): Promise<[FlowNode[], FlowEdge[]]> {
  const dfg = DfgBuilder.buildFromCentered(centerValue, module);
  return await renderDfg(module, dfg);
}
export async function renderDfgInsideBlock(
  blockID: BlockID,
  module: ModuleCache
): Promise<[FlowNode[], FlowEdge[]]> {
  const dfg = DfgBuilder.buildInsideBlock(blockID, module);
  return await renderDfg(module, dfg);
}
