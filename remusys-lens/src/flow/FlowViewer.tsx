import { Background, Controls, ReactFlow } from "@xyflow/react";
import { ReactFlowProvider } from "@xyflow/react";
import '@xyflow/react/dist/style.css'
import React, { useCallback, useEffect } from "react";
import { ModuleCache, useIRStore, type FocusSourceInfo } from "../ir/ir-state";
import type { BlockID, GlobalID } from "../ir/ir";
import { FlowEdgeTypes, type FlowEdge } from "./components/Edge";
import { FlowNodeTypes, type FlowNode } from "./components/Node";
import { renderCfgOfFunc } from "./cfg";
import { renderDominanceOfFunc } from "./dominance";

const fallBackNodes: FlowNode[] = [{
  id: 'module:empty',
  position: { x: 0, y: 0 },
  type: 'flowNode',
  data: {
    label: 'No function selected',
    bgColor: '#fef3c7',
    irObjID: null,
    focused: false,
  }
}];

export type FlowViewStat =
  | { type: 'Empty' }
  | { type: 'ShowFocusCfg' }
  | { type: 'ShowFuncCfg', func: GlobalID }
  | { type: 'ShowFuncDom', func: GlobalID }
  ;

export type FlowViewerProps = {
  stat: FlowViewStat;
};

function getFocusBlock(module: ModuleCache, focus: FocusSourceInfo): BlockID | null {
  let id = focus.id;
  if (!id)
    return null;
  if ("Block" in id)
    return id.Block;
  if ("Inst" in id) {
    let inst = module.loadInst(id.Inst);
    if (!inst)
      return null;
    return inst.parent;
  }
  return null;
}

export default function FlowViewer({ stat }: FlowViewerProps) {
  const [nodes, setNodes] = React.useState<FlowNode[]>([])
  const [edges, setEdges] = React.useState<FlowEdge[]>([])
  const irStore = useIRStore();

  const renderFuncCfg = useCallback(async (func: GlobalID, focusBB: BlockID | null) => {
    if (!irStore.module)
      throw new Error("IR module is not loaded");

    const [nodes, edges] = await renderCfgOfFunc(irStore.module, func, focusBB) ?? [[], []];
    setNodes(nodes);
    setEdges(edges);
  }, [irStore, setNodes, setEdges]);
  const renderFuncDom = useCallback(async (func: GlobalID) => {
    if (!irStore.module)
      throw new Error("IR module is not loaded");

    const [nodes, edges] = await renderDominanceOfFunc(func) ?? [[], []];
    setNodes(nodes);
    setEdges(edges);
  }, [irStore, setNodes, setEdges]);
  const selectStat = useCallback(() => {
    const focus = irStore.focusInfo;
    if (!irStore.module) {
      setNodes(fallBackNodes);
      setEdges([]);
      return;
    }
    switch (stat.type) {
      case 'Empty':
        setNodes(fallBackNodes);
        setEdges([]);
        break;
      case 'ShowFocusCfg': {
        if (!focus) {
          setNodes(fallBackNodes);
          setEdges([]);
          return;
        }
        let scopeFunc = focus?.scopeId;
        if (!scopeFunc) {
          setNodes(fallBackNodes);
          setEdges([]);
          return;
        }
        let focusBB = getFocusBlock(irStore.module, focus);
        renderFuncCfg(scopeFunc, focusBB).catch(err => {
          console.error("Failed to render CFG:", err);
          setNodes(fallBackNodes);
          setEdges([]);
        });
        break;
      }
      case 'ShowFuncCfg': {
        const focusBB = focus ? getFocusBlock(irStore.module, focus) : null;
        renderFuncCfg(stat.func, focusBB).catch(err => {
          console.error("Failed to render CFG:", err);
          setNodes(fallBackNodes);
          setEdges([]);
        });
        break;
      }
      case 'ShowFuncDom':
        renderFuncDom(stat.func).catch(err => {
          console.error("Failed to render dominance:", err);
          setNodes(fallBackNodes);
          setEdges([]);
        });
        break;
    }
  }, [stat, irStore, renderFuncCfg, renderFuncDom]);

  useEffect(() => { selectStat(); }, [selectStat]);

  return (
    <ReactFlowProvider>
      <ReactFlow
        nodeTypes={FlowNodeTypes} edgeTypes={FlowEdgeTypes} fitView
        nodes={nodes} edges={edges}
      >
        <Background id="FlowViewerBasic" />
        <Controls />
      </ReactFlow>
    </ReactFlowProvider>
  );
}
