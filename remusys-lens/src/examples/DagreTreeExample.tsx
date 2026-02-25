import React, { useEffect, useState } from "react";
import * as ReactFlow from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import * as dagre from "dagre";

// 定义节点数据类型
type TreeNodeData = {
  label: string;
  type: "module" | "function" | "block" | "instruction";
};

type TreeNode = ReactFlow.Node<TreeNodeData>;
type TreeEdge = ReactFlow.Edge;

// 自定义节点组件
const TreeNodeComponent: React.FC<any> = ({ data }) => {
  const getTypeColor = (type: string) => {
    switch (type) {
      case "module":
        return "#ef4444"; // 红色
      case "function":
        return "#fbbf24"; // 黄色
      case "block":
        return "#f97316"; // 橙色
      case "instruction":
        return "#22c55e"; // 绿色
      default:
        return "#6b7280"; // 灰色
    }
  };

  const getTypeText = (type: string) => {
    switch (type) {
      case "module":
        return "M";
      case "function":
        return "Fx";
      case "block":
        return "B";
      case "instruction":
        return "I";
      default:
        return "?";
    }
  };

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        border: "1px solid #d1d5db",
        borderRadius: "3px",
        backgroundColor: "white",
        boxShadow: "0 1px 3px rgba(0, 0, 0, 0.1)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {/* 顶栏 */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          padding: "8px 12px",
          backgroundColor: "#f9fafb",
          borderBottom: "1px solid #e5e7eb",
        }}
      >
        {/* 类型图标 */}
        <div
          style={{
            width: "16px",
            height: "16px",
            borderRadius: "50%",
            backgroundColor: getTypeColor(data.type),
            color:
              data.type === "function" || data.type === "instruction"
                ? "black"
                : "white",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: "9px",
            fontFamily: '"Cascadia Mono", monospace',
            marginRight: "8px",
            flexShrink: 0,
          }}
        >
          {getTypeText(data.type)}
        </div>

        {/* 节点名称 */}
        <div
          style={{
            flex: 1,
            fontSize: "14px",
            fontWeight: "500",
            color: "#111827",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {data.label}
        </div>

        {/* 操作按钮 */}
        <button
          style={{
            background: "none",
            border: "none",
            color: "#6b7280",
            cursor: "pointer",
            fontSize: "16px",
            padding: "0 4px",
          }}
          onClick={(e) => {
            e.stopPropagation();
            alert(`操作菜单: ${data.label}`);
          }}
        >
          ⋯
        </button>
      </div>

      {/* 内容区域 */}
      <div
        style={{
          padding: "12px",
          fontSize: "12px",
          color: "#4b5563",
          flex: 1,
        }}
      >
        <div style={{ marginBottom: "4px" }}>
          <strong>类型:</strong> {data.type}
        </div>
        <div>
          <strong>ID:</strong> {data.id?.substring(0, 8)}...
        </div>
      </div>
    </div>
  );
};

// 使用 dagre 进行从左向右的树布局
function layoutTree(
  nodes: TreeNode[],
  edges: TreeEdge[],
): { nodes: TreeNode[]; edges: TreeEdge[] } {
  // 创建 dagre 图实例
  const g = new dagre.graphlib.Graph({
    compound: false, // 设置为 true 可以支持复合节点（父子关系）
  });

  // 设置图的布局配置
  g.setGraph({
    rankdir: "LR", // 方向：从左到右 (LR = Left to Right)
    align: "UL", // 对齐方式：上左对齐
    nodesep: 60, // 同一层级节点之间的水平间距
    ranksep: 120, // 不同层级之间的垂直间距
    marginx: 50, // 水平边距
    marginy: 50, // 垂直边距
    ranker: "network-simplex", // 排名算法：网络单纯形法（适合树结构）
  });

  // 设置默认节点尺寸
  g.setDefaultNodeLabel(() => ({
    width: 180, // 节点宽度
    height: 100, // 节点高度
  }));

  // 设置默认边配置
  g.setDefaultEdgeLabel(() => ({
    minlen: 1, // 边的最小长度
    weight: 1, // 边的权重（影响布局紧凑度）
  }));

  // 添加所有节点到 dagre 图
  nodes.forEach((node) => {
    g.setNode(node.id, {
      width: 180,
      height: 100,
      label: node.data.label,
    });
  });

  // 添加所有边到 dagre 图
  edges.forEach((edge) => {
    g.setEdge(edge.source, edge.target, {
      minlen: 1,
      weight: 1,
    });
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
    };
  });

  return { nodes: layoutedNodes, edges };
}

