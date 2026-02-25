import React from "react";
import DagreTreeExample from "./DagreTreeExample";

const DagreExamplePage: React.FC = () => {
  return (
    <div
      style={{
        width: "100vw",
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        backgroundColor: "#f8fafc",
      }}
    >
      <header
        style={{
          padding: "24px",
          backgroundColor: "white",
          borderBottom: "1px solid #e2e8f0",
          boxShadow: "0 1px 3px rgba(0, 0, 0, 0.1)",
        }}
      >
        <h1
          style={{
            margin: "0 0 8px 0",
            color: "#1e293b",
            fontSize: "28px",
            fontWeight: "bold",
          }}
        >
          Dagre 树布局示例
        </h1>
        <p
          style={{
            margin: "0",
            color: "#64748b",
            fontSize: "16px",
            lineHeight: "1.5",
          }}
        >
          使用 dagre 库实现从左向右的树结构布局，适用于展示层次化数据如 IR
          模块结构。
        </p>
      </header>

      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          padding: "24px",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 300px",
            gap: "24px",
            height: "100%",
          }}
        >
          {/* 主内容区域 */}
          <div
            style={{
              backgroundColor: "white",
              borderRadius: "8px",
              border: "1px solid #e2e8f0",
              overflow: "hidden",
              boxShadow: "0 1px 3px rgba(0, 0, 0, 0.1)",
            }}
          >
            <DagreTreeExample />
          </div>

          {/* 侧边栏说明 */}
          <div
            style={{
              backgroundColor: "white",
              borderRadius: "8px",
              border: "1px solid #e2e8f0",
              padding: "24px",
              boxShadow: "0 1px 3px rgba(0, 0, 0, 0.1)",
              overflowY: "auto",
            }}
          >
            <h2
              style={{
                margin: "0 0 16px 0",
                color: "#1e293b",
                fontSize: "20px",
                fontWeight: "600",
              }}
            >
              配置说明
            </h2>

            <div style={{ marginBottom: "24px" }}>
              <h3
                style={{
                  margin: "0 0 8px 0",
                  color: "#334155",
                  fontSize: "16px",
                  fontWeight: "500",
                }}
              >
                Dagre 布局参数
              </h3>
              <ul
                style={{
                  margin: "0",
                  paddingLeft: "20px",
                  color: "#475569",
                  fontSize: "14px",
                  lineHeight: "1.6",
                }}
              >
                <li>
                  <code>rankdir: 'LR'</code> - 从左向右布局
                </li>
                <li>
                  <code>nodesep: 60</code> - 节点水平间距
                </li>
                <li>
                  <code>ranksep: 120</code> - 层级垂直间距
                </li>
                <li>
                  <code>marginx: 50</code> - 水平边距
                </li>
                <li>
                  <code>marginy: 50</code> - 垂直边距
                </li>
                <li>
                  <code>ranker: 'network-simplex'</code> - 布局算法
                </li>
              </ul>
            </div>

            <div style={{ marginBottom: "24px" }}>
              <h3
                style={{
                  margin: "0 0 8px 0",
                  color: "#334155",
                  fontSize: "16px",
                  fontWeight: "500",
                }}
              >
                节点类型
              </h3>
              <div
                style={{ display: "flex", flexDirection: "column", gap: "8px" }}
              >
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <div
                    style={{
                      width: "16px",
                      height: "16px",
                      borderRadius: "50%",
                      backgroundColor: "#ef4444",
                      color: "white",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "9px",
                      fontFamily: '"Cascadia Mono", monospace',
                      flexShrink: 0,
                    }}
                  >
                    M
                  </div>
                  <span style={{ fontSize: "14px", color: "#475569" }}>
                    模块 (Module)
                  </span>
                </div>
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <div
                    style={{
                      width: "16px",
                      height: "16px",
                      borderRadius: "50%",
                      backgroundColor: "#fbbf24",
                      color: "black",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "9px",
                      fontFamily: '"Cascadia Mono", monospace',
                      flexShrink: 0,
                    }}
                  >
                    Fx
                  </div>
                  <span style={{ fontSize: "14px", color: "#475569" }}>
                    函数 (Function)
                  </span>
                </div>
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <div
                    style={{
                      width: "16px",
                      height: "16px",
                      borderRadius: "50%",
                      backgroundColor: "#f97316",
                      color: "white",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "9px",
                      fontFamily: '"Cascadia Mono", monospace',
                      flexShrink: 0,
                    }}
                  >
                    B
                  </div>
                  <span style={{ fontSize: "14px", color: "#475569" }}>
                    基本块 (Block)
                  </span>
                </div>
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <div
                    style={{
                      width: "16px",
                      height: "16px",
                      borderRadius: "50%",
                      backgroundColor: "#22c55e",
                      color: "black",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "9px",
                      fontFamily: '"Cascadia Mono", monospace',
                      flexShrink: 0,
                    }}
                  >
                    I
                  </div>
                  <span style={{ fontSize: "14px", color: "#475569" }}>
                    指令 (Instruction)
                  </span>
                </div>
              </div>
            </div>

            <div style={{ marginBottom: "24px" }}>
              <h3
                style={{
                  margin: "0 0 8px 0",
                  color: "#334155",
                  fontSize: "16px",
                  fontWeight: "500",
                }}
              >
                交互说明
              </h3>
              <ul
                style={{
                  margin: "0",
                  paddingLeft: "20px",
                  color: "#475569",
                  fontSize: "14px",
                  lineHeight: "1.6",
                }}
              >
                <li>点击节点查看详细信息</li>
                <li>使用右上角控制面板缩放和平移</li>
                <li>右下角迷你地图显示整体布局</li>
                <li>节点顶栏 ⋯ 按钮可打开操作菜单</li>
              </ul>
            </div>

            <div>
              <h3
                style={{
                  margin: "0 0 8px 0",
                  color: "#334155",
                  fontSize: "16px",
                  fontWeight: "500",
                }}
              >
                代码集成
              </h3>
              <p
                style={{
                  margin: "0",
                  color: "#475569",
                  fontSize: "14px",
                  lineHeight: "1.6",
                }}
              >
                要集成到现有项目，参考 <code>DagreTreeExample.tsx</code> 中的{" "}
                <code>layoutTree</code> 函数，
                将你的节点和边数据传入即可获得从左向右的树布局。
              </p>
            </div>
          </div>
        </div>
      </div>

      <footer
        style={{
          padding: "16px 24px",
          backgroundColor: "white",
          borderTop: "1px solid #e2e8f0",
          color: "#64748b",
          fontSize: "14px",
          textAlign: "center",
        }}
      >
        <p style={{ margin: "0" }}>
          Visual Remusys - Dagre 树布局示例 | 使用 dagre 和 @xyflow/react
        </p>
      </footer>
    </div>
  );
};

export default DagreExamplePage;
