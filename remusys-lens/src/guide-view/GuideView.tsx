import React, { useState, useEffect, useCallback, useMemo } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  ReactFlowProvider,
} from "@xyflow/react";
import * as dagre from "dagre";
import { GuideNodeComp } from "./components/GuideNodeComp";
import { TreeNodeStorage, type TreeNodeKind } from "./guide-view-tree";
import type { ModuleCache } from "../ir/ir-state";
import type {
  GuideRFNode,
  GuideRFEdge,
  NavEvent,
  GuideRFNodeProp,
} from "./types";
import { SimpleMenu } from "./components/SimpleMenu";
import { getNodeIdLabel } from "./guide-view-tree";
import { sourceTrackableToString, type SourceTrackable } from "../ir/ir";

interface GuideViewProps {
  moduleCache: ModuleCache;
  onNavigate: (event: NavEvent) => void; // 向外暴露导航事件（给代码编辑器、属性面板用）
  incomingNavEvent?: NavEvent | null; // 可选：接收来自外部的导航事件并执行
  onConsumeNavEvent?: (event: NavEvent) => void; // 在成功处理 incomingNavEvent 后调用以清理，将事件回传给外部
}

function renderTree(
  storage: TreeNodeStorage,
  module: ModuleCache,
): [GuideRFNode[], GuideRFEdge[]] {
  const { nodes, edges } = storage.export(module);

  // 转换 Nodes (只转换展开的节点)
  const rfNodes: GuideRFNode[] = nodes
    .filter((n) => n.expanded)
    .map((n) => {
      let height = Math.max(52 + n.children.length * 41, 52 + 41);
      height = Math.min(height, 300); // 设置最大高度，避免过高的节点
      return {
        id: sourceTrackableToString(n.treeNode.selfId),
        type: "guideNode",
        position: { x: 0, y: 0 },
        data: n,
        width: 220,
        height, // 简单高度估算
      };
    });
  // 转换 Edges
  const rfEdges: GuideRFEdge[] = edges.map((e) => ({
    id: e.id,
    source: sourceTrackableToString(e.source.treeNode.selfId),
    target: sourceTrackableToString(e.target.treeNode.selfId),
    type: "default",
    markerEnd: { type: "arrowclosed" },
  }));

  // Dagre 布局 (LR 方向)
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  dagreGraph.setGraph({ rankdir: "LR", nodesep: 50, ranksep: 80 });

  rfNodes.forEach((node) =>
    dagreGraph.setNode(node.id, {
      width: node.width || 200,
      height: node.height || 100,
    }),
  );
  rfEdges.forEach((edge) => dagreGraph.setEdge(edge.source, edge.target));
  dagre.layout(dagreGraph);

  const laidNodes = rfNodes.map((node) => {
    const { x, y } = dagreGraph.node(node.id);
    return {
      ...node,
      position: { x: x - node.width! / 2, y: y - node.height! / 2 },
    };
  });
  return [laidNodes, rfEdges];
}

