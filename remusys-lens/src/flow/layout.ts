import * as Viz from '@viz-js/viz';
import type { GraphvizJSON, DrawOps, Point } from './graphviz-object';

export const CFG_DOT_TEXT: string = `digraph "CFG for 'main'" {
	graph [splines=true];
	node [shape=rectangle, fixedsize=true, width=0.6667, height=0.6667, style=filled, fillcolor="#f8fafc"];
	edge [penwidth=1];

	"entry" [label="entry"];
	"while.cond" [label="while.cond"];
	"while.body" [label="while.body"];
	"exit" [label="exit"];

	"entry" -> "while.cond";
	"while.cond" -> "while.body" [label="true"];
	"while.cond" -> "exit" [label="false"];
	"while.body" -> "while.cond";
}
`;

export type NodeLayout = {
    id: string;
    x: number;
    y: number;
    width: number;
    height: number;
}

export async function layoutCfgDot(dotText: string): Promise<GraphvizJSON> {
    const viz = await Viz.instance();
    const jsonObj = viz.renderJSON(dotText, { format: 'json0' });
    return jsonObj as GraphvizJSON;
}

// --- Graphviz DrawOps -> SVG path utilities ---

type Pt = { x: number; y: number }

// convert Point to Pt with optional transform (e.g. flip Y)
function makeToPt(transform?: (p: Point) => Point) {
    return function toPt(p: Point): Pt {
        const tp = transform ? transform(p) : p
        return { x: tp[0], y: tp[1] }
    }
}

function pointsToPolylinePath(pts: Pt[], close = false): string {
    if (!pts || pts.length === 0) return ''
    const parts = [`M ${pts[0].x} ${pts[0].y}`]
    for (let i = 1; i < pts.length; i++) parts.push(`L ${pts[i].x} ${pts[i].y}`)
    if (close) parts.push('Z')
    return parts.join(' ')
}

// Catmull-Rom -> cubic Bézier conversion (simple, visually smooth)
function catmullRomToBeziers(pts: Pt[]) {
    const beziers: string[] = []
    if (pts.length < 2) return beziers
    const p = [pts[0], ...pts, pts[pts.length - 1]]
    for (let i = 0; i < pts.length - 1; i++) {
        const [p0, p1, p2, p3] = p.slice(i, i + 4)
        const b1x = p1.x + (p2.x - p0.x) / 6
        const b1y = p1.y + (p2.y - p0.y) / 6
        const b2x = p2.x - (p3.x - p1.x) / 6
        const b2y = p2.y - (p3.y - p1.y) / 6
        beziers.push(`C ${b1x} ${b1y} ${b2x} ${b2y} ${p2.x} ${p2.y}`)
    }
    return beziers
}

function bsplinePointsToPath(points: Point[], toPtFn: (p: Point) => Pt) {
    if (!points || points.length === 0) return ''
    const pts = points.map(toPtFn)
    const beziers = catmullRomToBeziers(pts)
    if (!beziers.length) return pointsToPolylinePath(pts)
    return `M ${pts[0].x} ${pts[0].y} ${beziers.join(' ')}`
}

function ellipseToPath(rect: [number, number, number, number]) {
    const [x1, y1, x2, y2] = rect
    const cx = (x1 + x2) / 2
    const cy = (y1 + y2) / 2
    const rx = Math.abs((x2 - x1) / 2)
    const ry = Math.abs((y2 - y1) / 2)
    if (rx === 0 || ry === 0) return ''
    return `M ${cx - rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx + rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx - rx} ${cy}`
}

/**
 * 将 Graphviz DrawOps 转为 SVG path 字符串数组（可能返回多条 path）
 */
export function drawOpsToSvgPaths(ops?: DrawOps, transform?: (p: Point) => Point): string[] {
    if (!ops || ops.length === 0) return []
    const toPt = makeToPt(transform)
    const paths: string[] = []
    for (const op of ops) {
        switch (op.op) {
            case 'b':
            case 'B': {
                const bs = op
                if (bs.points && bs.points.length) {
                    paths.push(bsplinePointsToPath(bs.points, toPt))
                }
                break
            }
            case 'P':
            case 'p': {
                const pg = op
                if (pg.points && pg.points.length) {
                    paths.push(pointsToPolylinePath((pg.points as Point[]).map(toPt), true))
                }
                break
            }
            case 'L': {
                const pl = op
                if (pl.points && pl.points.length) paths.push(pointsToPolylinePath((pl.points as Point[]).map(toPt), false))
                break
            }
            case 'e':
            case 'E': {
                if (op.rect) {
                    // rect = [x1,y1,x2,y2] -> transform both corners
                    const r = op.rect as [number, number, number, number]
                    const p0 = transform ? transform([r[0], r[1]]) : [r[0], r[1]]
                    const p1 = transform ? transform([r[2], r[3]]) : [r[2], r[3]]
                    paths.push(ellipseToPath([p0[0], p0[1], p1[0], p1[1]]))
                }
                break
            }
            default:
                // ignore color/font/style/text ops for path extraction
                break
        }
    }
    return paths
}

export type EdgeLayout = { id: string; source?: string; target?: string; mainPaths: string[]; arrowPaths: string[] }

/**
 * 从 Graphviz JSON 中提取节点和边的布局（节点像素化，边路径为 SVG d 字符串数组）
 */
export function extractLayoutsFromGraphviz(json: GraphvizJSON) {
    const DPI = 96 // 将 Graphviz 英寸单位转换为像素
    const nodes: NodeLayout[] = []
    const edges: EdgeLayout[] = []

    const gvidToName = new Map<number, string>()
        // parse bounding box if present to flip Y
        let yMax: number | undefined = undefined
        if (typeof json.bb === 'string') {
            const parts = (json.bb as string).split(',').map(s => parseFloat(s))
            if (parts.length === 4 && !Number.isNaN(parts[3])) {
                yMax = parts[3]
            }
        }

        if (json.objects) {
            for (const obj of json.objects) {
                const name = obj.name
                const posStr = (obj.pos as string) || ''
                if (!name || !posStr) continue
                const [xs, ys] = posStr.split(',')
                const x = parseFloat(xs)
                const y = parseFloat(ys)
                const w = obj.width ? parseFloat(obj.width as string) * DPI : 64
                const h = obj.height ? parseFloat(obj.height as string) * DPI : 64
                const fy = (typeof yMax === 'number') ? (yMax - y) : y
                nodes.push({ id: name, x, y: fy, width: w, height: h })
                if (typeof obj._gvid === 'number') gvidToName.set(obj._gvid, name)
            }
        }

    if (json.edges) {
        for (const e of json.edges) {
            const id = (e._gvid !== undefined) ? String(e._gvid) : `${e.tail}-${e.head}`
            const main: string[] = []
            const arrows: string[] = []
                const transform = (typeof yMax === 'number') ? ((p: Point) => [p[0], yMax - p[1]] as Point) : undefined
                if (e._draw_) main.push(...drawOpsToSvgPaths(e._draw_, transform))
                if (e._hdraw_) arrows.push(...drawOpsToSvgPaths(e._hdraw_, transform))
                if (e._tdraw_) arrows.push(...drawOpsToSvgPaths(e._tdraw_, transform))
            const source = typeof e.tail === 'number' ? gvidToName.get(e.tail) : undefined
            const target = typeof e.head === 'number' ? gvidToName.get(e.head) : undefined
            edges.push({
                id,
                source,
                target,
                mainPaths: main.filter(Boolean),
                arrowPaths: arrows.filter(Boolean)
            })
        }
    }

    return { nodes, edges }
}
