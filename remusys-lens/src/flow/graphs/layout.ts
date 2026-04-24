import type { FlowEdge } from "../Edge";
import type { FlowElemNode, FlowNode } from "../Node";
import Dagre from "dagre";
import * as D3Shape from "d3-shape";

export type FlowGraph = {
    nodes: FlowNode[];
    edges: FlowEdge[];
}
export type SimpleFlowGraph = {
    nodes: FlowElemNode[];
    edges: FlowEdge[];
}

/**
 * 针对单层无子图的图进行布局, 使用 dagre 完成结点布局和边路由, 之后再用 d3-shape 来生成边的路径字符串.
 * 这个函数会直接修改传入的 nodes 和 edges 数组, 给每个 node 添加 position 属性, 给每个 edge 的 data 添加 path 属性.
 *
 * @param nodes 传入的结点数组, 这个函数会直接修改每个 node 对象, 给它添加 position 属性.
 * @param edges 传入的边数组, 这个函数会直接修改每个 edge 对象, 给它的 data 添加 path 属性.
 *              Remusys-lens 需要支持重边, 所以结点的 ID 需要填好.
 */
export function dagreLayoutFlow(nodes: FlowElemNode[], edges: FlowEdge[]) {
    const g = new Dagre.graphlib.Graph({
        multigraph: true,
    });
    g.setGraph({ rankdir: "TB", nodesep: 24, ranksep: 56 });
    g.setDefaultEdgeLabel(() => ({}));

    for (const node of nodes) {
        g.setNode(node.id, {
            width: node.width || 240,
            height: node.height || 52
        });
    }
    for (const edge of edges) {
        let label = edge.label;
        if (typeof label === "object") {
            label = "";
        }
        g.setEdge(edge.source, edge.target, { label }, edge.id);
    }
    Dagre.layout(g);

    for (const node of nodes) {
        const { x, y } = g.node(node.id);
        const width = node.width || 240;
        const height = node.height || 52;
        node.position = {
            x: x - width / 2,
            y: y - height / 2,
        };
    }
    for (const edge of edges) {
        const gEdge = g.edge(edge.source, edge.target, edge.id);
        const points = gEdge?.points ?? [];
        if (!edge.data) {
            throw new Error("Edge data is required for layout");
        }
        if (points.length === 0) {
            edge.data.path = "";
            edge.data.labelPosition = { x: 0, y: 0 };
            continue;
        }

        const fallbackPoint = points[Math.floor(points.length / 2)] || { x: 0, y: 0 };
        const labelPos = gEdge?.labelPos ?? fallbackPoint;
        edge.data.path = D3Shape.line<{ x: number; y: number }>()
            .x(d => d.x)
            .y(d => d.y)
            .curve(D3Shape.curveBasis)(points) || "";
        edge.data.labelPosition = {
            x: labelPos.x,
            y: labelPos.y,
        };
    }
}
