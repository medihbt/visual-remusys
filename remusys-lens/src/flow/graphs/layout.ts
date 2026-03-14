import * as Viz from "@viz-js/viz";
import type { GraphvizJSON, DrawOps, Point } from "../graphviz-object";
import type { FlowElemNode, FlowNode, FlowGroupNode } from "../components/Node";
import type { FlowEdge } from "../components/Edge";
import type { BlockDfgSectionKind } from "../../ir/ir";

const viz = await Viz.instance();
export type FlowGraph = [FlowNode[], FlowEdge[]];
export type AsyncFlowGraph = Promise<FlowGraph>;

// Simple flow nodes, without any sub-graphs.
export function layoutSimpleFlow(
  nodes: FlowElemNode[],
  edges: FlowEdge[],
): FlowGraph {
  const dot = getDotFromSimple(nodes, edges);
  const json = viz.renderJSON(dot, {
    engine: "dot",
    format: "json0",
  }) as GraphvizJSON;
  return decodeSimpleLayout(nodes, edges, json);
}

function getDotFromSimple(nodes: FlowElemNode[], edges: FlowEdge[]): Viz.Graph {
  const nodeSizePixels = {
    width: (nodes[0]?.width ?? 120) * 1.25,
    height: (nodes[0]?.height ?? 45) * 1.25,
  };
  const nodeSizeInches = {
    width: nodeSizePixels.width / 96, // assuming 96 DPI
    height: nodeSizePixels.height / 96,
  };

  const graph: Viz.Graph = {
    directed: true,
    nodes: nodes.map((node) => ({ name: node.id })),
    edges: edges.map((e) => ({
      attributes: {
        remusys_edge_id: e.id,
        label: e.data?.label ?? e.id,
      },
      tail: e.source,
      head: e.target,
    })),
    graphAttributes: {
      splines: "spline",
    },
    nodeAttributes: {
      shape: "box",
      width: nodeSizeInches.width,
      height: nodeSizeInches.height,
      fixedsize: true,
    },
    edgeAttributes: { penwidth: "1" },
  };
  return graph;
}

