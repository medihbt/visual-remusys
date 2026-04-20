import * as XYFlow from "@xyflow/react";
import * as Dagre from "dagre";
import { ChildRow } from "./ChildRow";
import { TypeIcon } from "./TypeIcon";
import { useGuideViewTreeStore } from "./guide-view-tree";
import type { GuideNodeData, GuideNodeExpand } from "remusys-wasm-b2";

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
    const { data } = props;
    const { expand, collapse, requestFocus } = useGuideViewTreeStore();

    function tryFocusNode(node: GuideNodeData) {
        requestFocus(node);
    }

    function handleToggle(child: GuideNodeData) {
        if (guideNodeExpanded(child)) {
            collapse(child);
            return;
        }
        expand(child);
    }

    const isFocused = data.focusClass === "FocusNode";

    return (
        <div style={{ width: "100%", height: "100%", position: "relative" }}>
            <XYFlow.Handle type="target" position={XYFlow.Position.Left} style={{ opacity: 0.5 }} />

            <div
                style={{
                    width: "100%",
                    height: "100%",
                    border: "1px solid #d1d5db",
                    borderRadius: "4px",
                    backgroundColor: "#fff",
                    boxShadow: "0 2px 4px rgba(0,0,0,0.05)",
                    display: "flex",
                    flexDirection: "column",
                    overflow: "hidden",
                    fontFamily: "system-ui, sans-serif",
                }}
            >
                <div
                    onDoubleClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        tryFocusNode(data);
                    }}
                    style={{
                        display: "flex",
                        alignItems: "center",
                        padding: "8px 12px",
                        backgroundColor: isFocused ? "#eef2ff" : "#f9fafb",
                        borderBottom: "1px solid #e5e7eb",
                        cursor: "pointer",
                        userSelect: "none",
                    }}
                >
                    <div
                        style={{
                            marginRight: "8px",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                        }}
                    >
                        {isFocused ? (
                            <div
                                style={{
                                    borderRadius: 9999,
                                    padding: 4,
                                    border: "2px solid #60a5fa",
                                    display: "inline-flex",
                                }}
                            >
                                <TypeIcon kind={data.kind} />
                            </div>
                        ) : (
                            <TypeIcon kind={data.kind} />
                        )}
                    </div>
                    <div
                        style={{
                            flex: 1,
                            fontSize: "13px",
                            fontWeight: 600,
                            color: "#1f2937",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                        }}
                    >
                        {data.label.trim() === "" ? "(no name)" : data.label}
                    </div>
                </div>

                <div style={{ overflowY: "auto", height: "100%" }}>
                    {data.children.map((child) => (
                        <ChildRow key={child.id} child={child} onToggle={handleToggle} />
                    ))}
                    {data.children.length === 0 && (
                        <div
                            style={{
                                padding: "8px",
                                fontSize: "11px",
                                color: "#9ca3af",
                                textAlign: "center",
                            }}
                        >
                            (无子节点)
                        </div>
                    )}
                </div>
            </div>

            <XYFlow.Handle
                type="source"
                position={XYFlow.Position.Right}
                style={{ opacity: 0.5 }}
            />
        </div>
    );
}
