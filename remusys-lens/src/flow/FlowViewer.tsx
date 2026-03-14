import { Background, Controls, ReactFlow } from "@xyflow/react";
import { ReactFlowProvider } from "@xyflow/react";
import React, { useCallback, useEffect } from "react";
import { ModuleCache, useIRStore, type FocusSourceInfo } from "../ir/ir-state";
import type { BlockID, GlobalID, JumpTargetID, ValueDt } from "../ir/ir";
import { FlowEdgeTypes, type FlowEdge } from "./components/Edge";
import { FlowNodeTypes, type FlowNode } from "./components/Node";
import { renderCfgOfFunc } from "./graphs/cfg";
import { renderDominanceOfFunc } from "./graphs/dominance";
import { renderDfgFromCentered, renderDfgInsideBlock } from "./graphs/dfg";
import "./FlowViewer.css";
import { useFlowStore } from "./flow-stat";
import { FlowToast } from "./components/Toast";

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
function getFocusEdge(focus: FocusSourceInfo | null): JumpTargetID | null {
  const id = focus?.id;
  if (!id || id.type !== "JumpTarget") return null;
  return id.value;
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
        const focusEdge = getFocusEdge(focus);
        return (
          (await renderCfgOfFunc(module, scopeFunc, focusBB, focusEdge)) ?? [
            [],
            [],
          ]
        );
      }
      case "FuncCfg": {
        const focusBB = focus ? getFocusBlock(module, focus) : null;
        const focusEdge = getFocusEdge(focus);
        return (
          (await renderCfgOfFunc(module, graph.func, focusBB, focusEdge)) ?? [
            [],
            [],
          ]
        );
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

export function FlowGraph({ compId }: FlowGraphProps) {
  const [nodes, setNodes] = React.useState<FlowNode[]>([]);
  const [edges, setEdges] = React.useState<FlowEdge[]>([]);
  const irStore = useIRStore();
  const graph = useFlowStore((store) => store.graphType);

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

  const onNodeDoubleClick = useCallback(
    (event: React.MouseEvent, node: FlowNode) => {
      event.preventDefault();
      if (!node.data?.irObjID) return;
      irStore.focusOn(node.data.irObjID);
    },
    [irStore],
  );
  const onEdgeDoubleClick = useCallback(
    (event: React.MouseEvent, edge: FlowEdge) => {
      event.preventDefault();
      if (!edge.data?.irObjID) return;
      irStore.focusOn(edge.data.irObjID);
    },
    [irStore],
  );

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
        onNodeDoubleClick={onNodeDoubleClick}
        onEdgeDoubleClick={onEdgeDoubleClick}
      >
        <Background id={`${compId}-background`} />
        <Controls />
      </ReactFlow>
    </ReactFlowProvider>
  );
}

export default function FlowViewer() {
  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <FlowGraph compId="flowViewerBottom" />
      <FlowToast />
    </div>
  );
}
