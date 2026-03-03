import { type Node, type Edge } from "@xyflow/react";
import {
  ReactFlow,
  Controls,
  Background,
  type ReactFlowInstance,
  type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import * as dagre from "dagre";
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type {
  BlockDt,
  FuncObjDt,
  GlobalObjDt,
  InstDt,
  SourceLoc,
} from "../ir/ir";
import { GuideNodeComp } from "./components/GuideNodeComp";

/* DO NOT DELETE: load this if fail */
const GuideViewText = (
  <>
    <h3>导航视图</h3>
    <p>
      从 Module 全局对象到当前锁定对象的 React Flow 树状视图, 展现“模块 - 函数 -
      基本块 - 指令”的 IR 模块层次架构，在这个架构上提供导航和聚焦功能。
    </p>
    <h3>结点</h3>
    <p>
      圆角矩形小窗口, 圆角只有 3px. 通过 React Flow 的 handle 与其他结点连接.
      组成包括:
    </p>
    <ul>
      <li>
        顶栏: 展示类型图标、结点名称, 有个 <code>⋯</code> 形状的按钮,
        点击按钮即可展开对这个结点的操作菜单(右键单击顶栏也可以)
      </li>
      <li>
        子结点列表视图，每行一个子结点概述（类型图标、名称）。最右侧有一个表示展开或者关闭的
        radio button. 点击列表整行的任意位置都展开这个位置上的子结点,
        这一行变成浅灰色示意展开; 再次点击则收回, 子结点变回白色.
        收回后销毁子结点状态, 也就是如果收回前这个子结点被展开成一棵子树,
        下次展开时会全部重新请求数据, 子结点不会保持展开状态.
      </li>
    </ul>
    <h3>图标</h3>
    <p>
      圆形 SVG 图标, 直径 16px, 文字 9pt. 字体为 Cascadia Mono,
      各类型文字分别是:
    </p>
    <ul>
      <li>模块: M, 红底白字.</li>
      <li>
        全局变量: Gv, 定义为靛蓝底白字, 声明为灰 (<code>#666</code>) 底白字.
      </li>
      <li>
        函数: Fx, 定义为黄底黑字, 声明为灰(<code>#666</code>)底白字.
      </li>
      <li>基本块: B, 橙底白字.</li>
      <li>Phi 指令: Φ, 浅蓝底黑字.</li>
      <li>终止指令: Ti, 橙底白字.</li>
      <li>普通指令: I, 浅绿底黑字.</li>
    </ul>
    <h3>边</h3>
    <p>使用 React Flow 默认的边样式，那个就已经很好用了.</p>
    <h3>树结构</h3>
    <p>
      从左到右、动态展开的横向树结构,
      每次在当前选中的结点上点击某个子结点的行引发展开操作时,
      就会重新计算整棵树的结点布局, 并把焦点设在当前选中的结点上.
    </p>
    <h3>总体交互</h3>
    <p>
      系统刚刚加载完一段 IR, 进入阅读界面, 出现这个导航视图.
      此时导航视图只有一个 Module, 聚焦在 Module 里.
    </p>
    <p>Module 的动作选项包括:</p>
    <ul>
      <li>
        (所有结点都这样)聚焦: 焦点放到这个结点上, 触发 Monaco
        区域显示该结点关联的源码并聚焦到该结点的源码位置
      </li>
      <li>(所有结点都这样)展开本级: 展开该结点下的所有子结点 1 层</li>
      <li>(所有结点都这样)展开所有: 递归展开该结点下的所有子结点</li>
      <li>
        (所有结点都这样)重命名: 检查名称是否冲突, 重新设置结点的名称,
        更新所有关联的图和源码映射
      </li>
      <li>显示引用图: 展示全局变量引用图(函数调用图)</li>
    </ul>
    <p>函数的动作选项包括:</p>
    <ul>
      <li>(三选一:1, 默认)显示 CFG</li>
      <li>(三选一:2)显示支配树</li>
      <li>(三选一:3)显示调用者图</li>
      <li>
        应用 Pass: 选择一个 Pass, 给这个函数应用这个 Pass, 在时间线上展示这个
        Pass 的执行过程和结果
      </li>
    </ul>
    <p>基本块的动作选项包括:</p>
    <ul>
      <li>显示 DFG</li>
      <li>
        分析顺序依赖分割点: 计算这个基本块的顺序依赖分割点, 在 CFG 里高亮显示
      </li>
      <li>
        前驱后继: 展示这个基本块的前驱和后继基本块列表,
        点击列表项可以聚焦到对应基本块
      </li>
    </ul>
    <p>指令的动作选项包括:</p>
    <ul>
      <li>
        显示数据流依赖: 展示这个指令的数据流依赖图,
        包括它依赖的其他指令和依赖它的其他指令
      </li>
      <li>
        相关指令列表: 展示与这个指令相关的其他指令列表,
        包括同一基本块里的其他指令、使用了同一变量的其他指令等
      </li>
      <li>(对于调用指令)内联: 创建快照并内联该调用点</li>
    </ul>
    <p>
      在 demo 阶段, 所有与具体功能相关联的选项都不要实现, 弹一个 alert 出来即可.
    </p>
  </>
);

export type IRArchObj = GlobalObjDt | FuncObjDt | BlockDt | InstDt | null;
export type IRArchObjTy =
  | "Module"
  | "Global"
  | "ExternGlobal"
  | "Func"
  | "ExternFunc"
  | "Block"
  | "Inst"
  | "Phi"
  | "Terminator";

export function getObjText(ir_obj: IRArchObj): IRArchObjTy {
  if (ir_obj === null) {
    return "Module";
  }

  switch (ir_obj.typeid) {
    case "GlobalVar":
      // 根据 init 字段判断是定义还是声明
      return ir_obj.init === "None" ? "ExternGlobal" : "Global";
    case "Func":
      // 根据 blocks 字段判断是定义还是声明
      return ir_obj.blocks ? "Func" : "ExternFunc";
    case "Block":
      return "Block";
    case "Inst":
      return "Inst";
    case "Phi":
      return "Phi";
    case "Terminator":
      return "Terminator";
    default:
      // 类型保护，确保所有情况都被处理
      return "Module";
  }
}

export type GuideNodeData = {
  label: string;
  ir_obj: IRArchObj;
  children: GuideNodeChildStat[];
  path: string;
  onToggleChild?: (childPath: string) => void;
  onMenuOpen?: (payload: GuideMenuPayload) => void;
};

export type GuideNode = Node<GuideNodeData, "guideNode">;
export type GuideNodeChildStat =
  | { expanded: false; typeid: IRArchObjTy; label: string; path: string }
  | { expanded: true; data: GuideNodeData; path: string };

export type GuideMenuItem = { key: string; label: string };
export type GuideMenuPayload = {
  items: GuideMenuItem[];
  displayName: string;
  nodeType: IRArchObjTy;
  idText: string;
  path: string;
  clientX: number;
  clientY: number;
};
type GuideMenuState = GuideMenuPayload & {
  left: number;
  top: number;
};

const nodeTypes = { guideNode: GuideNodeComp };

type MockTreeNode = {
  label: string;
  ir_obj: IRArchObj;
  typeid: IRArchObjTy;
  children: MockTreeNode[];
};

function getMockChildPath(parentPath: string, index: number): string {
  return `${parentPath}/${index}`;
}

function getParentPath(path: string): string {
  const parts = path.split("/");
  if (parts.length <= 1) return path;
  return parts.slice(0, -1).join("/") || path;
}

function getMockNodeByPath(
  root: MockTreeNode,
  path: string,
): MockTreeNode | null {
  const parts = path.split("/").slice(1);
  let current: MockTreeNode = root;
  for (const part of parts) {
    const index = Number(part);
    if (!Number.isInteger(index) || !current.children[index]) {
      return null;
    }
    current = current.children[index];
  }
  return current;
}

function collectDescendantPaths(
  root: MockTreeNode,
  path: string,
): string[] {
  const node = getMockNodeByPath(root, path);
  if (!node) return [];
  const paths: string[] = [];
  const dfs = (current: MockTreeNode, currentPath: string) => {
    current.children.forEach((child, index) => {
      const childPath = getMockChildPath(currentPath, index);
      paths.push(childPath);
      dfs(child, childPath);
    });
  };
  dfs(node, path);
  return paths;
}

function buildGuideTree(
  root: MockTreeNode,
  path: string,
  expandedMap: Record<string, boolean>,
  onToggleChild: (childPath: string) => void,
  onMenuOpen: (payload: GuideMenuPayload) => void,
): GuideNodeData {
  const children: GuideNodeChildStat[] = root.children.map((child, index) => {
    const childPath = getMockChildPath(path, index);
    const isExpanded = !!expandedMap[childPath];
    if (!isExpanded) {
      return {
        expanded: false,
        typeid: child.typeid,
        label: child.label,
        path: childPath,
      };
    }
    return {
      expanded: true,
      data: buildGuideTree(
        child,
        childPath,
        expandedMap,
        onToggleChild,
        onMenuOpen,
      ),
      path: childPath,
    };
  });

  return {
    label: root.label,
    ir_obj: root.ir_obj,
    children,
    path,
    onToggleChild,
    onMenuOpen,
  };
}

function layoutNodes(
  nodes: GuideNode[],
  edges: Edge[],
): { nodes: GuideNode[]; edges: Edge[] } {
  if (nodes.length === 0) return { nodes, edges };

  // 创建 dagre 图实例
  const g = new dagre.graphlib.Graph({
    compound: false,
  });

  // 设置图的布局配置 - 从左到右的树结构
  g.setGraph({
    rankdir: "LR", // 方向：从左到右
    align: "UL", // 对齐方式：上左对齐
    nodesep: 60, // 同一层级节点之间的水平间距
    ranksep: 100, // 不同层级之间的垂直间距
    marginx: 40, // 水平边距
    marginy: 40, // 垂直边距
    ranker: "network-simplex", // 排名算法：网络单纯形法（适合树结构）
  });

  // 设置默认节点尺寸
  g.setDefaultNodeLabel(() => ({
    width: 200, // 节点宽度
    height: 120, // 节点高度
  }));

  // 设置默认边配置
  g.setDefaultEdgeLabel(() => ({
    minlen: 1, // 边的最小长度
    weight: 1, // 边的权重
  }));

  // 添加所有节点到 dagre 图
  nodes.forEach((node) => {
    g.setNode(node.id, {
      width: node.width || 200,
      height: node.height || 120,
      label: node.data.label,
    });
  });

  // 添加所有边到 dagre 图
  edges.forEach((edge) => {
    if (edge.source && edge.target) {
      g.setEdge(edge.source, edge.target, {
        minlen: 1,
        weight: 1,
      });
    }
  });

  // 执行布局计算
  dagre.layout(g);

  // 更新节点位置和尺寸
  const layoutedNodes = nodes.map((node) => {
    const dagreNode = g.node(node.id);

    // 计算节点中心点位置
    const centerX = dagreNode.x;
    const centerY = dagreNode.y;

    // 转换为 React Flow 的左上角坐标
    const position = {
      x: centerX - dagreNode.width / 2,
      y: centerY - dagreNode.height / 2,
    };

    return {
      ...node,
      position,
      style: {
        width: dagreNode.width,
        height: dagreNode.height,
      },
      draggable: false, // 布局后禁止拖动以保持树结构
      selectable: true,
    };
  });

  // 更新边样式 - 使用默认边类型
  return { nodes: layoutedNodes, edges };
}

function makeNodes(root: GuideNodeData): [GuideNode[], Edge[]] {
  const nodes: GuideNode[] = []; // 结点列表, 按 DFS 顺序排布
  const edges: Edge[] = [];

  function dfs_nodes(nodeData: GuideNodeData): GuideNode {
    const nodeId = nodeData.path;
    const node: GuideNode = {
      id: nodeId,
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: nodeData,
      width: 200,
      height: (() => {
        const num = nodeData.children.length * 33 + 55;
        if (num < 100) {
          return 100;
        } else {
          return num;
        }
      })(),
    };
    nodes.push(node);

    for (const child of nodeData.children) {
      if (!child.expanded) continue;
      const child_node = dfs_nodes(child.data);
      const childId = child_node.id;
      const edge: Edge = {
        id: `edge-${nodeId}-${childId}`,
        source: nodeId,
        target: childId,
      };
      edges.push(edge);
    }
    return node;
  }
  dfs_nodes(root);
  return [nodes, edges];
}

const mainFuncSource = `define i32 @main() {
entry:
  br label %while.cond
while.cond:
  %cond = call i1 @cond()
  br i1 %cond, label %while.body, label %exit
while.body:
  call void @body()
  br label %while.cond
exit:
  ret i32 0
}
`;
function makeMainFuncMock(): MockTreeNode {
  const entryBB: BlockDt = {
    typeid: "Block",
    id: "b:1:1",
    parent: "g:0:1",
    name: "entry",
    source_loc: {
      begin: { line: 2, column: 1 },
      end: { line: 3, column: 23 },
    },
    insts: [
      {
        typeid: "Terminator",
        id: "i:2:1",
        parent: "b:1:1",
        terminator: "i:2:1",
        source_loc: {
          begin: { line: 3, column: 3 },
          end: { line: 3, column: 23 },
        },
        name: undefined,
        opcode: "jmp",
        operands: [],
        succs: [
          {
            id: "j:1:1",
            kind: "Jump",
            target: "b:2:1",
            source_loc: {
              begin: { line: 3, column: 12 },
              end: { line: 3, column: 23 },
            },
          },
        ],
      },
    ],
  };

  const whileCondBB: BlockDt = {
    typeid: "Block",
    id: "b:2:1",
    parent: "g:0:1",
    name: "while.cond",
    source_loc: {
      begin: { line: 4, column: 1 },
      end: { line: 6, column: 23 },
    },
    insts: [
      {
        typeid: "Inst",
        id: "i:3:1",
        parent: "b:2:1",
        source_loc: {
          begin: { line: 5, column: 3 },
          end: { line: 5, column: 23 },
        },
        name: "%cond",
        opcode: "call",
        operands: [
          {
            id: "u:1:1",
            user: { Inst: "i:3:1" },
            kind: "CallArg:0",
            value: {
              Global: "g:1:1",
            },
            source_loc: {
              begin: { line: 5, column: 10 },
              end: { line: 5, column: 14 },
            },
          },
        ],
      },
      {
        typeid: "Terminator",
        id: "i:4:1",
        parent: "b:2:1",
        terminator: "i:4:1",
        source_loc: {
          begin: { line: 6, column: 3 },
          end: { line: 6, column: 23 },
        },
        opcode: "br",
        operands: [
          {
            id: "u:2:1",
            user: { Inst: "i:4:1" },
            kind: "Cond",
            value: {
              Inst: "i:3:1",
            },
            source_loc: {
              begin: { line: 6, column: 7 },
              end: { line: 6, column: 12 },
            },
          },
        ],
        succs: [
          {
            id: "j:2:1",
            kind: "Jump",
            target: "b:3:1",
            source_loc: {
              begin: { line: 6, column: 16 },
              end: { line: 6, column: 27 },
            },
          },
          {
            id: "j:3:1",
            kind: "Jump",
            target: "b:4:1",
            source_loc: {
              begin: { line: 6, column: 32 },
              end: { line: 6, column: 37 },
            },
          },
        ],
      },
    ],
  };

  const whileBodyBB: BlockDt = {
    typeid: "Block",
    id: "b:3:1",
    parent: "g:0:1",
    name: "while.body",
    source_loc: {
      begin: { line: 7, column: 1 },
      end: { line: 9, column: 23 },
    },
    insts: [
      {
        typeid: "Inst",
        id: "i:5:1",
        parent: "b:3:1",
        source_loc: {
          begin: { line: 8, column: 3 },
          end: { line: 8, column: 22 },
        },
        opcode: "call",
        operands: [
          {
            id: "u:3:1",
            user: { Inst: "i:5:1" },
            kind: "CallArg:0",
            value: {
              Global: "g:2:1",
            },
            source_loc: {
              begin: { line: 8, column: 9 },
              end: { line: 8, column: 13 },
            },
          },
        ],
      },
      {
        typeid: "Terminator",
        id: "i:6:1",
        parent: "b:3:1",
        terminator: "i:6:1",
        source_loc: {
          begin: { line: 9, column: 3 },
          end: { line: 9, column: 23 },
        },
        opcode: "jmp",
        operands: [],
        succs: [
          {
            id: "j:4:1",
            kind: "Jump",
            target: "b:2:1",
            source_loc: {
              begin: { line: 9, column: 12 },
              end: { line: 9, column: 23 },
            },
          },
        ],
      },
    ],
  };

  const exitBB: BlockDt = {
    typeid: "Block",
    id: "b:4:1",
    parent: "g:0:1",
    name: "exit",
    source_loc: {
      begin: { line: 10, column: 1 },
      end: { line: 11, column: 12 },
    },
    insts: [
      {
        typeid: "Terminator",
        id: "i:7:1",
        parent: "b:4:1",
        terminator: "i:7:1",
        source_loc: {
          begin: { line: 11, column: 3 },
          end: { line: 11, column: 12 },
        },
        opcode: "ret",
        operands: [
          {
            id: "u:4:1",
            user: { Inst: "i:7:1" },
            kind: "RetVal",
            value: { I32: 0 },
            source_loc: {
              begin: { line: 11, column: 7 },
              end: { line: 11, column: 8 },
            },
          },
        ],
        succs: [],
      },
    ],
  };

  const mainFunc: FuncObjDt = {
    typeid: "Func",
    id: "g:0:1",
    name: "main",
    linkage: "DSOLocal",
    ty: "func:0",
    overview_loc: {
      begin: { line: 1, column: 1 },
      end: { line: 13, column: 1 },
    },
    source: mainFuncSource,
    ret_ty: "i32",
    args: [],
    blocks: [entryBB, whileCondBB, whileBodyBB, exitBB],
  };
  const mainChildren: MockTreeNode[] = (mainFunc.blocks || []).map((bb) => {
    const instChildren: MockTreeNode[] = bb.insts.map((inst) => {
      return {
        label: inst.name || `${inst.id} (${inst.opcode})`,
        ir_obj: inst,
        typeid: getObjText(inst),
        children: [],
      };
    });
    return {
      label: `%${bb.name || bb.id}`,
      ir_obj: bb,
      typeid: "Block",
      children: instChildren,
    };
  });

  return {
    label: "@main",
    ir_obj: mainFunc,
    typeid: "Func",
    children: mainChildren,
  };
}

function makeFuncMock(
  name: string,
  id: string,
  blocks?: BlockDt[],
): MockTreeNode {
  const defaultLoc: SourceLoc = {
    begin: { line: 1, column: 1 },
    end: { line: 1, column: 1 },
  };
  const funcObj: FuncObjDt = {
    typeid: "Func",
    id: id as FuncObjDt["id"],
    name,
    linkage: "External",
    ty: "func:0",
    overview_loc: defaultLoc,
    source: "",
    ret_ty: "i32",
    args: [],
  };
  if (blocks !== undefined) {
    funcObj.blocks = blocks;
  }
  return {
    label: name,
    ir_obj: funcObj,
    typeid: getObjText(funcObj),
    children: [],
  };
}

function buildMockTree(): MockTreeNode {
  const mainFunc = makeMainFuncMock();
  const externCalc = makeFuncMock("calculate", "g:1:1");
  const helperFunc = makeFuncMock("helper", "g:2:1", []);
  return {
    label: "main_module",
    ir_obj: null,
    typeid: "Module",
    children: [mainFunc, externCalc, helperFunc],
  };
}

export default function GuideView() {
  const [nodes, setNodes] = useState<GuideNode[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [expandedMap, setExpandedMap] = useState<Record<string, boolean>>({});
  const [focusedPath, setFocusedPath] = useState<string | null>(null);
  const [rfInstance, setRfInstance] = useState<ReactFlowInstance | null>(null);
  const [menuState, setMenuState] = useState<GuideMenuState | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const mockRoot = useMemo(() => buildMockTree(), []);

  const handleToggleChild = useCallback(
    (childPath: string) => {
      setExpandedMap((prev) => {
        const isExpanded = !!prev[childPath];
        const next = { ...prev };
        if (isExpanded) {
          setFocusedPath(getParentPath(childPath));
          delete next[childPath];
          const descendants = collectDescendantPaths(mockRoot, childPath);
          descendants.forEach((path) => {
            delete next[path];
          });
          return next;
        }
        setFocusedPath(childPath);
        next[childPath] = true;
        return next;
      });
    },
    [mockRoot],
  );

  const handleMenuOpen = useCallback((payload: GuideMenuPayload) => {
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const left = payload.clientX - rect.left;
    const top = payload.clientY - rect.top;
    setMenuState({ ...payload, left, top });
  }, []);

  const handleMenuClose = useCallback(() => {
    setMenuState(null);
  }, []);

  const guideRoot = useMemo(
    () =>
      buildGuideTree(
        mockRoot,
        "root",
        expandedMap,
        handleToggleChild,
        handleMenuOpen,
      ),
    [mockRoot, expandedMap, handleToggleChild, handleMenuOpen],
  );

  useEffect(() => {
    // 生成节点和边
    const [initialNodes, initialEdges] = makeNodes(guideRoot);

    // 应用布局
    const { nodes: layoutedNodes, edges: layoutedEdges } = layoutNodes(
      initialNodes,
      initialEdges,
    );

    // 使用 requestAnimationFrame   避免同步 setState 警告
    requestAnimationFrame(() => {
      setNodes(layoutedNodes);
      setEdges(layoutedEdges);
    });
  }, [guideRoot]);

  useEffect(() => {
    if (!focusedPath || !rfInstance) return;
    const node = nodes.find((item) => item.id === focusedPath);
    if (!node) return;
    const width = Number(node.style?.width ?? node.width ?? 0) || 0;
    const height = Number(node.style?.height ?? node.height ?? 0) || 0;
    const centerX = node.position.x + width / 2;
    const centerY = node.position.y + height / 2;
    rfInstance.setCenter(centerX, centerY, { zoom: 1.05, duration: 200 });
  }, [focusedPath, nodes, rfInstance]);

  // 处理节点点击
  const handleNodeClick: NodeMouseHandler = (event, node) => {
    event.stopPropagation();
    const data = node.data as GuideNodeData | undefined;
    const label = data?.label ?? node.id;
    alert(`点击了节点: ${label}\nID: ${node.id}`);
  };

  // 处理边点击
  const handleEdgeClick = (event: React.MouseEvent, edge: Edge) => {
    event.stopPropagation();
    alert(`点击了边: ${edge.source} → ${edge.target}`);
  };

  const getMenuTypeLabel = (nodeType: IRArchObjTy): string => {
    switch (nodeType) {
      case "Module":
        return "模块";
      case "Func":
      case "ExternFunc":
        return "函数";
      case "Global":
      case "ExternGlobal":
        return "全局变量";
      case "Block":
        return "基本块";
      case "Inst":
      case "Phi":
      case "Terminator":
        return "指令";
      default:
        return "对象";
    }
  };

  const handleMenuItemClick = (item: GuideMenuItem) => {
    if (!menuState) return;
    if (item.key === "expand-one") {
      const node = getMockNodeByPath(mockRoot, menuState.path);
      if (node) {
        setExpandedMap((prev) => {
          const next = { ...prev };
          node.children.forEach((_, index) => {
            const childPath = getMockChildPath(menuState.path, index);
            next[childPath] = true;
          });
          return next;
        });
        setFocusedPath(menuState.path);
      }
      setMenuState(null);
      return;
    }

    if (item.key === "expand-all") {
      const paths = collectDescendantPaths(mockRoot, menuState.path);
      if (paths.length > 0) {
        setExpandedMap((prev) => {
          const next = { ...prev };
          paths.forEach((path) => {
            next[path] = true;
          });
          return next;
        });
        setFocusedPath(menuState.path);
      }
      setMenuState(null);
      return;
    }
    const typeText = getMenuTypeLabel(menuState.nodeType);
    alert(`操作: ${item.label}\n对象: ${menuState.displayName}\n类型: ${typeText}`);
    setMenuState(null);
  };

  return (
    <React.Suspense fallback={GuideViewText}>
      <div
        className="guide-view"
        ref={containerRef}
        style={{ width: "100%", height: "100%", position: "relative" }}
        onClick={handleMenuClose}
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeDoubleClick={handleNodeClick}
          onEdgeClick={handleEdgeClick}
          onInit={setRfInstance}
          fitView
          fitViewOptions={{ padding: 0.2 }}
        >
          <Background />
          <Controls />
        </ReactFlow>
        {menuState && (
          <div
            style={{
              position: "absolute",
              top: menuState.top,
              left: menuState.left,
              zIndex: 20,
              width: "160px",
              backgroundColor: "white",
              border: "1px solid #e5e7eb",
              borderRadius: "6px",
              boxShadow: "0 8px 24px rgba(0, 0, 0, 0.12)",
              padding: "6px 0",
              display: "flex",
              flexDirection: "column",
            }}
            onClick={(e) => e.stopPropagation()}
            onMouseLeave={handleMenuClose}
          >
            {menuState.items.map((item) => (
              <button
                key={item.key}
                style={{
                  width: "100%",
                  textAlign: "left",
                  background: "transparent",
                  border: "none",
                  padding: "6px 12px",
                  fontSize: "12px",
                  color: "#111827",
                  cursor: "pointer",
                }}
                onClick={() => handleMenuItemClick(item)}
              >
                {item.label}
              </button>
            ))}
          </div>
        )}
      </div>
    </React.Suspense>
  );
}