type Pt = { x: number; y: number };

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
  else return `M ${pts[0].x} ${pts[0].y} ${beziers.join(" ")}`;
}
function ellipseToPath(rect: [number, number, number, number]): string {
  const [x1, y1, x2, y2] = rect;
  const cx = (x1 + x2) / 2;
  const cy = (y1 + y2) / 2;
  const rx = Math.abs((x2 - x1) / 2);
  const ry = Math.abs((y2 - y1) / 2);
  if (rx === 0 || ry === 0) return "";
  return `M ${cx - rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx + rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx - rx} ${cy}`;
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
      case "B": {
        // Graphviz 'b' / 'B' draw ops represent smooth splines. Convert the
        // supplied point sequence into a smooth cubic Bézier path so the
        // resulting SVG matches Graphviz's intended visual curves.
        if (op.points?.length) {
          paths.push(bsplinePointsToPath(op.points, toPt));
        }
        break;
      }
      case "P":
      case "p": {
        if (op.points?.length)
          paths.push(pointsToPolylinePath(op.points.map(toPt), true));
        break;
      }
      case "L": {
        if (op.points?.length)
          paths.push(pointsToPolylinePath(op.points.map(toPt), false));
        break;
      }
      case "e":
      case "E": {
        if (op.rect) {
          const p0 = transform
            ? transform([op.rect[0], op.rect[1]])
            : ([op.rect[0], op.rect[1]] as Point);
          const p1 = transform
            ? transform([op.rect[2], op.rect[3]])
            : ([op.rect[2], op.rect[3]] as Point);
          paths.push(ellipseToPath([p0[0], p0[1], p1[0], p1[1]]));
        }
        break;
      }
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

function parseEdgePosToPath(
  pos?: string,
  transform?: (p: Point) => Point,
): string {
  if (!pos) return "";
  // Example: "e,38.5,36.175 38.5,86.799 38.5,75.163 ..."
  const tokens = pos.trim().split(/\s+/).filter(Boolean);
  const points: Point[] = [];

  for (const token of tokens) {
    const body =
      token.startsWith("e,") || token.startsWith("s,") ? token.slice(2) : token;
    const [xs, ys] = body.split(",");
    const x = Number.parseFloat(xs);
    const y = Number.parseFloat(ys);
    if (Number.isFinite(x) && Number.isFinite(y)) points.push([x, y]);
  }

  if (!points.length) return "";
  const toPt = makeToPt(transform);
  // Use the same smoothing conversion as draw ops: convert the point sequence
  // into a visually smooth cubic Bézier path (Catmull‑Rom -> Bézier).
  return bsplinePointsToPath(points, toPt);
}

function decodeSimpleLayout(
  nodes: FlowNode[],
  edges: FlowEdge[],
  json: GraphvizJSON,
): [FlowNode[], FlowEdge[]] {
  const edgeMap: Map<string, FlowEdge> = new Map(edges.map((e) => [e.id, e]));
  const nodeMap: Map<string, FlowNode> = new Map(nodes.map((n) => [n.id, n]));
  if (!json.objects || !json.edges) return [nodes, edges];

  let yMax: number | undefined;
  if (typeof json.bb === "string") {
    const parts = json.bb.split(",").map((x) => Number.parseFloat(x));
    if (parts.length === 4 && Number.isFinite(parts[3])) {
      yMax = parts[3];
    }
  }
  const transform =
    typeof yMax === "number"
      ? (p: Point) => [p[0], yMax! - p[1]] as Point
      : undefined;

  const gvidToName = new Map<number, string>();
  for (const obj of json.objects) {
    if (typeof obj._gvid === "number") {
      gvidToName.set(obj._gvid, obj.name);
    }

    const nodeName = obj.name;
    const node = nodeMap.get(nodeName);
    if (node && obj.pos) {
      const [xStr, yStr] = (obj.pos as string).split(",");
      const x = parseFloat(xStr);
      const rawY = parseFloat(yStr);
      const y = typeof yMax === "number" ? yMax - rawY : rawY;
      // Graphviz reports node position as the center; convert to top-left by
      // subtracting half of the node's width/height. Graphviz width/height are
      // in inches; convert to pixels (assume 96 DPI). If width/height are
      // missing, fall back to reasonable defaults.
      const DPI = 96; // pixels per inch
      const widthIn = obj.width ? parseFloat(obj.width as string) : undefined;
      const heightIn = obj.height
        ? parseFloat(obj.height as string)
        : undefined;
      const widthWithBox =
        typeof widthIn === "number" && !Number.isNaN(widthIn)
          ? widthIn * DPI
          : (node.width ?? 64);
      const heightWithBox =
        typeof heightIn === "number" && !Number.isNaN(heightIn)
          ? heightIn * DPI
          : (node.height ?? 64);
      const width = widthWithBox / 1.25; // remove the padding we added for layout
      const height = heightWithBox / 1.25;
      node.position = { x: x - width / 2, y: y - height / 2 };
      // store computed size for downstream consumers (if they reference it)
      if (!node.data) throw new Error("Node has no data");
    }
  }

  const fallbackEdgeByEndpoints = (
    source?: string,
    target?: string,
  ): FlowEdge | undefined => {
    if (!source || !target) return undefined;
    return edges.find((e) => e.source === source && e.target === target);
  };

  for (const e of json.edges) {
    const source = gvidToName.get(e.tail);
    const target = gvidToName.get(e.head);

    const textOps = extractTextOps(e._ldraw_, transform);
    const edgeIdFromAttr =
      typeof e.remusys_edge_id === "string" ? e.remusys_edge_id : "";
    const labelFromDraw = textOps
      .map((t) => t.text)
      .join(" ")
      .trim();
    const labelText =
      (typeof e.label === "string" ? e.label : "") || labelFromDraw;

    const edgeById = edgeIdFromAttr
      ? edgeMap.get(edgeIdFromAttr)
      : typeof e._gvid === "number"
        ? edgeMap.get(String(e._gvid))
        : undefined;
    const edge = edgeById ?? fallbackEdgeByEndpoints(source, target);
    if (!edge) continue;

    const mainPaths = drawOpsToSvgPaths(e._draw_, transform);
    const fallbackMainPath = parseEdgePosToPath(
      typeof e.pos === "string" ? e.pos : undefined,
      transform,
    );
    const arrowPaths = [
      ...drawOpsToSvgPaths(e._hdraw_, transform),
      ...drawOpsToSvgPaths(e._tdraw_, transform),
    ];

    const labelPos = textOps[0];
    if (!edge.data)
      throw new Error(`Edge ${edge.id} missing data property for layout info`);
    edge.source = source ?? edge.source;
    edge.target = target ?? edge.target;
    if (labelText) edge.label = labelText;
    edge.data = {
      ...edge.data,
      mainPaths: mainPaths.length
        ? mainPaths
        : fallbackMainPath
          ? [fallbackMainPath]
          : [],
      arrowPaths,
      labelX: labelPos?.x ?? edge.data?.labelX ?? 0,
      labelY: labelPos?.y ?? edge.data?.labelY ?? 0,
    };
  }
  return [nodes, edges];
}

export type FlowSection = {
  id: string;
  label: string;
  /** BlockDfgSectionKind = "Income" | "Outcome" | "Pure" | "Effect" */
  kind: BlockDfgSectionKind;
  nodes: FlowElemNode[];
  internalEdges: FlowEdge[];
};
export type SectionFlowGraph = {
  sections: FlowSection[];
  crossEdges: FlowEdge[];
};

function buildSectionDotGraph(
  graph: SectionFlowGraph,
  sampleNode?: FlowElemNode,
): Viz.Graph {
  // 节点尺寸计算（复用现有逻辑）
  const nodeSizePixels = {
    width: (sampleNode?.width ?? 120) * 1.25,
    height: (sampleNode?.height ?? 45) * 1.25,
  };
  const nodeSizeInches = {
    width: nodeSizePixels.width / 96, // 96 DPI 假设
    height: nodeSizePixels.height / 96,
  };

  const dotGraph: Viz.Graph = {
    directed: true,
    strict: false, // 允许重边
    graphAttributes: {
      rankdir: "TB", // 纵向布局
      splines: "spline", // 平滑曲线
    },
    nodeAttributes: {
      shape: "box",
      width: nodeSizeInches.width,
      height: nodeSizeInches.height,
      fixedsize: true,
    },
    edgeAttributes: {
      penwidth: "1",
    },
    subgraphs: [],
    edges: [],
  };

  // 为每个节创建集群子图
  let rank = 10;
  for (const section of graph.sections) {
    const subgraph: Viz.Subgraph = {
      name: `cluster_${section.id}`, // "cluster_" 前缀启用集群布局
      nodes: section.nodes.map((node) => ({ name: node.id })),
      edges: [],
    };

    // 设置节特定的布局约束
    const graphAttrs: Viz.Attributes = {};
    if (section.kind === "Income") {
      graphAttrs.rank = "source";
    } else if (section.kind === "Outcome") {
      graphAttrs.rank = "sink";
    } else {
      graphAttrs.rank = rank;
      rank += 10; // 为 Pure 和 Effect 分配中间的 rank 值，保持它们在 Income 和 Outcome 之间
    }
    if (Object.keys(graphAttrs).length > 0) {
      subgraph.graphAttributes = graphAttrs;
    }

    // 转换内部边
    subgraph.edges = section.internalEdges.map((edge) => ({
      tail: edge.source,
      head: edge.target,
      attributes: {
        remusys_edge_id: edge.id,
        label: edge.data?.label ?? edge.id,
      },
    }));

    // Effect 节：添加隐形边强制垂直顺序
    if (section.kind === "Effect" && section.nodes.length > 1) {
      for (let i = 0; i < section.nodes.length - 1; i++) {
        const nodeA = section.nodes[i];
        const nodeB = section.nodes[i + 1];
        subgraph.edges!.push({
          tail: nodeA.id,
          head: nodeB.id,
          attributes: {
            style: "invis",
            constraint: true,
            remusys_edge_id: `invis_${section.id}_${i}`,
          },
        });
      }
    }

    dotGraph.subgraphs!.push(subgraph);
  }

  // 添加跨节边（顶级边）
  dotGraph.edges = graph.crossEdges.map((edge) => ({
    tail: edge.source,
    head: edge.target,
    attributes: {
      remusys_edge_id: edge.id,
      label: edge.data?.label ?? edge.id,
    },
  }));

  return dotGraph;
}

export function layoutSectionFlow(graph: SectionFlowGraph): FlowGraph {
  // 收集所有节点和边
  const allNodes: FlowElemNode[] = [];
  const allEdges: FlowEdge[] = [];

  for (const section of graph.sections) {
    allNodes.push(...section.nodes);
    allEdges.push(...section.internalEdges);
  }
  allEdges.push(...graph.crossEdges);

  // 构建 Graphviz 图结构
  const dotGraph = buildSectionDotGraph(graph, allNodes[0]);

  // 调用 Graphviz 布局引擎
  const json = viz.renderJSON(dotGraph, {
    engine: "dot",
    format: "json0",
  }) as GraphvizJSON;

  // 解码布局结果（得到绝对坐标的节点）
  const [flatNodes, edges] = decodeSimpleLayout(allNodes, allEdges, json);

  // 转换为 React Flow 分组结构
  return createGroupedLayout(graph, flatNodes, edges);
}

function createGroupedLayout(
  graph: SectionFlowGraph,
  flatNodes: FlowNode[],
  edges: FlowEdge[],
): FlowGraph {
  const resultNodes: FlowNode[] = [];
  const nodeMap = new Map<string, FlowNode>();

  // 将平面节点存入映射表
  for (const node of flatNodes) {
    nodeMap.set(node.id, node);
  }

  // 为每个 section 创建 group 节点并处理子节点
  for (const section of graph.sections) {
    // 收集 section 内所有节点
    const sectionNodes: FlowNode[] = [];
    for (const node of section.nodes) {
      const flowNode = nodeMap.get(node.id);
      if (flowNode && flowNode.type === "elemNode") {
        sectionNodes.push(flowNode);
      }
    }

    if (sectionNodes.length === 0) {
      continue;
    }

    // 计算 section 的边界框（基于节点的绝对位置）
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;

    for (const node of sectionNodes) {
      const { x, y } = node.position;
      const width = node.width ?? 120;
      const height = node.height ?? 45;

      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x + width);
      maxY = Math.max(maxY, y + height);
    }

    // 添加一些内边距
    const padding = 20;
    const groupX = minX - padding;
    const groupY = minY - padding;
    const groupWidth = maxX - minX + 2 * padding;
    const groupHeight = maxY - minY + 2 * padding;

    // 创建 group 节点
    const groupNode: FlowGroupNode = {
      id: `group_${section.id}`,
      type: "groupNode",
      position: { x: groupX, y: groupY },
      width: groupWidth,
      height: groupHeight,
      data: {
        label: section.label,
        focused: false,
        irObjID: null,
        bgColor: getGroupNodeBgColor(section.kind),
      },
      style: {
        border: "1px dashed #ccc",
        borderRadius: 3,
        backgroundColor: getGroupNodeBgColor(section.kind),
      },
      draggable: section.kind === "Pure",
    };

    // 首先添加 group 节点（父节点必须在子节点之前）
    resultNodes.push(groupNode);

    // 然后添加子节点，转换为相对位置并设置 parentId
    for (const node of sectionNodes) {
      if (node.type === "elemNode") {
        const relativeNode: FlowNode = {
          ...node,
          position: {
            x: node.position.x - groupX,
            y: node.position.y - groupY,
          },
          parentId: groupNode.id,
          extent: "parent",
          draggable: section.kind === "Pure",
        };
        resultNodes.push(relativeNode);
      }
    }
  }

  // 添加不属于任何 section 的节点（如果有的话）
  for (const node of flatNodes) {
    if (!resultNodes.some((n) => n.id === node.id)) {
      resultNodes.push(node);
    }
  }

  return [resultNodes, edges];
}

function getGroupNodeBgColor(kind: BlockDfgSectionKind): string {
  // Group nodes use minimal styling, but we provide subtle color hints
  switch (kind) {
    case "Income":
      return "rgba(144, 238, 144, 0.1)";
    case "Outcome":
      return "rgba(240, 128, 128, 0.1)";
    case "Pure":
      return "rgba(255, 255, 224, 0.1)";
    case "Effect":
      return "rgba(173, 216, 230, 0.1)";
    default:
      return "transparent";
  }
}
