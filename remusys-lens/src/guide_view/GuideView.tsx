import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  collapseChildrenNode,
  collapseNode,
  createGuideTreeController,
  dfsExpandNode,
  disposeGuideTreeController,
  expandChildrenNode,
  expandNode,
  refreshSameModule,
  requestFocusNode,
  type GuideTreeController,
} from "./guide-view-tree";
import {
  collectGuideTree,
  GuideHandlersContext,
  type GuideNodeHandlers,
  type GuideRFNode,
} from "./GuideContext";
import { NodeMenu, type NodeMenuItem, type NodeMenuProps } from "./NodeMenu";
import { useIRStore } from "../ir/state";
import type { GuideNodeData, GuideNodeExpand } from "remusys-wasm";
import { useGraphState, type GraphStore } from "../flow/state";
import {
  Background,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  type Edge,
} from "@xyflow/react";
import GuideViewNode from "./Node";

// ---------------------------------------------------------------------------
// 右键菜单
// ---------------------------------------------------------------------------

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
      onSelect: (node) => treeActions.requestFocus(node),
    },
    {
      label: "展开一层子结点",
      onSelect: (node) => treeActions.expandChildren(node),
    },
    {
      label: "展开全部子结点",
      onSelect: (node) => treeActions.dfsExpand(node),
    },
    {
      label: "收起结点",
      onSelect: (node) => treeActions.collapse(node),
    },
    {
      label: "收起全部子结点",
      onSelect: (node) => treeActions.collapseChildren(node),
    },
  ];

  let items: NodeMenuItem[];
  switch (node.kind) {
    case "Module":
      items = baseItems.filter((item) => item.label !== "收起结点");
      break;
    case "Func":
    case "GlobalVar":
    case "Block":
    case "NormalInst":
    case "TerminatorInst":
    case "PhiInst":
      items = baseItems;
      break;
    default:
      items = [
        {
          label: "聚焦此处",
          onSelect: (node) => treeActions.requestFocus(node),
        },
      ];
      break;
  }

  // 图类型切换
  switch (node.kind) {
    case "Module":
      items.push({
        label: "显示函数调用图",
        onSelect(_) {
          graphStore.setGraphType({ type: "CallGraph" });
        },
      });
      break;
    case "Func": {
      const irObj = node.irObject;
      if (irObj.type === "FuncHeader" || irObj.type === "FuncArg") break;
      if (irObj.type !== "Global")
        throw new Error("Func node with non-Global IR object");
      items.push(
        {
          label: "显示 CFG",
          onSelect(_: GuideNodeData) {
            graphStore.setGraphType({ type: "FuncCfg", func: irObj.value });
          },
        },
        {
          label: "显示支配树",
          onSelect(_: GuideNodeData) {
            graphStore.setGraphType({ type: "FuncDom", func: irObj.value });
          },
        },
      );
      break;
    }
    case "Block": {
      const irObj = node.irObject;
      if (irObj.type === "BlockIdent") {
        return [
          {
            label: "聚焦此处",
            onSelect: (node) => treeActions.requestFocus(node),
          },
        ];
      }
      if (irObj.type !== "Block")
        throw new Error("Block node with non-Block IR object");
      items.push({
        label: "显示 DFG",
        onSelect(_: GuideNodeData) {
          graphStore.setGraphType({ type: "BlockDfg", block: irObj.value });
        },
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
        onSelect(_: GuideNodeData) {
          graphStore.setGraphType({ type: "DefUse", center: irObj.value });
        },
      });
      break;
    }
  }
  return items;
}

// ---------------------------------------------------------------------------
// 空树占位
// ---------------------------------------------------------------------------

function emptyPlaceholder(): [GuideRFNode[], Edge[]] {
  return [
    [
      {
        type: "GuideNode",
        id: "empty",
        data: {
          id: "empty",
          irObject: { type: "Module" },
          label: "错误: 无法构建引导树",
          kind: "Module",
          focusClass: "NotFocused",
          children: [],
        } as GuideNodeExpand,
        position: { x: 0, y: 0 },
        width: 240,
        height: 52,
      },
    ],
    [],
  ];
}

const guideNodeTypes = {
  GuideNode: GuideViewNode,
};

// ---------------------------------------------------------------------------
// GuideView 组件
// ---------------------------------------------------------------------------