export const GuideView: React.FC<GuideViewProps> = ({
  moduleCache,
  onNavigate,
  incomingNavEvent,
  onConsumeNavEvent,
}) => {
  // 配置：调试菜单模式
  // - true: menuDebugMode 启用时，点击其他地方不关闭菜单（便于调试）
  // - false: 点击其他地方会关闭菜单（正常行为）
  const menuDebugMode = false;
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
    nodeRef: SourceTrackable;
    kind: TreeNodeKind;
  } | null>(null);

  // 当菜单打开时，根据 `menuDebugMode` 决定是否在点击页面其他地方关闭菜单
  useEffect(() => {
    if (!menuState) return;
    const onDocClick = (_: MouseEvent) => {
      if (menuDebugMode) {
        // debug 模式下不自动关闭菜单
        return;
      }
      setMenuState(null);
    };
    document.addEventListener("click", onDocClick);
    return () => document.removeEventListener("click", onDocClick);
  }, [menuState, menuDebugMode]);

  // 4. 核心逻辑：当 storage 变化时更新布局
  useEffect(() => {
    const [newNodes, newEdges] = renderTree(storage, moduleCache);
    setNodes(newNodes);
    setEdges(newEdges);
  }, [storage, moduleCache, setNodes, setEdges]);

  useEffect(() => {
    console.debug(
      "GuideView: storage changed, nodesById size =",
      storage.nodesSize,
    );
  }, [storage]);

  // 5. 事件处理 (直接作用于 storage)
  const handleToggle = useCallback(
    (ref: SourceTrackable) => {
      setStorage((prev) => {
        const next = prev.shareClone();
        const exists = next.get(ref);
        if (exists) next.collapse(ref);
        else next.expand(ref, moduleCache);
        return next;
      });
    },
    [moduleCache],
  );

  const handleFocus = useCallback(
    (ref: SourceTrackable, kind: TreeNodeKind, label: string) => {
      onNavigate({ type: "Focus", nodeRef: ref, kind, label });
    },
    [onNavigate],
  );

  const handleRequestMenu = useCallback(
    (e: React.MouseEvent, ref: SourceTrackable, kind: TreeNodeKind) => {
      setMenuState({
        x: e.clientX,
        y: e.clientY,
        nodeRef: ref,
        kind,
      });
    },
    [],
  );

  // 处理来自外部（App）的 NavEvent，比如 ExpandOne/ExpandAll/Collapse
  useEffect(() => {
    const ev = incomingNavEvent;
    if (!ev) return;

    console.debug("GuideView: received incomingNavEvent", ev);

    switch (ev.type) {
      case "ExpandOne":
        setStorage((prev) => {
          const next = prev.shareClone();
          try {
            next.expandChildren(ev.nodeRef, moduleCache);
          } catch (e) {
            console.warn("GuideView: expand one failed", ev.nodeRef, e);
          }
          return next;
        });
        break;
      case "ExpandAll":
        setStorage((prev) => {
          const next = prev.shareClone();
          try {
            next.dfsExpand(ev.nodeRef, moduleCache);
          } catch (e) {
            console.warn("GuideView: expand all failed", ev.nodeRef, e);
          }
          return next;
        });
        break;
      case "Collapse":
        setStorage((prev) => {
          const next = prev.shareClone();
          try {
            next.collapseChildren(ev.nodeRef);
          } catch (e) {
            console.warn("GuideView: collapse failed", ev.nodeRef, e);
          }
          return next;
        });
        break;
      default:
        break;
    }

    if (onConsumeNavEvent) onConsumeNavEvent(ev);
  }, [incomingNavEvent, moduleCache, onConsumeNavEvent]);

  // 根据节点类型生成 menu items（每项绑定一个 NavEvent）
  const buildMenuItems = useCallback(
    (
      ref: SourceTrackable,
      kind: TreeNodeKind,
    ): { label: string; event: NavEvent }[] => {
      const label = getNodeIdLabel(moduleCache, ref);
      const baseItems: { label: string; event: NavEvent }[] = [
        {
          label: "展开一层子节点",
          event: { type: "ExpandOne", nodeRef: ref, kind },
        },
        {
          label: "展开全部子节点",
          event: { type: "ExpandAll", nodeRef: ref, kind },
        },
        { label: "折叠节点", event: { type: "Collapse", nodeRef: ref, kind } },
        {
          label: "聚焦此处",
          event: { type: "Focus", nodeRef: ref, kind, label },
        },
      ];

      // 为特定类型增加额外操作
      if (kind === "Func") {
        if (ref.type === "Global") {
          baseItems.push({
            label: "显示 CFG",
            event: { type: "ShowCfg", funcDef: ref.value },
          });
          baseItems.push({
            label: "显示支配树",
            event: { type: "ShowDominance", funcDef: ref.value },
          });
        }
      } else if (kind === "Block") {
        if (ref.type === "Block") {
          baseItems.push({
            label: "显示局部 def-use 关系",
            event: { type: "ShowDfg", blockID: ref.value },
          });
        }
      } else if (kind === "Inst" || kind === "Phi" || kind === "Terminator") {
        if (ref.type === "Inst") {
          baseItems.push({
            label: "显示指令 def-use 关系",
            event: {
              type: "ShowValueDefUse",
              valueID: { type: "Inst", value: ref.value },
            },
          });
        }
      }

      return baseItems;
    },
    [moduleCache],
  );

  const guideNodeTypes = useMemo(
    () => ({
      guideNode: (props: GuideRFNodeProp) => (
        <GuideNodeComp
          {...props}
          onToggle={handleToggle}
          onFocus={handleFocus}
          onRequestMenu={handleRequestMenu}
        />
      ),
    }),
    [handleToggle, handleFocus, handleRequestMenu],
  );

  const handleMenuAction = useCallback(
    (event: NavEvent) => {
      if (!menuState) return;
      console.debug("GuideView: handling menu action", event);

      // 对于本地能直接完成的动作（例如 ExpandAll），同时继续发送事件以通知外部
      if (event.type === "ExpandAll") {
        setStorage((prev) => {
          const next = prev.shareClone();
          try {
            const expanded = next.dfsExpand(event.nodeRef, moduleCache);
            console.debug(
              "GuideView: dfsExpand returned",
              expanded.length,
              "nodes",
            );
          } catch (e) {
            console.warn("dfsExpand failed", e);
          }
          return next;
        });
      } else if (event.type === "ExpandOne") {
        setStorage((prev) => {
          const next = prev.shareClone();
          try {
            next.expandChildren(event.nodeRef, moduleCache);
          } catch (e) {
            console.warn("expandChildren failed", e);
          }
          return next;
        });
      }

      // 将事件发给外部消费者
      onNavigate(event);
      setMenuState(null);
    },
    [menuState, onNavigate, moduleCache, setStorage],
  );

  return (
    <div style={{ width: "100%", height: "100%", background: "#fff" }}>
      <ReactFlowProvider>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={guideNodeTypes}
          fitView
        >
          <Background id="GuideView" />
          <Controls />
        </ReactFlow>
      </ReactFlowProvider>

      {menuState && (
        <SimpleMenu
          x={menuState.x}
          y={menuState.y}
          onClose={() => setMenuState(null)}
          onAction={handleMenuAction}
          kind={menuState.kind}
          items={buildMenuItems(menuState.nodeRef, menuState.kind)}
        />
      )}
    </div>
  );
};
