import type { FlowNode, FlowElemNode } from "../components/Node";
import { type FlowEdge } from "../components/Edge";
import {
  IDCast,
  type BlockDfgSectionKind,
  type BlockID,
  type SourceTrackable,
  type UseID,
  type UseKind,
  type ValueDt,
} from "../../ir/ir";
import type { ModuleCache } from "../../ir/ir-state";
import {
  layoutSimpleFlow,
  layoutSectionFlow,
  type SectionFlowGraph,
  type FlowSection,
} from "./layout";

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
      edges: Array.from(this.edges.values()),
    };
  }
  nodeGetID(fallback: UseID | null, value: ValueDt): string {
    switch (value.type) {
      case "Inst":
      case "Global":
      case "Block":
      case "Expr":
        return value.value;
      case "FuncArg":
        return `Arg(${value.value[0]},${value.value[1]})`;
      default:
        if (fallback) return fallback;
        else
          throw new Error("Cannot generate node ID for value without fallback");
    }
  }
  addNode(
    value: ValueDt,
    fallbackID: UseID | null,
    kind: DfgNodeKind,
  ): DfgNode {
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
  addEdgeWithNodes(
    edge: UseID,
    userKind: DfgNodeKind,
    operandKind: DfgNodeKind,
  ): DfgEdge {
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
        useID: edge,
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
}

export async function renderDfg(
  module: ModuleCache,
  dfg: Dfg,
): Promise<[FlowNode[], FlowEdge[]]> {
  const flowNodes: FlowElemNode[] = dfg.nodes.map((node) => ({
    id: node.nodeID,
    position: { x: 0, y: 0 },
    width: 120,
    height: 45,
    type: "elemNode",
    data: {
      label: module.valueGetName(node.value) ?? node.nodeID,
      focused: false,
      irObjID: null,
      bgColor:
        node.kind === "Focused"
          ? "lightblue"
          : node.kind === "Income"
            ? "lightgreen"
            : "lightcoral",
    },
  }));
  const flowEdges: FlowEdge[] = dfg.edges.map((edge) => ({
    id: edge.useID,
    source: edge.operandID,
    target: edge.userID,
    type: "flowEdge",
    data: {
      label: edge.useKind,
      mainPaths: [],
      arrowPaths: [],
      labelX: 0,
      labelY: 0,
    },
  }));
  return layoutSimpleFlow(flowNodes, flowEdges);
}

export async function renderDfgFromCentered(
  centerValue: ValueDt,
  module: ModuleCache,
): Promise<[FlowNode[], FlowEdge[]]> {
  const dfg = DfgBuilder.buildFromCentered(centerValue, module);
  return await renderDfg(module, dfg);
}
export async function renderDfgInsideBlock(
  blockID: BlockID,
  module: ModuleCache,
): Promise<[FlowNode[], FlowEdge[]]> {
  const { nodes: sectionsDt, edges: edgesDt } = module.makeBlockDfg(blockID);

  // 构建 SectionFlowGraph
  const sectionFlowGraph: SectionFlowGraph = {
    sections: [],
    crossEdges: [],
  };

  // 映射：section ID -> FlowSection
  const sectionMap = new Map<number, FlowSection>();

  // 转换每个 section
  for (const sectionDt of sectionsDt) {
    const flowNodes: FlowElemNode[] = sectionDt.nodes.map((nodeDt) => {
      const irObjID: SourceTrackable | null = IDCast.asSourceTrackable(
        nodeDt.id,
      );
      return {
        id: nodeDt.id,
        position: { x: 0, y: 0 },
        width: 120,
        height: 45,
        type: "elemNode",
        data: {
          label: module.valueGetName(nodeDt.value) ?? nodeDt.id,
          focused: false,
          irObjID,
          bgColor: getSectionBgColor(sectionDt.kind),
        },
      };
    });

    const flowSection: FlowSection = {
      id: sectionDt.id.toString(),
      label: `${sectionDt.kind} (${sectionDt.id})`,
      kind: sectionDt.kind,
      nodes: flowNodes,
      internalEdges: [],
    };

    sectionFlowGraph.sections.push(flowSection);
    sectionMap.set(sectionDt.id, flowSection);
  }

  // 转换边：根据 section_id 判断是内部边还是跨节边
  for (const edgeDt of edgesDt) {
    const flowEdge: FlowEdge = {
      id: edgeDt.id,
      source: edgeDt.operand,
      target: edgeDt.user,
      type: "flowEdge",
      data: {
        label: edgeDt.kind,
        mainPaths: [],
        arrowPaths: [],
        labelX: 0,
        labelY: 0,
      },
    };

    // 如果边有 section_id 且对应的 section 存在，则尝试作为内部边
    if (edgeDt.section_id !== undefined) {
      const section = sectionMap.get(edgeDt.section_id);
      if (section) {
        // 简化：直接添加为内部边，由布局函数处理边分类
        section.internalEdges.push(flowEdge);
        continue;
      }
    }

    // 否则为跨节边
    sectionFlowGraph.crossEdges.push(flowEdge);
  }

  // 调用布局函数
  return layoutSectionFlow(sectionFlowGraph);
}

// 辅助函数：根据节类型获取背景颜色
function getSectionBgColor(kind: BlockDfgSectionKind): string {
  switch (kind) {
    case "Income":
      return "lightgreen";
    case "Outcome":
      return "lightcoral";
    case "Pure":
      return "lightyellow";
    case "Effect":
      return "lightblue";
    default:
      return "#e0e0e0";
  }
}
