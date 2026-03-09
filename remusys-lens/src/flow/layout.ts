import * as Viz from '@viz-js/viz';
import type { GraphvizJSON, DrawOps, Point } from './graphviz-object';
import type { FlowNode } from './components/Node';
import type { FlowEdge } from './components/Edge';

export async function layoutFlow(
  nodes: FlowNode[],
  edges: FlowEdge[],
): Promise<[FlowNode[], FlowEdge[]]> {
  const viz = await Viz.instance();
  const dotObject = getDotObject(nodes, edges);
  // Draw ops (_draw_/_ldraw_/_hdraw_) are only available in "json" output.
  const jsonObj = viz.renderJSON(dotObject, { format: 'json' }) as GraphvizJSON;
  return decodeLayout(nodes, edges, jsonObj);
}
function getDotObject(nodes: FlowNode[], edges: FlowEdge[]): Viz.Graph {
  let graph: Viz.Graph = {
    directed: true,
    nodes: nodes.map(n => ({ name: n.id })),
    edges: edges.map(e => ({
      attributes: {
        remusys_edge_id: e.id,
        label: e.data?.label ?? e.id,
      },
      tail: e.source,
      head: e.target,
    })),
    // Request smooth cubic splines from Graphviz. 'spline' asks for
    // interpolated cubic Bézier splines (smoother than polylines).
    graphAttributes: { splines: 'spline' },
    nodeAttributes: { shape: 'rectangle' },
    edgeAttributes: { penwidth: '1' },
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
  if (!pts.length) return '';
  const segs = [`M ${pts[0].x} ${pts[0].y}`];
  for (let i = 1; i < pts.length; i++)
    segs.push(`L ${pts[i].x} ${pts[i].y}`);
  if (close)
    segs.push('Z');
  return segs.join(' ');
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
  if (!points.length) return '';
  const pts = points.map(toPt);
  const beziers = catmullRomToBeziers(pts);
  if (!beziers.length) return pointsToPolylinePath(pts, false);
  return `M ${pts[0].x} ${pts[0].y} ${beziers.join(' ')}`;
}

function ellipseToPath(rect: [number, number, number, number]): string {
  const [x1, y1, x2, y2] = rect;
  const cx = (x1 + x2) / 2;
  const cy = (y1 + y2) / 2;
  const rx = Math.abs((x2 - x1) / 2);
  const ry = Math.abs((y2 - y1) / 2);
  if (rx === 0 || ry === 0) return '';
  return `M ${cx - rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx + rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx - rx} ${cy}`;
}

function drawOpsToSvgPaths(ops?: DrawOps, transform?: (p: Point) => Point): string[] {
  if (!ops || !ops.length) return [];
  const toPt = makeToPt(transform);
  const paths: string[] = [];

  for (const op of ops) {
    switch (op.op) {
      case 'b':
      case 'B': {
        // Graphviz 'b' / 'B' draw ops represent smooth splines. Convert the
        // supplied point sequence into a smooth cubic Bézier path so the
        // resulting SVG matches Graphviz's intended visual curves.
        if (op.points?.length) {
          paths.push(bsplinePointsToPath(op.points, toPt));
        }
        break;
      }
      case 'P':
      case 'p': {
        if (op.points?.length) paths.push(pointsToPolylinePath(op.points.map(toPt), true));
        break;
      }
      case 'L': {
        if (op.points?.length) paths.push(pointsToPolylinePath(op.points.map(toPt), false));
        break;
      }
      case 'e':
      case 'E': {
        if (op.rect) {
          const p0 = transform ? transform([op.rect[0], op.rect[1]]) : [op.rect[0], op.rect[1]] as Point;
          const p1 = transform ? transform([op.rect[2], op.rect[3]]) : [op.rect[2], op.rect[3]] as Point;
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

function extractTextOps(ops?: DrawOps, transform?: (p: Point) => Point): { text: string; x: number; y: number }[] {
  if (!ops || !ops.length) return [];
  const toPt = makeToPt(transform);
  const results: { text: string; x: number; y: number }[] = [];

  for (const op of ops) {
    if (op.op !== 'T') continue;
    const pt = toPt(op.pt);
    results.push({ text: op.text, x: pt.x, y: pt.y });
  }
  return results;
}

function parseEdgePosToPath(pos?: string, transform?: (p: Point) => Point): string {
  if (!pos) return '';
  // Example: "e,38.5,36.175 38.5,86.799 38.5,75.163 ..."
  const tokens = pos.trim().split(/\s+/).filter(Boolean);
  const points: Point[] = [];

  for (const token of tokens) {
    const body = token.startsWith('e,') || token.startsWith('s,') ? token.slice(2) : token;
    const [xs, ys] = body.split(',');
    const x = Number.parseFloat(xs);
    const y = Number.parseFloat(ys);
    if (Number.isFinite(x) && Number.isFinite(y)) points.push([x, y]);
  }

  if (!points.length) return '';
  const toPt = makeToPt(transform);
  // Use the same smoothing conversion as draw ops: convert the point sequence
  // into a visually smooth cubic Bézier path (Catmull‑Rom -> Bézier).
  return bsplinePointsToPath(points, toPt);
}

function decodeLayout(nodes: FlowNode[], edges: FlowEdge[], json: GraphvizJSON): [FlowNode[], FlowEdge[]] {
  const edgeMap: Map<string, FlowEdge> = new Map(edges.map(e => [e.id, e]));
  const nodeMap: Map<string, FlowNode> = new Map(nodes.map(n => [n.id, n]));
  if (!json.objects || !json.edges)
    return [nodes, edges];

  let yMax: number | undefined;
  if (typeof json.bb === 'string') {
    const parts = json.bb.split(',').map(x => Number.parseFloat(x));
    if (parts.length === 4 && Number.isFinite(parts[3])) {
      yMax = parts[3];
    }
  }
  const transform = typeof yMax === 'number'
    ? ((p: Point) => [p[0], yMax! - p[1]] as Point)
    : undefined;

  const gvidToName = new Map<number, string>();
  for (const obj of json.objects) {
    if (typeof obj._gvid === 'number') {
      gvidToName.set(obj._gvid, obj.name);
    }

    const nodeName = obj.name;
    const node = nodeMap.get(nodeName);
    if (node && obj.pos) {
      const [xStr, yStr] = (obj.pos as string).split(',');
      const x = parseFloat(xStr);
      const rawY = parseFloat(yStr);
      const y = typeof yMax === 'number' ? (yMax - rawY) : rawY;
      // Graphviz reports node position as the center; convert to top-left by
      // subtracting half of the node's width/height. Graphviz width/height are
      // in inches; convert to pixels (assume 96 DPI). If width/height are
      // missing, fall back to reasonable defaults.
      const DPI = 96; // pixels per inch
      const widthIn = obj.width ? parseFloat(obj.width as string) : undefined;
      const heightIn = obj.height ? parseFloat(obj.height as string) : undefined;
      const w = typeof widthIn === 'number' && !Number.isNaN(widthIn) ? widthIn * DPI : (node.width ?? 64);
      const h = typeof heightIn === 'number' && !Number.isNaN(heightIn) ? heightIn * DPI : (node.height ?? 64);
      node.position = { x: x - w / 2, y: y - h / 2 };
      // store computed size for downstream consumers (if they reference it)
      if (!node.data) node.data = {} as any;
    }
  }

  const fallbackEdgeByEndpoints = (source?: string, target?: string): FlowEdge | undefined => {
    if (!source || !target) return undefined;
    return edges.find(e => e.source === source && e.target === target);
  };

  for (const e of json.edges) {
    const source = gvidToName.get(e.tail);
    const target = gvidToName.get(e.head);

    const textOps = extractTextOps(e._ldraw_, transform);
    const edgeIdFromAttr = typeof e.remusys_edge_id === 'string' ? e.remusys_edge_id : '';
    const labelFromDraw = textOps.map(t => t.text).join(' ').trim();
    const labelText = (typeof e.label === 'string' ? e.label : '') || labelFromDraw;

    const edgeById = edgeIdFromAttr
      ? edgeMap.get(edgeIdFromAttr)
      : (typeof e._gvid === 'number' ? edgeMap.get(String(e._gvid)) : undefined);
    const edge = edgeById ?? fallbackEdgeByEndpoints(source, target);
    if (!edge) continue;

    const mainPaths = drawOpsToSvgPaths(e._draw_, transform);
    const fallbackMainPath = parseEdgePosToPath(typeof e.pos === 'string' ? e.pos : undefined, transform);
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
      mainPaths: mainPaths.length ? mainPaths : (fallbackMainPath ? [fallbackMainPath] : []),
      arrowPaths,
      labelX: labelPos?.x ?? edge.data?.labelX ?? 0,
      labelY: labelPos?.y ?? edge.data?.labelY ?? 0,
    };
  }
  return [nodes, edges];
}