// 生成示例树数据
function generateTreeData(): { nodes: TreeNode[]; edges: TreeEdge[] } {
  const nodes: TreeNode[] = [
    // 根节点 - 模块
    {
      id: "module-1",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "main_module",
        type: "module",
      },
    },

    // 第一层 - 函数
    {
      id: "function-1",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "main()",
        type: "function",
      },
    },
    {
      id: "function-2",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "calculate()",
        type: "function",
      },
    },
    {
      id: "function-3",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "helper()",
        type: "function",
      },
    },

    // 第二层 - 基本块（属于 main 函数）
    {
      id: "block-1",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "entry",
        type: "block",
      },
    },
    {
      id: "block-2",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "while.cond",
        type: "block",
      },
    },
    {
      id: "block-3",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "while.body",
        type: "block",
      },
    },
    {
      id: "block-4",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "exit",
        type: "block",
      },
    },

    // 第三层 - 指令（属于 entry 块）
    {
      id: "inst-1",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "alloca %i",
        type: "instruction",
      },
    },
    {
      id: "inst-2",
      type: "guideNode",
      position: { x: 0, y: 0 },
      data: {
        label: "store 0, %i",
        type: "instruction",
      },
    },
  ];

  const edges: TreeEdge[] = [
    // 模块到函数的边
    {
      id: "e-module-f1",
      source: "module-1",
      target: "function-1",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
    {
      id: "e-module-f2",
      source: "module-1",
      target: "function-2",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
    {
      id: "e-module-f3",
      source: "module-1",
      target: "function-3",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },

    // 函数到基本块的边（main 函数）
    {
      id: "e-f1-b1",
      source: "function-1",
      target: "block-1",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
    {
      id: "e-f1-b2",
      source: "function-1",
      target: "block-2",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
    {
      id: "e-f1-b3",
      source: "function-1",
      target: "block-3",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
    {
      id: "e-f1-b4",
      source: "function-1",
      target: "block-4",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },

    // 基本块到指令的边
    {
      id: "e-b1-i1",
      source: "block-1",
      target: "inst-1",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
    {
      id: "e-b1-i2",
      source: "block-1",
      target: "inst-2",
      type: "smoothstep",
      animated: false,
      style: { stroke: "#94a3b8", strokeWidth: 2 },
    },
  ];

  return { nodes, edges };
}

// 主组件
const DagreTreeExample: React.FC = () => {
  const [nodes, setNodes] = useState<TreeNode[]>([]);
  const [edges, setEdges] = useState<TreeEdge[]>([]);

  const nodeTypes: ReactFlow.NodeTypes = {
    guideNode: TreeNodeComponent,
  };

  const edgeTypes: ReactFlow.EdgeTypes = {
    smoothstep: ReactFlow.SmoothStepEdge,
  };

  // 初始化数据和布局
  useEffect(() => {
    // 生成树数据
    const { nodes: initialNodes, edges: initialEdges } = generateTreeData();

    // 应用 dagre 布局
    const { nodes: layoutedNodes, edges: layoutedEdges } = layoutTree(
      initialNodes,
      initialEdges,
    );

    setNodes(layoutedNodes);
    setEdges(layoutedEdges);
  }, []);

  // 处理节点点击（展开/折叠）
  const handleNodeClick = (event: React.MouseEvent, node: TreeNode) => {
    event.stopPropagation();
    alert(
      `点击了节点: ${node.data.label}\nID: ${node.id}\n类型: ${node.data.type}`,
    );
  };

  // 处理边点击
  const handleEdgeClick = (event: React.MouseEvent, edge: TreeEdge) => {
    event.stopPropagation();
    alert(`点击了边: ${edge.source} → ${edge.target}`);
  };

  return (
    <div style={{ width: "100%", height: "800px" }}>
      <div
        style={{
          marginBottom: "16px",
          padding: "16px",
          backgroundColor: "#f8fafc",
        }}
      >
        <h2 style={{ margin: "0 0 8px 0", color: "#1e293b" }}>
          Dagre 从左向右树布局示例
        </h2>
        <p style={{ margin: "0", color: "#475569" }}>
          这是一个使用 dagre 库实现的从左向右树结构布局示例。 布局方向：从左到右
          (rankdir: 'LR')，适合展示层次结构数据。
        </p>
        <div style={{ marginTop: "12px", fontSize: "14px", color: "#64748b" }}>
          <strong>布局配置:</strong> rankdir: LR, nodesep: 60, ranksep: 120
        </div>
      </div>

      <ReactFlow.ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onNodeClick={handleNodeClick}
        onEdgeClick={handleEdgeClick}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        proOptions={{ hideAttribution: true }}
      >
        <ReactFlow.Background />
        <ReactFlow.Controls />
      </ReactFlow.ReactFlow>

      <div
        style={{
          marginTop: "16px",
          padding: "16px",
          backgroundColor: "#f8fafc",
          fontSize: "14px",
        }}
      >
        <h3 style={{ margin: "0 0 8px 0", color: "#1e293b" }}>使用说明：</h3>
        <ul style={{ margin: "0", paddingLeft: "20px", color: "#475569" }}>
          <li>点击节点查看详细信息</li>
          <li>使用控制面板缩放和平移视图</li>
          <li>迷你地图显示整体布局结构</li>
          <li>
            节点颜色表示不同类型：红(模块)、黄(函数)、橙(基本块)、绿(指令)
          </li>
        </ul>
      </div>
    </div>
  );
};

export default DagreTreeExample;
