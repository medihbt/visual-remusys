import type {
	BlockID,
	CfgEdgeDfsRole,
	CfgEdgeDt,
	CfgNodeDt,
	GlobalID,
	IRObjPath,
	JumpTargetID,
	JumpTargetKind,
} from "remusys-wasm";

import type { IRState } from "../../ir/state";
import type { FlowEdge } from "../Edge";
import type { FlowElemNode } from "../Node";
import { dagreLayoutFlow, type FlowGraph } from "./layout";

type FocusTarget = {
	block: BlockID | null;
	edge: JumpTargetID | null;
};

function extractFocusTarget(focusPath: IRObjPath): FocusTarget {
	const current = focusPath[focusPath.length - 1];
	if (!current || current.type === "Module") {
		return { block: null, edge: null };
	}
	if (current.type === "Block") {
		return { block: current.value, edge: null };
	}
	if (current.type === "Inst") {
		const current = focusPath[focusPath.length - 2];
		if (current && current.type === "Block") {
			return { block: current.value, edge: null };
		} else {
			return { block: null, edge: null };
		}
	}
	if (current.type === "JumpTarget") {
		return { block: null, edge: current.value };
	}
	return { block: null, edge: null };
}

function cfgNodeColor(node: CfgNodeDt): string {
	switch (node.role) {
		case "Entry":
			return "#d1fae5";
		case "Exit":
			return "#fee2e2";
		case "Branch":
			return "#ffffff";
	}
}

function cfgEdgeColor(kind: JumpTargetKind): string {
	if (kind === "BrThen") return "#16a34a";
	if (kind === "BrElse") return "#dc2626";
	if (kind === "SwitchDefault") return "#2563eb";
	if (kind.startsWith("SwitchCase:")) return "#d97706";
	return "#222222";
}

function cfgEdgeDash(role: CfgEdgeDfsRole): string | undefined {
	if (role === "Back") return "6 3";
	if (role === "Forward") return "4 2";
	if (role === "Cross") return "4 4";
	if (role === "SelfRing") return "3 3";
	return undefined;
}

function makeFlowNodes(nodes: CfgNodeDt[], focus: FocusTarget): FlowElemNode[] {
	return nodes.map((node) => ({
		id: node.block,
		position: { x: 0, y: 0 },
		type: "elemNode",
		width: 160,
		height: 50,
		data: {
			label: node.label,
			focused: focus.block === node.block,
			irObjID: { type: "Block", value: node.block },
			bgColor: cfgNodeColor(node),
		},
	}));
}

function makeFlowEdges(edges: CfgEdgeDt[], focus: FocusTarget): FlowEdge[] {
	return edges.map((edge) => {
		const dash = cfgEdgeDash(edge.dfs_role);
		return {
			id: edge.id,
			source: edge.from,
			target: edge.to,
			type: "FlowEdge",
			label: edge.jt_kind,
			style: {
				stroke: cfgEdgeColor(edge.jt_kind),
				strokeDasharray: dash,
				strokeWidth: edge.id === focus.edge ? 1.5 : 1,
			},
			labelStyle: {
				fill: edge.id === focus.edge ? "#222222" : "#888888",
			},
			data: {
				path: "",
				labelPosition: { x: 0, y: 0 },
				isFocused: focus.edge === edge.id,
				irObjID: { type: "JumpTarget", value: edge.id },
			},
		};
	});
}

export function getFuncCfg(irState: IRState, funcID: GlobalID): FlowGraph {
	const dto = irState.getFuncCfg(funcID);
	const focus = extractFocusTarget(irState.focus);
	const nodes = makeFlowNodes(dto.nodes, focus);
	const edges = makeFlowEdges(dto.edges, focus);
	dagreLayoutFlow(nodes, edges);
	return { nodes, edges };
}
