import type {
    DefUseGraph,
    DfgEdge,
    DfgNode,
    DfgNodeRole,
    IRObjPath,
    IRTreeObjID,
    InstID,
} from "remusys-wasm";

import type { IRState } from "../../ir/state";
import type { FlowEdge } from "../Edge";
import type { FlowElemNode } from "../Node";
import { dagreLayoutFlow, type FlowGraph } from "./layout";

function nodeColorByRole(role: DfgNodeRole): string {
    switch (role) {
        case "Income":
            return "#dbeafe";
        case "Outgo":
            return "#dcfce7";
        case "Phi":
            return "#fef3c7";
        case "Pure":
            return "#f3f4f6";
        case "Effect":
            return "#fee2e2";
        case "Terminator":
            return "#ffedd5";
    }
}

function dfgNodeToIRObjID(node: DfgNode): IRTreeObjID | null {
    switch (node.value.type) {
        case "Global":
            return { type: "Global", value: node.value.value };
        case "FuncArg":
            return { type: "FuncArg", value: node.value.value };
        case "Block":
            return { type: "Block", value: node.value.value };
        case "Inst":
            return { type: "Inst", value: node.value.value };
        default:
            return null;
    }
}

function edgeToIRObjID(edge: DfgEdge): IRTreeObjID {
    return { type: "Use", value: edge.id };
}

function focusMatchesIRObject(focus: IRObjPath, obj: IRTreeObjID | null): boolean {
    if (!obj || focus.length === 0) return false;
    const current = focus[focus.length - 1];
    if (current.type !== obj.type) return false;
    if (current.type === "Module" || obj.type === "Module") return true;
    return current.value === obj.value;
}

function makeNodes(defUse: DefUseGraph, focus: IRObjPath, center: InstID): FlowElemNode[] {
    return defUse.nodes.map((node) => {
        const nodeObj = dfgNodeToIRObjID(node);
        const isCenter = node.value.type === "Inst" && node.value.value === center;
        return {
            id: node.id,
            type: "elemNode",
            position: { x: 0, y: 0 },
            width: 190,
            height: 52,
            data: {
                label: node.label,
                focused: isCenter || focusMatchesIRObject(focus, nodeObj),
                irObjID: nodeObj,
                bgColor: isCenter ? "#fecaca" : nodeColorByRole(node.role),
            },
        };
    });
}

function makeEdges(defUse: DefUseGraph, focus: IRObjPath): FlowEdge[] {
    return defUse.edges.map((edge) => ({
        id: edge.id,
        source: edge.from,
        target: edge.to,
        type: "FlowEdge",
        label: edge.label,
        style: {
            stroke: "#334155",
            strokeWidth: 1.2,
        },
        data: {
            path: "",
            labelPosition: { x: 0, y: 0 },
            isFocused: focusMatchesIRObject(focus, edgeToIRObjID(edge)),
            irObjID: edgeToIRObjID(edge),
        },
    }));
}

export function getDefUseGraph(irState: IRState, center: InstID): FlowGraph {
    const defUse = irState.getDefUseGraph(center);
    const nodes = makeNodes(defUse, irState.focus, center);
    const edges = makeEdges(defUse, irState.focus);
    dagreLayoutFlow(nodes, edges);
    return { nodes, edges };
}
