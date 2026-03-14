import { Background, Controls, ReactFlow } from "@xyflow/react";
import { ReactFlowProvider } from "@xyflow/react";
import React, { useCallback, useEffect } from "react";
import { ModuleCache, useIRStore, type FocusSourceInfo } from "../ir/ir-state";
import type { BlockID, GlobalID, ValueDt } from "../ir/ir";
import { FlowEdgeTypes, type FlowEdge } from "./components/Edge";
import { FlowNodeTypes, type FlowNode } from "./components/Node";
import { renderCfgOfFunc } from "./graphs/cfg";
import { renderDominanceOfFunc } from "./graphs/dominance";
import { renderDfgFromCentered, renderDfgInsideBlock } from "./graphs/dfg";
import "./FlowViewer.css";

const noFuncSelectNodes: FlowNode[] = [
  {
    id: "module:empty",
    position: { x: 0, y: 0 },
    type: "elemNode",
    data: {
      label: "No function selected",
      bgColor: "#fef3c7",
      irObjID: null,
      focused: false,
    },
  },
];
function showErrorNodes(message: string, sourceLoc?: string): FlowNode[] {
  let label = `Error: ${message}`;
  if (sourceLoc) {
    label += `\nSource: ${sourceLoc}`;
  }
  return [
    {
      id: "module:error",
      position: { x: 0, y: 0 },
      type: "elemNode",
      data: {
        label,
        bgColor: "#fee2e2",
        irObjID: null,
        focused: false,
      },
    },
  ];
}
function todoNodes(feature: string): FlowNode[] {
  return [
    {
      id: "module:todo",
      position: { x: 0, y: 0 },
      type: "elemNode",
      data: {
        label: `TODO: ${feature} not implemented`,
        bgColor: "#e0e0e0",
        irObjID: null,
        focused: false,
      },
    },
  ];
}

export type FlowGraphType =
  | { type: "Empty" }
  | { type: "Focus" }
  | { type: "CallGraph" }
  | { type: "ItemReference"; item: GlobalID }
  | { type: "FuncCfg"; func: GlobalID }
  | { type: "FuncDom"; func: GlobalID }
  | { type: "BlockDfg"; block: BlockID }
  | { type: "DefUse"; center: ValueDt };

export type FlowGraphProps = {
  graph: FlowGraphType;
  compId: string;
};

function getFocusBlock(
  module: ModuleCache,
  focus: FocusSourceInfo,
): BlockID | null {
  const id = focus.id;
  if (!id) return null;
  switch (id.type) {
    case "Block":
      return id.value;
    case "Inst": {
      const inst = module.loadInst(id.value);
      if (!inst) return null;
      return inst.parent;
    }
    default:
      return null;
  }
}

async function renderGraph(
  module: ModuleCache,
  graph: FlowGraphType,
  focus: FocusSourceInfo | null,
): Promise<[FlowNode[], FlowEdge[]]> {
  try {
    switch (graph.type) {
      case "Empty":
        return [noFuncSelectNodes, []];
      case "CallGraph": {
        return [todoNodes("CallGraph"), []];
      }
      case "ItemReference": {
        return [todoNodes("ItemReference"), []];
      }
      case "Focus": {
        if (!focus) return [noFuncSelectNodes, []];
        const scopeFunc = focus?.scopeId;
        if (!scopeFunc) {
          return [todoNodes("Focus CallGraph"), []];
        }
        const focusBB = getFocusBlock(module, focus);
        return (await renderCfgOfFunc(module, scopeFunc, focusBB)) ?? [[], []];
      }
      case "FuncCfg": {
        const focusBB = focus ? getFocusBlock(module, focus) : null;
        return (await renderCfgOfFunc(module, graph.func, focusBB)) ?? [[], []];
      }
      case "FuncDom": {
        const focusBB = focus ? getFocusBlock(module, focus) : null;
        return (
          (await renderDominanceOfFunc(module, focusBB, graph.func)) ?? [[], []]
        );
      }
      case "BlockDfg":
        return await renderDfgInsideBlock(graph.block, module);
      case "DefUse":
        return await renderDfgFromCentered(graph.center, module);
      default:
        return [noFuncSelectNodes, []];
    }
  } catch (err) {
    console.error("Failed to render graph:", err);
    let msg: string;
    let sourceLoc: string | undefined;
    if (err instanceof Error) {
      msg = err.message;
      sourceLoc = err.stack;
    } else {
      msg = String(err);
    }
    return [showErrorNodes(msg, sourceLoc), []];
  }
}

export function FlowGraph({ graph, compId }: FlowGraphProps) {
  const [nodes, setNodes] = React.useState<FlowNode[]>([]);
  const [edges, setEdges] = React.useState<FlowEdge[]>([]);
  const irStore = useIRStore();

  const renderGraphFunc = useCallback(async () => {
    if (!irStore.module) {
      setNodes(noFuncSelectNodes);
      setEdges([]);
      return;
    }
    const focus = irStore.focusInfo;
    const [nodes, edges] = await renderGraph(irStore.module, graph, focus);
    setNodes(nodes);
    setEdges(edges);
  }, [irStore, graph]);

  useEffect(() => {
    renderGraphFunc();
  }, [renderGraphFunc]);

  return (
    <ReactFlowProvider>
      <ReactFlow
        nodeTypes={FlowNodeTypes}
        edgeTypes={FlowEdgeTypes}
        fitView
        nodes={nodes}
        edges={edges}
      >
        <Background id={`${compId}-background`} />
        <Controls />
      </ReactFlow>
    </ReactFlowProvider>
  );
}

export type FlowViewerProps = {
  fgGraph?: FlowGraphType;
};
export default function FlowViewer({ fgGraph }: FlowViewerProps) {
  return (
    <FlowGraph graph={fgGraph ?? { type: "Focus" }} compId="flowViewerBottom" />
  );
}
