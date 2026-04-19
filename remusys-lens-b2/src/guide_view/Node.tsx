import type { IRTreeNodeClass, IRTreeObjID } from "../ir/types";
import * as XYFlow from "@xyflow/react";
import * as Dagre from "dagre";

export type GuideNodeBase = {
    id: string;
    irObject: IRTreeObjID;
    label: string;
    kind: IRTreeNodeClass;
    focused: boolean;
    parent?: GuideNodeBase;
}
export type GuideNodeExpand = GuideNodeBase & {
    children: GuideNodeBase[]; // rendered node 一定有 children, 因为没有 children 的 node 不会被渲染.
};
export type GuideNodeItem = GuideNodeBase & {
    children?: undefined; // menu item 一定没有 children, 因为只有被展开的 node 才会有 children.
};
export type GuideNodeData = GuideNodeExpand | GuideNodeItem;

export function guideNodeExpanded(data: GuideNodeData): boolean {
    return data.children !== undefined;
}
export function guideNodeSize(data: GuideNodeExpand): { width: number; height: number } {
    const header_height = 52;
    const item_height = 41;
    const width = 240;
    const height = header_height + data.children.length * item_height;
    return { width, height };
}

export function collectGuideTree(root: GuideNodeData): [GuideRFNode[], XYFlow.Edge[]] {
    const nodes: GuideRFNode[] = [];
    const edges: XYFlow.Edge[] = [];

    function dfs(node: GuideNodeData) {
        if (!node.children) {
            // node that is not expanded, do not add to nodes list.
            return;
        }
        const { width, height } = guideNodeSize(node);
        nodes.push({
            id: node.id,
            type: "GuideNode",
            data: node,
            position: { x: 0, y: 0 },
            width,
            height,
        });
        for (const child of node.children) {
            const edgeId = `${node.id}_${child.id}`;
            edges.push({
                id: edgeId,
                source: node.id,
                target: child.id,
                type: "default",
                markerEnd: { type: "arrowclosed" },
            });
            dfs(child);
        }
    }
    dfs(root);
    dagreLayoutGuideTree(nodes, edges);
    return [nodes, edges];
}

function dagreLayoutGuideTree(nodes: GuideRFNode[], edges: XYFlow.Edge[]) {
    const g = new Dagre.graphlib.Graph();
    g.setGraph({ rankdir: "LR", nodesep: 50, ranksep: 80 });
    g.setDefaultEdgeLabel(() => ({}));

    for (const node of nodes) {
        g.setNode(node.id, {
            width: node.width || 240,
            height: node.height || 52
        });
    }
    for (const edge of edges) {
        g.setEdge(edge.source, edge.target);
    }
    Dagre.layout(g);

    for (const node of nodes) {
        const { x, y } = g.node(node.id);
        node.position = { x, y };
    }
}

export type GuideRFNode = XYFlow.Node<GuideNodeExpand, "GuideNode">;
export type GuideNodeProps = XYFlow.NodeProps<GuideRFNode>;

export default function GuideViewNode(props: GuideNodeProps) {
}
