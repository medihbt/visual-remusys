import React, { useState, useEffect, useCallback, useMemo } from "react";
import { ReactFlow, Background, Controls, useNodesState, useEdgesState } from "@xyflow/react";
import * as dagre from "dagre";
import { GuideNodeComp } from "./components/GuideNodeComp";
import { TreeNodeStorage, type TreeNodeKind, type TreeNodeRef } from "./guide-view-tree";
import type { ModuleCache } from "../ir/ir-state";
import type { GuideRFNode, GuideRFEdge, NavEvent } from "./types";
import { SimpleMenu } from "./components/SimpleMenu";

interface GuideViewProps {
  moduleCache: ModuleCache;
  onNavigate: (event: NavEvent) => void; // 向外暴露导航事件（给代码编辑器、属性面板用）
}

function nodeIdStr(ref: TreeNodeRef) {
  switch (ref.type) {
    case "Module": return "Module";
    case "Block": return ref.block_id;
    case "GlobalObj": return ref.global_id;
    case "Inst": return ref.inst_id;
  }
}

function renderTree(storage: TreeNodeStorage, module: ModuleCache): [GuideRFNode[], GuideRFEdge[]] {
  const { nodes, edges } = storage.export(module);

  // 转换 Nodes (只转换展开的节点)
  const rfNodes: GuideRFNode[] = nodes
    .filter(n => n.expanded)
    .map(n => ({
      id: nodeIdStr(n.treeNode.selfId),
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: n,
      width: 220,
      height: 52 + n.children.length * 41, // 简单高度估算
    }));
  // 转换 Edges
  const rfEdges: GuideRFEdge[] = edges.map(e => ({
    id: e.id,
    source: nodeIdStr(e.source.treeNode.selfId),
    target: nodeIdStr(e.target.treeNode.selfId),
    type: "default",
    markerEnd: { type: 'arrowclosed' }
  }));

  // Dagre 布局 (LR 方向)
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  dagreGraph.setGraph({ rankdir: 'LR', nodesep: 50, ranksep: 80 });

  rfNodes.forEach(node => dagreGraph.setNode(node.id, { width: node.width || 200, height: node.height || 100 }));
  rfEdges.forEach(edge => dagreGraph.setEdge(edge.source, edge.target));
  dagre.layout(dagreGraph);

  const laidNodes = rfNodes.map(node => {
    const { x, y } = dagreGraph.node(node.id);
    return { ...node, position: { x: x - (node.width! / 2), y: y - (node.height! / 2) } };
  });
  return [laidNodes, rfEdges];
}

export const GuideView: React.FC<GuideViewProps> = ({ moduleCache, onNavigate }) => {
  // 1. 状态：唯一数据源
  const [storage, setStorage] = useState<TreeNodeStorage>(() => {
    const s = new TreeNodeStorage(moduleCache.moduleId);
    s.expand({ type: "Module" }, moduleCache); // 默认展开 Module
    return s;
  });

  // 2. ReactFlow 状态
  const [nodes, setNodes] = useNodesState<GuideRFNode>([]);
  const [edges, setEdges] = useEdgesState<GuideRFEdge>([]);

  // 3. 菜单状态 (存储坐标)
  const [menuState, setMenuState] = useState<{
    x: number;
    y: number;
    nodeRef: TreeNodeRef;
    kind: TreeNodeKind;
  } | null>(null);

  // 4. 核心逻辑：当 storage 变化时更新布局
  useEffect(() => {
    const [newNodes, newEdges] = renderTree(storage, moduleCache);
    setNodes(newNodes);
    setEdges(newEdges);
  }, [storage, moduleCache, setNodes, setEdges]);

  // 5. 事件处理 (直接作用于 storage)
  const handleToggle = useCallback((ref: TreeNodeRef) => {
    setStorage(prev => {
      const next = prev.shareClone();
      const exists = next.get(ref);
      if (exists) next.collapse(ref);
      else next.expand(ref, moduleCache);
      return next;
    });
  }, [moduleCache]);

  const handleFocus = useCallback((ref: TreeNodeRef, kind: TreeNodeKind, label: string) => {
    onNavigate({ type: 'FOCUS', nodeRef: ref, kind, label });
  }, [onNavigate]);

  const handleRequestMenu = useCallback((e: React.MouseEvent, ref: TreeNodeRef, kind: TreeNodeKind) => {
    setMenuState({
      x: e.clientX,
      y: e.clientY,
      nodeRef: ref,
      kind
    });
  }, []);

  const guideNodeTypes = useMemo(() => ({
    guideNode: (props: any) => (
      <GuideNodeComp
        {...props}
        onToggle={handleToggle}
        onFocus={handleFocus}
        onRequestMenu={handleRequestMenu}
      />
    )
  }), [handleToggle, handleFocus, handleRequestMenu]);

  const handleMenuAction = useCallback((action: string) => {
    if (!menuState) return;

    if (action === 'expand-all') {
      setStorage(prev => {
        const next = prev.shareClone();
        next.dfsExpand(menuState.nodeRef, moduleCache);
        return next;
      });
    } else if (action === 'focus') {
      handleFocus(menuState.nodeRef, menuState.kind, "Unknown Label");
    }

    setMenuState(null);
  }, [menuState, handleFocus, moduleCache, setStorage]);

  return (
    <div style={{ width: "100%", height: "100%", background: "#fff" }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={guideNodeTypes}
        fitView={false} // 关键：避免每次更新重置视图
        panOnDrag={true}
        zoomOnScroll={true}
      >
        <Background gap={20} size={1} />
        <Controls />
      </ReactFlow>

      {menuState && (
        <SimpleMenu
          x={menuState.x}
          y={menuState.y}
          onClose={() => setMenuState(null)}
          onAction={handleMenuAction}
          kind={menuState.kind}
        />
      )}
    </div>
  );
};
