import { type FlowNode, type FlowGroupNodeData } from "../components/Node";
import { type FlowEdge } from "../components/Edge";
import type { BlockID, UseID, UseKind, ValueDt } from "../../ir/ir";
import type { ModuleCache } from "../../ir/ir-state";
import * as Viz from "@viz-js/viz";
import type { DrawOps, GraphvizJSON, Point } from "../graphviz-object";
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
  const flowNodes: FlowNode[] = dfg.nodes.map((node) => ({
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
  return await layoutFlow(flowNodes, flowEdges);
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
  const { nodes: sections, edges: edgesDt } = module.makeBlockDfg(blockID);

  const sectionFlow = sections.map((section) => {
    const nodes = section.nodes.map(
      (nodeDt) =>
        ({
          id: nodeDt.id,
          position: { x: 0, y: 0 },
          width: 120,
          height: 45,
          type: "elemNode",
          data: {
            label: module.valueGetName(nodeDt.value) ?? nodeDt.id,
            focused: false,
            irObjID: null,
            bgColor:
              section.kind === "Pure" || section.kind === "Effect"
                ? "lightblue"
                : section.kind === "Income"
                  ? "lightgreen"
                  : "lightcoral",
            sectionId: section.id,
            sectionKind: section.kind,
          },
          label: module.valueGetName(nodeDt.value) ?? nodeDt.id,
        }) as FlowNode,
    );
    return {
      kind: section.kind,
      nodes,
    };
  });

  const flowEdges: FlowEdge[] = edgesDt.map((edgeDt) => ({
    id: edgeDt.id,
    source: edgeDt.operand,
    target: edgeDt.user,
    type: "flowEdge",
    label: edgeDt.kind,
    data: {
      label: edgeDt.kind,
      mainPaths: [],
      arrowPaths: [],
      labelX: 0,
      labelY: 0,
    },
  }));

  return await layoutBlockDfgFlow(sectionFlow, flowEdges);
}

type Pt = { x: number; y: number };
type SectionFlowInput = { nodes: FlowNode[]; kind: string };

function makeToPt(transform?: (p: Point) => Point) {
  return function toPt(p: Point): Pt {
    const tp = transform ? transform(p) : p;
    return { x: tp[0], y: tp[1] };
  };
}

function pointsToPolylinePath(pts: Pt[], close = false): string {
  if (!pts.length) return "";
  const segs = [`M ${pts[0].x} ${pts[0].y}`];
  for (let i = 1; i < pts.length; i++) segs.push(`L ${pts[i].x} ${pts[i].y}`);
  if (close) segs.push("Z");
  return segs.join(" ");
}

function catmullRomToBeziers(pts: Pt[]): string[] {
  const beziers: string[] = [];
  if (pts.length < 2) return beziers;
  const p = [pts[0], ...pts, pts[pts.length - 1]];
  for (let i = 0; i < pts.length - 1; i++) {
    const [p0, p1, p2, p3] = p.slice(i, i + 4);
    const b1x = p1.x + (p2.x - p0.x) / 6;
    const b1y = p1.y + (p2.y - p0.y) / 6;
    const b2x = p2.x - (p3.x - p1.x) / 6;
    const b2y = p2.y - (p3.y - p1.y) / 6;
    beziers.push(`C ${b1x} ${b1y} ${b2x} ${b2y} ${p2.x} ${p2.y}`);
  }
  return beziers;
}

function bsplinePointsToPath(points: Point[], toPt: (p: Point) => Pt): string {
  if (!points.length) return "";
  const pts = points.map(toPt);
  const beziers = catmullRomToBeziers(pts);
  if (!beziers.length) return pointsToPolylinePath(pts, false);
  return `M ${pts[0].x} ${pts[0].y} ${beziers.join(" ")}`;
}

function drawOpsToSvgPaths(
  ops?: DrawOps,
  transform?: (p: Point) => Point,
): string[] {
  if (!ops || !ops.length) return [];
  const toPt = makeToPt(transform);
  const paths: string[] = [];
  for (const op of ops) {
    switch (op.op) {
      case "b":
      case "B":
        if (op.points?.length) paths.push(bsplinePointsToPath(op.points, toPt));
        break;
      case "P":
      case "p":
        if (op.points?.length)
          paths.push(pointsToPolylinePath(op.points.map(toPt), true));
        break;
      case "L":
        if (op.points?.length)
          paths.push(pointsToPolylinePath(op.points.map(toPt), false));
        break;
      default:
        break;
    }
  }
  return paths.filter(Boolean);
}

function extractTextOps(
  ops?: DrawOps,
  transform?: (p: Point) => Point,
): { text: string; x: number; y: number }[] {
  if (!ops || !ops.length) return [];
  const toPt = makeToPt(transform);
  const results: { text: string; x: number; y: number }[] = [];
  for (const op of ops) {
    if (op.op !== "T") continue;
    const pt = toPt(op.pt);
    results.push({ text: op.text, x: pt.x, y: pt.y });
  }
  return results;
}

function collectBoundsFromDrawOps(
  ops?: DrawOps,
  transform?: (p: Point) => Point,
): { minX: number; minY: number; maxX: number; maxY: number } | null {
  if (!ops || !ops.length) return null;
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;
  const pushPoint = (point: Point) => {
    const pt = transform ? transform(point) : point;
    minX = Math.min(minX, pt[0]);
    minY = Math.min(minY, pt[1]);
    maxX = Math.max(maxX, pt[0]);
    maxY = Math.max(maxY, pt[1]);
  };
  for (const op of ops) {
    switch (op.op) {
      case "b":
      case "B":
      case "L":
      case "p":
      case "P":
        for (const point of op.points ?? []) pushPoint(point);
        break;
      case "e":
      case "E":
        if (op.rect) {
          pushPoint([op.rect[0], op.rect[1]]);
          pushPoint([op.rect[2], op.rect[3]]);
        }
        break;
      default:
        break;
    }
  }
  if (
    !Number.isFinite(minX) ||
    !Number.isFinite(minY) ||
    !Number.isFinite(maxX) ||
    !Number.isFinite(maxY)
  ) {
    return null;
  }
  return { minX, minY, maxX, maxY };
}

function makeSectionEdgePath(from: Pt, to: Pt): string {
  const dy = Math.max(40, (to.y - from.y) * 0.6);
  const c1 = { x: from.x, y: from.y + dy };
  const c2 = { x: to.x, y: to.y - dy };
  return `M ${from.x} ${from.y} C ${c1.x} ${c1.y} ${c2.x} ${c2.y} ${to.x} ${to.y}`;
}

function makeArrowAt(to: Pt, size = 5): string {
  const p1 = { x: to.x, y: to.y };
  const p2 = { x: to.x - size, y: to.y - size * 1.2 };
  const p3 = { x: to.x + size, y: to.y - size * 1.2 };
  return `M ${p1.x} ${p1.y} L ${p2.x} ${p2.y} L ${p3.x} ${p3.y} Z`;
}

async function layoutBlockDfgFlow(
  sections: SectionFlowInput[],
  edges: FlowEdge[],
): Promise<[FlowNode[], FlowEdge[]]> {
  const viz = await Viz.instance();

  const allNodes = sections.flatMap((section) => section.nodes);
  const nodeMap = new Map<string, FlowNode>(
    allNodes.map((node) => [node.id, node]),
  );

  const effectExecEdges: FlowEdge[] = sections.flatMap(
    (section, sectionIdx) => {
      if (section.kind !== "Effect" || section.nodes.length < 2) return [];
      const result: FlowEdge[] = [];
      for (let i = 0; i < section.nodes.length - 1; i++) {
        const from = section.nodes[i].id;
        const to = section.nodes[i + 1].id;
        result.push({
          id: `effect-flow:${sectionIdx}:${from}->${to}`,
          source: from,
          target: to,
          type: "flowEdge",
          label: "exec",
          data: {
            label: "exec",
            mainPaths: [],
            arrowPaths: [],
            labelX: 0,
            labelY: 0,
            strokeColor: "#94a3b8",
          },
        });
      }
      return result;
    },
  );

  const graphEdges: FlowEdge[] = [...edges, ...effectExecEdges];
  const edgeMap = new Map<string, FlowEdge>(
    graphEdges.map((edge) => [edge.id, edge]),
  );

  const nodeSizePixels = {
    width: (allNodes[0]?.width ?? 120) * 1.25,
    height: (allNodes[0]?.height ?? 45) * 1.25,
  };
  const nodeSizeInches = {
    width: nodeSizePixels.width / 96,
    height: nodeSizePixels.height / 96,
  };

  const sectionMeta = sections.map((section, index) => {
    const groupId = `dfg-section-${index}`;
    const clusterName = `cluster_dfg_${index}`;
    const anchorName = `anchor_dfg_${index}`;
    return {
      ...section,
      index,
      groupId,
      clusterName,
      anchorName,
    };
  });

  const graph: Viz.Graph = {
    directed: true,
    graphAttributes: {
      splines: "spline",
      rankdir: "TB",
      newrank: true,
    },
    nodeAttributes: {
      shape: "box",
      width: nodeSizeInches.width,
      height: nodeSizeInches.height,
      fixedsize: true,
    },
    edgeAttributes: { penwidth: 1 },
    subgraphs: sectionMeta.map((section) => ({
      name: section.clusterName,
      graphAttributes: {
        label: section.kind,
        style: "rounded",
        color: "#c7c7c7",
        penwidth: 1,
      },
      nodes: [
        ...section.nodes.map((node) => ({ name: node.id })),
        {
          name: section.anchorName,
          attributes: {
            shape: "point",
            width: 0.01,
            height: 0.01,
            fixedsize: true,
            style: "invis",
            label: "",
          },
        },
      ],
    })),
    edges: [
      ...graphEdges.map((edge) => ({
        tail: edge.source,
        head: edge.target,
        attributes: {
          remusys_edge_id: edge.id,
          label: edge.data?.label ?? edge.id,
        },
      })),
      ...sectionMeta.slice(0, -1).map((section) => ({
        tail: section.anchorName,
        head: sectionMeta[section.index + 1].anchorName,
        attributes: {
          style: "invis",
          constraint: true,
          weight: 100,
          minlen: 1,
        },
      })),
    ],
  };

  const jsonObj = viz.renderJSON(graph, { format: "json" }) as GraphvizJSON;
  if (!jsonObj.objects || !jsonObj.edges) {
    return [allNodes, edges];
  }

  let yMax: number | undefined;
  if (typeof jsonObj.bb === "string") {
    const parts = jsonObj.bb.split(",").map((v) => Number.parseFloat(v));
    if (parts.length === 4 && Number.isFinite(parts[3])) yMax = parts[3];
  }
  const transform =
    typeof yMax === "number"
      ? (p: Point) => [p[0], yMax! - p[1]] as Point
      : undefined;

  const gvidToName = new Map<number, string>();
  const clusterNameToGroup = new Map(
    sectionMeta.map((s) => [s.clusterName, s]),
  );
  const groupBounds = new Map<
    string,
    { minX: number; minY: number; maxX: number; maxY: number }
  >();

  for (const obj of jsonObj.objects) {
    if (typeof obj._gvid === "number") gvidToName.set(obj._gvid, obj.name);

    const section = clusterNameToGroup.get(obj.name);
    if (section) {
      const bounds = collectBoundsFromDrawOps(obj._draw_, transform);
      if (bounds) groupBounds.set(section.groupId, bounds);
      continue;
    }

    const node = nodeMap.get(obj.name);
    if (!node || !obj.pos) continue;

    const [xStr, yStr] = (obj.pos as string).split(",");
    const x = Number.parseFloat(xStr);
    const rawY = Number.parseFloat(yStr);
    const y = typeof yMax === "number" ? yMax - rawY : rawY;

    const DPI = 96;
    const widthIn = obj.width
      ? Number.parseFloat(obj.width as string)
      : undefined;
    const heightIn = obj.height
      ? Number.parseFloat(obj.height as string)
      : undefined;
    const widthWithBox =
      typeof widthIn === "number" && !Number.isNaN(widthIn)
        ? widthIn * DPI
        : (node.width ?? 64);
    const heightWithBox =
      typeof heightIn === "number" && !Number.isNaN(heightIn)
        ? heightIn * DPI
        : (node.height ?? 64);
    const width = widthWithBox / 1.25;
    const height = heightWithBox / 1.25;
    node.position = { x: x - width / 2, y: y - height / 2 };
  }

  for (const edgeObj of jsonObj.edges) {
    const edgeId =
      typeof edgeObj.remusys_edge_id === "string"
        ? edgeObj.remusys_edge_id
        : undefined;
    if (!edgeId) continue;
    const edge = edgeMap.get(edgeId);
    if (!edge || !edge.data) continue;

    const sourceName = gvidToName.get(edgeObj.tail);
    const targetName = gvidToName.get(edgeObj.head);
    if (sourceName) edge.source = sourceName;
    if (targetName) edge.target = targetName;

    const mainPaths = drawOpsToSvgPaths(edgeObj._draw_, transform);
    const arrowPaths = [
      ...drawOpsToSvgPaths(edgeObj._hdraw_, transform),
      ...drawOpsToSvgPaths(edgeObj._tdraw_, transform),
    ];
    const textOps = extractTextOps(edgeObj._ldraw_, transform);
    const labelPos = textOps[0];

    edge.data = {
      ...edge.data,
      mainPaths,
      arrowPaths,
      labelX: labelPos?.x ?? edge.data.labelX,
      labelY: labelPos?.y ?? edge.data.labelY,
    };
  }

  const groupNodes: FlowNode[] = sectionMeta.map((section) => {
    const bounds = groupBounds.get(section.groupId) ?? {
      minX: 0,
      minY: 0,
      maxX: 160,
      maxY: 72,
    };
    return {
      id: section.groupId,
      position: { x: bounds.minX, y: bounds.minY },
      width: Math.max(40, bounds.maxX - bounds.minX),
      height: Math.max(28, bounds.maxY - bounds.minY),
      type: "groupNode",
      selectable: false,
      draggable: false,
      data: {
        label: section.kind,
        focused: false,
        irObjID: null,
        bgColor: "transparent",
        childIds: section.nodes.map((node) => node.id),
      } satisfies FlowGroupNodeData,
    };
  });

  const sectionEdges: FlowEdge[] = sectionMeta.slice(2, -1).map((section) => {
    const fromGroup = groupNodes.find((node) => node.id === section.groupId);
    const toGroup = groupNodes.find(
      (node) => node.id === sectionMeta[section.index + 1].groupId,
    );
    if (!fromGroup || !toGroup) {
      throw new Error(
        `Group nodes not found for section edges: ${section.groupId} or ${sectionMeta[section.index + 1].groupId}`,
      );
    }
    const from = {
      x: fromGroup.position.x + (fromGroup.width ?? 0) / 2,
      y: fromGroup.position.y + (fromGroup.height ?? 0),
    };
    const to = {
      x: toGroup.position.x + (toGroup.width ?? 0) / 2,
      y: toGroup.position.y,
    };
    return {
      id: `section-flow:${section.groupId}->${sectionMeta[section.index + 1].groupId}`,
      source: section.groupId,
      target: sectionMeta[section.index + 1].groupId,
      type: "flowEdge",
      data: {
        label: "exec",
        mainPaths: [makeSectionEdgePath(from, to)],
        arrowPaths: [makeArrowAt(to)],
        labelX: (from.x + to.x) / 2,
        labelY: (from.y + to.y) / 2,
        strokeColor: "#006aff",
      },
    };
  });

  return [
    [...groupNodes, ...allNodes],
    [...sectionEdges, ...graphEdges],
  ];
}
