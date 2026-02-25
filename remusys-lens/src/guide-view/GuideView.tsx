import { type Node, type Edge } from "@xyflow/react";
import { ReactFlow, Controls, Background } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import * as dagre from "dagre";
import React, { useEffect, useState } from "react";
import type { BlockDt, FuncObjDt, GlobalObjDt, InstDt } from "../ir/ir";
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

export type IRArchObj = GlobalObjDt | BlockDt | InstDt | null;
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
};

export type GuideNode = Node<GuideNodeData, "guideNode">;
export type GuideNodeChildStat =
  | { expanded: false; typeid: IRArchObjTy; label: string }
  | { expanded: true; data: GuideNodeData };

const nodeTypes = { guideNode: GuideNodeComp };

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
    const nodeId = `n${nodes.length}`;
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
        animated: true,
        style: {
          strokeColor: "black",
          strokeWidth: 3,
        },
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
function makeMain(): GuideNodeData {
  const entryBB: BlockDt = {
    typeid: "Block",
    id: "b:1:1",
    source_loc: {
      begin: { line: 2, column: 1 },
      end: { line: 3, column: 23 },
    },
    insts: [
      {
        typeid: "Terminator",
        id: "i:2:1",
        source_loc: {
          begin: { line: 3, column: 3 },
          end: { line: 3, column: 23 },
        },
        name: null,
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
    source_loc: {
      begin: { line: 4, column: 1 },
      end: { line: 6, column: 23 },
    },
    insts: [
      {
        typeid: "Inst",
        id: "i:3:1",
        source_loc: {
          begin: { line: 5, column: 3 },
          end: { line: 5, column: 23 },
        },
        name: "%cond",
        opcode: "call",
        operands: [
          {
            id: "u:1:1",
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
        source_loc: {
          begin: { line: 6, column: 3 },
          end: { line: 6, column: 23 },
        },
        name: null,
        opcode: "br",
        operands: [
          {
            id: "u:2:1",
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
    source_loc: {
      begin: { line: 7, column: 1 },
      end: { line: 9, column: 23 },
    },
    insts: [
      {
        typeid: "Inst",
        id: "i:5:1",
        source_loc: {
          begin: { line: 8, column: 3 },
          end: { line: 8, column: 22 },
        },
        name: null,
        opcode: "call",
        operands: [
          {
            id: "u:3:1",
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
        source_loc: {
          begin: { line: 9, column: 3 },
          end: { line: 9, column: 23 },
        },
        name: null,
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
    source_loc: {
      begin: { line: 10, column: 1 },
      end: { line: 11, column: 12 },
    },
    insts: [
      {
        typeid: "Terminator",
        id: "i:7:1",
        source_loc: {
          begin: { line: 11, column: 3 },
          end: { line: 11, column: 12 },
        },
        name: null,
        opcode: "ret",
        operands: [
          {
            id: "u:4:1",
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
  return {
    label: "@main",
    ir_obj: mainFunc,
    children: (mainFunc.blocks || []).map((bb) => {
      return {
        expanded: true,
        data: {
          label: `%${bb.name || bb.id}`,
          ir_obj: bb,
          children: bb.insts.map((inst) => {
            return {
              expanded: false,
              typeid: "Inst",
              label: inst.name || inst.id,
            };
          }),
        },
      };
    }),
  };
}

export default function GuideView() {
  const [nodes, setNodes] = useState<GuideNode[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);

  useEffect(() => {
    // 创建示例根节点数据
    const rootNodeData: GuideNodeData = {
      label: "main_module",
      ir_obj: null,
      children: [
        {
          expanded: true,
          data: makeMain(),
        },
        {
          expanded: false,
          typeid: "ExternFunc",
          label: "calculate",
        },
        {
          expanded: false,
          typeid: "Func",
          label: "helper",
        },
      ],
    };

    // 生成节点和边
    const [initialNodes, initialEdges] = makeNodes(rootNodeData);

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
  }, []);

  // 处理节点点击
  const handleNodeClick = (event: React.MouseEvent, node: GuideNode) => {
    event.stopPropagation();
    alert(`点击了节点: ${node.data.label}\nID: ${node.id}`);
  };

  // 处理边点击
  const handleEdgeClick = (event: React.MouseEvent, edge: Edge) => {
    event.stopPropagation();
    alert(`点击了边: ${edge.source} → ${edge.target}`);
  };

  return (
    <React.Suspense fallback={GuideViewText}>
      <div className="guide-view" style={{ width: "100%", height: "100%" }}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeClick={handleNodeClick}
          onEdgeClick={handleEdgeClick}
          fitView
          fitViewOptions={{ padding: 0.2 }}
        >
          <Background />
          <Controls />
        </ReactFlow>
      </div>
    </React.Suspense>
  );
}
