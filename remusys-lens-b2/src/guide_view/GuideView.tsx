import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  collapseNode,
  createGuideTreeController,
  dfsExpandNode,
  disposeGuideTreeController,
  expandNode,
  expandChildrenNode,
  refreshSameModule,
  requestFocusNode,
  type GuideTreeController,
  collapseChildrenNode,
} from "./guide-view-tree";
import { collectGuideTree, guideNodeTypes, type GuideRFNode } from "./Node";
import { NodeMenu, type NodeMenuItem, type NodeMenuProps } from "./NodeMenu";
import { useIRStore } from "../ir/state";
import type { GuideNodeData, GuideNodeExpand } from "remusys-wasm-b2";
import { useGraphState, type GraphStore, } from "../flow/state";
import { Background, Controls, ReactFlow, ReactFlowProvider, type Edge } from "@xyflow/react";

type GuideTreeActions = {
  requestFocus: (node: GuideNodeData) => void;
  expandChildren: (node: GuideNodeData) => void;
  dfsExpand: (node: GuideNodeData) => void;
  collapse: (node: GuideNodeData) => void;
  collapseChildren: (node: GuideNodeData) => void;
};

function buildMenuItems(
  node: GuideNodeData,
  treeActions: GuideTreeActions,
  graphStore: GraphStore,
): NodeMenuItem[] {
  const baseItems: NodeMenuItem[] = [
    {
      label: "聚焦此处",
      onSelect: node => treeActions.requestFocus(node),
    },
    {
      label: "展开一层子结点",
      onSelect: node => treeActions.expandChildren(node),
    },
    {
      label: "展开全部子结点",
      onSelect: node => treeActions.dfsExpand(node),
    },
    {
      label: "收起结点",
      onSelect: node => treeActions.collapse(node),
    },
    {
      label: "收起全部子结点",
      onSelect: node => treeActions.collapseChildren(node),
    }
  ];
  let items: NodeMenuItem[];
  switch (node.kind) {
    case "Module":
    case "Func": case "GlobalVar":
    case "Block":
    case "NormalInst": case "TerminatorInst": case "PhiInst": {
      items = baseItems;
      break;
    }
    default:
      items = [];
      break;
  }

  switch (node.kind) {
    case "Module":
      items.push({
        label: "显示函数调用图",
        onSelect(_) { graphStore.setGraphType({ type: "CallGraph" }) },
      });
      break;
    case "Func": {
      const irObj = node.irObject;
      if (irObj.type === "FuncHeader" || irObj.type === "FuncArg") {
        // 这两种节点没有对应的 IR 实体, 无法提供特定于实体的菜单项.
        break;
      }
      if (irObj.type !== "Global")
        throw new Error("Func node with non-Global IR object");
      items.push(...[
        {
          label: "显示 CFG",
          onSelect(_: GuideNodeData) { graphStore.setGraphType({ type: "FuncCfg", func: irObj.value }) }
        },
        {
          label: "显示支配树",
          onSelect(_: GuideNodeData) { graphStore.setGraphType({ type: "FuncDom", func: irObj.value }) }
        }
      ]);
      break;
    }
    case "Block": {
      const irObj = node.irObject;
      if (irObj.type === "BlockIdent") {
        // 这类节点没有对应的 Block 实体, 无法提供特定于实体的菜单项.
        return [];
      }
      if (irObj.type !== "Block")
        throw new Error("Block node with non-Block IR object");
      items.push({
        label: "显示 DFG",
        onSelect(_: GuideNodeData) { graphStore.setGraphType({ type: "BlockDfg", block: irObj.value }) }
      });
      break;
    }
    case "NormalInst":
    case "TerminatorInst":
    case "PhiInst": {
      const irObj = node.irObject;
      if (irObj.type !== "Inst")
        throw new Error("Inst node with non-Inst IR object");
      items.push({
        label: "显示 Def-Use 图",
        onSelect(_: GuideNodeData) { graphStore.setGraphType({ type: "DefUse", center: irObj.value }) }
      });
    }
  }
  return items;
}