export default function GuideView() {
  const irState = useIRStore();
  const graphStore = useGraphState();
  const controllerRef = useRef<GuideTreeController>(
    createGuideTreeController(),
  );
  const [menu, setMenu] = useState<NodeMenuProps | null>(null);

  // ── 树根状态 ─────────────────────────────────────────────────────
  const [treeRoot, setTreeRoot] = useState<GuideNodeExpand | null>(null);
  const skipRefreshRef = useRef(false);

  // ── 生命周期 ─────────────────────────────────────────────────────
  useEffect(() => {
    const control = controllerRef.current;
    return () => {
      disposeGuideTreeController(control);
    };
  }, []);

  // 外部驱动的树刷新：焦点变更、模块替换。
  useEffect(() => {
    if (skipRefreshRef.current) {
      skipRefreshRef.current = false;
      return;
    }
    if (!irState.module) return;
    const root = refreshSameModule(controllerRef.current, irState).root;
    setTreeRoot(root);
  }, [irState, irState.focus, irState.module]);

  // ── handler：展开 / 收起 / 聚焦 ──────────────────────────────────

  const handleRequestFocus = useCallback(
    (node: GuideNodeData) => {
      const result = requestFocusNode(controllerRef.current, irState, node);
      skipRefreshRef.current = true;
      setTreeRoot(result.root);
    },
    [irState],
  );

  const handleExpandChildren = useCallback(
    (node: GuideNodeData) => {
      const result = expandChildrenNode(controllerRef.current, irState, node);
      skipRefreshRef.current = true;
      setTreeRoot(result.root);
    },
    [irState],
  );

  const handleExpandOne = useCallback(
    (node: GuideNodeData) => {
      const result = expandNode(controllerRef.current, irState, node);
      skipRefreshRef.current = true;
      setTreeRoot(result.root);
    },
    [irState],
  );

  const handleDfsExpand = useCallback(
    (node: GuideNodeData) => {
      const result = dfsExpandNode(controllerRef.current, irState, node);
      skipRefreshRef.current = true;
      setTreeRoot(result.root);
    },
    [irState],
  );

  const handleCollapse = useCallback(
    (node: GuideNodeData) => {
      const result = collapseNode(controllerRef.current, irState, node);
      skipRefreshRef.current = true;
      setTreeRoot(result.root);
    },
    [irState],
  );

  const handleCollapseChildren = useCallback(
    (node: GuideNodeData) => {
      const result = collapseChildrenNode(controllerRef.current, irState, node);
      skipRefreshRef.current = true;
      setTreeRoot(result.root);
    },
    [irState],
  );

  const treeActions = useMemo<GuideTreeActions>(
    () => ({
      requestFocus: handleRequestFocus,
      expandChildren: handleExpandChildren,
      dfsExpand: handleDfsExpand,
      collapse: handleCollapse,
      collapseChildren: handleCollapseChildren,
    }),
    [
      handleRequestFocus,
      handleExpandChildren,
      handleDfsExpand,
      handleCollapse,
      handleCollapseChildren,
    ],
  );

  // ── 回调包（通过 Context 注入节点，而非塞进 data） ──────────────
  const handlers = useMemo<GuideNodeHandlers>(
    () => ({
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
          onClose: () => setMenu(null),
        });
      },
    }),
    [
      handleRequestFocus,
      handleExpandOne,
      handleCollapse,
      treeActions,
      graphStore,
    ],
  );

  // ── 纯计算：GuideNodeExpand → React Flow nodes / edges ───────────
  const [nodes, edges] = useMemo<[GuideRFNode[], Edge[]]>(() => {
    if (!treeRoot) return emptyPlaceholder();

    const [newNodes, newEdges] = collectGuideTree(treeRoot);

    if (newNodes.length === 0) return emptyPlaceholder();
    return [newNodes, newEdges];
  }, [treeRoot]);

  const buildMenuItemsCb = useCallback(
    (node: GuideNodeData) => buildMenuItems(node, treeActions, graphStore),
    [treeActions, graphStore],
  );

  // ── 渲染 ─────────────────────────────────────────────────────────
  return (
    <div style={{ width: "100%", height: "100%", background: "#fff" }}>
      <ReactFlowProvider>
        <GuideHandlersContext.Provider value={handlers}>
          <ReactFlow
            nodeTypes={guideNodeTypes}
            nodes={nodes}
            edges={edges}
            onNodeDoubleClick={(event, node) => {
              event.preventDefault();
              handlers.onFocus(node.data);
            }}
            onNodeContextMenu={(event, node) => {
              event.preventDefault();
              const menuItems = buildMenuItemsCb(node.data);
              setMenu({
                items: menuItems,
                x: event.clientX,
                y: event.clientY,
                node: node.data,
                onClose: () => setMenu(null),
              });
            }}
            onClick={() => setMenu(null)}
            fitView
          >
            <Background id="io.medihbt.remusysLens.GuideView" />
            <Controls />
          </ReactFlow>
          {menu && <NodeMenu {...menu} />}
        </GuideHandlersContext.Provider>
      </ReactFlowProvider>
    </div>
  );
}
