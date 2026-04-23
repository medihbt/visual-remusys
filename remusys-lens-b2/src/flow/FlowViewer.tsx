/**
 * # FlowViewer -- 流图视图
 *
 * 查看焦点处或者其他选点的流图。
 * 
 * ## 支持什么流图
 * 
 * 支持的流图类型参见 `state.ts` 中的 `GraphType` 定义，目前包括：
 * 
 * - 空图（Empty）
 * - 错误图（Error）：不是正经流图, 是项目出错了, 刚好有个界面可以显示错误信息。
 * - 焦点图（Focus）：不是某个特定的流图，而是根据当前焦点的类型自动选择一个图来显示。
 * - 调用图（CallGraph）：显示函数之间的调用关系。
 *   Focus 的备选之一, 当焦点为全局变量、外部函数、模块时, Focus 图会自动切换到调用图。
 * - 函数控制流图（FuncCfg）：显示一个函数内部的基本块和它们之间的控制流关系。
 *   Focus 的备选之一, 当焦点在函数定义及以下时, Focus 图会自动切换到函数控制流图。
 * - 函数支配树（FuncDom）：显示一个函数内部的基本块和它们之间的支配关系。
 *   与 Focus 无关, 任何情况下都不会自动切换, 需要通过菜单手动切换
 * - 基本块数据流图（BlockDfg）：显示一个基本块内部的指令和它们之间的数据流关系。
 *   与 Focus 无关, 任何情况下都不会自动切换, 需要通过菜单手动切换
 * - 定义-使用链（DefUse）：以某条指令为中心，显示与它相关的定义-使用关系。
 *   与 Focus 无关, 任何情况下都不会自动切换, 需要通过菜单手动切换
 * 
 * ## 数据来源在哪儿
 * 
 * 目前的设计是，FlowViewer 直接从 IRStore 获取数据， IRStore 直接调用 WASM API
 * 获取数据并进行必要的转换。这样可以不用维护复杂的前端缓存, 美哉
 * 
 * ## 怎么排版
 * 
 * 除了 BlockDfg 之外, 其他图都是没有子图的单层图, 所以使用 dagre 做结点排版+边路由.
 * 相比 GraphViz, dagre 的 API 更加清晰，更容易维护.
 * 
 * BlockDfg 是 Section-Node 双层图, 有子图结构, 因此使用 Elk.js 来排版. Elkjs 比较
 * 复杂，但至少不像 GraphViz 那样接口模糊不清，而且 Elkjs 能排带子图的图, dagre 不行.
 * 
 * ## 交互
 * 
 * 目前的交互比较简单，双击结点时会尝试聚焦到这个结点对应的 IR 实体上（如果有的话）。
 * 双击边时会尝试聚焦到这个边对应的 IR 实体上（如果有的话）。更多功能等待未来开发。
 */

import { Background, Controls, MarkerType, ReactFlow, ReactFlowProvider } from "@xyflow/react";
import { useCallback, useEffect, useMemo, useState } from "react";

import type { IRObjPath, IRTreeObjID, ModuleInfo } from "remusys-wasm-b2";

import { useIRStore, type IRState } from "../ir/state";
import { flowEdgeTypes, type FlowEdge } from "./Edge";
import { FlowNodeTypes, type FlowNode } from "./Node";
import FlowToolbar from "./Toolbar";
import { getBlockDfg } from "./graphs/block-dfg";
import { getCallGraph } from "./graphs/call-graph";
import { getFuncCfg } from "./graphs/cfg";
import { getDefUseGraph } from "./graphs/defuse-graph";
import { getFuncDominance } from "./graphs/dominance";
import { useGraphState, type GraphType } from "./state";

function infoGraph(message: string): { nodes: FlowNode[]; edges: FlowEdge[] } {
  return {
    nodes: [
      {
        id: `flow-info:${message}`,
        type: "elemNode",
        position: { x: 0, y: 0 },
        width: 240,
        height: 52,
        data: {
          label: message,
          focused: false,
          irObjID: null,
          bgColor: "#f8fafc",
        },
      },
    ],
    edges: [],
  };
}