export default function GuideView() {
  const irState = useIRStore();
  const graphStore = useGraphState();
  const controllerRef = useRef<GuideTreeController>(createGuideTreeController());
  const pendingRootRef = useRef<GuideNodeExpand | null>(null);
  const [revision, bumpRevision] = useReducer((x: number) => x + 1, 0);
  const [menu, setMenu] = useState<NodeMenuProps | null>(null);

  useEffect(() => {
    return () => {
      disposeGuideTreeController(controllerRef.current);
    };
  }, []);

  const handleRequestFocus = useCallback((node: GuideNodeData) => {
    const result = requestFocusNode(controllerRef.current, irState, node);
    pendingRootRef.current = result.root;
    bumpRevision();
  }, [irState]);

  const handleExpandChildren = useCallback((node: GuideNodeData) => {
    const result = expandChildrenNode(controllerRef.current, irState, node);
    pendingRootRef.current = result.root;
    bumpRevision();
  }, [irState]);

  const handleExpandOne = useCallback((node: GuideNodeData) => {
    const result = expandNode(controllerRef.current, irState, node);
    pendingRootRef.current = result.root;
    bumpRevision();
  }, [irState]);

  const handleDfsExpand = useCallback((node: GuideNodeData) => {
    const result = dfsExpandNode(controllerRef.current, irState, node);
    pendingRootRef.current = result.root;
    bumpRevision();
  }, [irState]);

  const handleCollapse = useCallback((node: GuideNodeData) => {
    const result = collapseNode(controllerRef.current, irState, node);
    pendingRootRef.current = result.root;
    bumpRevision();
  }, [irState]);

  const handleCollapseChildren = useCallback((node: GuideNodeData) => {
    const result = collapseChildrenNode(controllerRef.current, irState, node);
    pendingRootRef.current = result.root;
    bumpRevision();
  }, [irState]);

  const treeActions: GuideTreeActions = {
    requestFocus: handleRequestFocus,
    expandChildren: handleExpandChildren,
    dfsExpand: handleDfsExpand,
    collapse: handleCollapse,
    collapseChildren: handleCollapseChildren,
  };

  const [nodes, edges] = useMemo<[GuideRFNode[], Edge[]]>(() => {
    const pendingRoot = pendingRootRef.current;
    pendingRootRef.current = null;
    const root = pendingRoot ?? refreshSameModule(controllerRef.current, irState).root;
    const [newNodes, newEdges] = collectGuideTree(root, {
      onFocus: handleRequestFocus,
      onToggle: (node) => {
        if (node.children) {
          handleCollapse(node);
        } else {
          handleExpandOne(node);
        }
      },
      onRowContextMenu(event, rowNode) {
        event.stopPropagation();
        const menuItems = buildMenuItems(rowNode, treeActions, graphStore);
        setMenu({
          items: menuItems,
          x: event.clientX,
          y: event.clientY,
          node: rowNode,
          onClose: () => setMenu(null)
        });
      },
    });
    if (newNodes.length === 0) {
      return [[{
        type: "GuideNode",
        id: "empty",
        data: {
          id: "empty",
          irObject: { type: "Module" },
          label: "错误: 无法构建引导树",
          kind: "Module",
          focusClass: "NotFocused",
          children: [],
          onFocus: () => { },
          onToggle: () => { },
          onRowContextMenu(event, _) { event.stopPropagation(); },
        },
        position: { x: 0, y: 0 },
        width: 240,
        height: 52,
      }], []];
    }
    return [newNodes, newEdges];
  }, [irState, revision, handleRequestFocus, handleExpandOne, handleCollapse]);

  const buildMenuItemsCb = useCallback(
    (node: GuideNodeData) => buildMenuItems(node, treeActions, graphStore),
    [treeActions, graphStore]
  );

  return (
    <div style={{ width: "100%", height: "100%", background: "#fff" }}>
      <ReactFlowProvider>
        <ReactFlow
          nodeTypes={guideNodeTypes}
          nodes={nodes}
          edges={edges}
          onNodeDoubleClick={(event, node) => {
            event.preventDefault();
            node.data.onFocus(node.data);
          }}
          onNodeContextMenu={(event, node) => {
            event.preventDefault();
            const menuItems = buildMenuItemsCb(node.data);
            setMenu({
              items: menuItems,
              x: event.clientX,
              y: event.clientY,
              node: node.data,
              onClose: () => setMenu(null)
            });
          }}
          onClick={() => setMenu(null)}
          fitView
        >
          <Background id="io.medihbt.remusysLens.GuideView" />
          <Controls />
        </ReactFlow>
        {menu && (<NodeMenu {...menu} />)}

      </ReactFlowProvider>
    </div>
  );
}