function errorGraph(message: string, details?: string): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const text = details ? `${message}\n${details}` : message;
  return {
    nodes: [
      {
        id: `flow-error:${message}`,
        type: "elemNode",
        position: { x: 0, y: 0 },
        width: 280,
        height: 64,
        data: {
          label: text,
          focused: false,
          irObjID: null,
          bgColor: "#fee2e2",
        },
      },
    ],
    edges: [],
  };
}

async function resolveGraph(irState: IRState, graphType: GraphType): Promise<{ nodes: FlowNode[]; edges: FlowEdge[] }> {
  if (!irState.module) {
    return infoGraph("No module loaded");
  }

  if (graphType.type === "Error") {
    return errorGraph(`Error: ${graphType.message}`);
  }

  if (graphType.type === "Empty") {
    return infoGraph("No function selected");
  }

  if (graphType.type === "CallGraph") {
    return getCallGraph(irState);
  }

  if (graphType.type === "FuncCfg") {
    return getFuncCfg(irState, graphType.func);
  }

  if (graphType.type === "BlockDfg") {
    return await getBlockDfg(irState, graphType.block);
  }

  if (graphType.type === "FuncDom") {
    return getFuncDominance(irState, graphType.func);
  }

  if (graphType.type === "DefUse") {
    return getDefUseGraph(irState, graphType.center);
  }

  return infoGraph("Unsupported graph type");
}

function objectToPath(module: ModuleInfo, obj: IRTreeObjID): IRObjPath {
  return module.path_of_tree_object(obj);
}

export default function FlowViewer() {
  const irState = useIRStore();
  const graphStore = useGraphState();

  const [nodes, setNodes] = useState<FlowNode[]>([]);
  const [edges, setEdges] = useState<FlowEdge[]>([]);

  const resolvedGraphType = useMemo(() => graphStore.getRealGraphType(irState), [graphStore, irState]);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const graph = await resolveGraph(irState, resolvedGraphType);
        if (!alive) return;
        setNodes(graph.nodes);
        setEdges(graph.edges);
      } catch (error) {
        if (!alive) return;
        const message = error instanceof Error ? error.message : String(error);
        const stack = error instanceof Error ? error.stack : undefined;
        // console.error("flow render failed", { error, resolvedGraphType });
        const graph = errorGraph(`Render failed: ${message}`, stack);
        setNodes(graph.nodes);
        setEdges(graph.edges);
        throw error;
      }
    })();

    return () => {
      alive = false;
    };
  }, [irState, resolvedGraphType]);

  const onNodeDoubleClick = useCallback((event: React.MouseEvent, node: FlowNode) => {
    event.preventDefault();
    if (!node.data?.irObjID || !irState.module) return;
    try {
      irState.setFocus(objectToPath(irState.module, node.data.irObjID));
    } catch (error) {
      console.error("node focus failed", error);
    }
  }, [irState]);

  const onEdgeDoubleClick = useCallback((event: React.MouseEvent, edge: FlowEdge) => {
    event.preventDefault();
    if (!edge.data?.irObjID || !irState.module) return;
    try {
      irState.setFocus(objectToPath(irState.module, edge.data.irObjID));
    } catch (error) {
      console.error("edge focus failed", error);
    }
  }, [irState]);

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <ReactFlowProvider>
        <ReactFlow
          fitView
          nodes={nodes}
          edges={edges}
          defaultEdgeOptions={{
            markerEnd: {
              type: MarkerType.ArrowClosed,
              width: 18,
              height: 18,
              color: "#334155",
            },
          }}
          nodeTypes={FlowNodeTypes}
          edgeTypes={flowEdgeTypes}
          onNodeDoubleClick={onNodeDoubleClick}
          onEdgeDoubleClick={onEdgeDoubleClick}
        >
          <Background />
          <Controls />
        </ReactFlow>
      </ReactFlowProvider>
      <FlowToolbar />
    </div>
  );
}
