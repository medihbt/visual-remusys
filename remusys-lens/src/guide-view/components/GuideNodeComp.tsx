import React from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type {
  IRArchObj,
  GuideNodeChildStat,
  GuideNode,
  IRArchObjTy,
} from "../GuideView";
import { getObjText } from "../GuideView";

// 解析 ID 格式: `{pool type}:{slot index}:{slot generation}`
const parseID = (id: string) => {
  const parts = id.split(":");
  return parts.length === 3
    ? { poolType: parts[0], slotIndex: parts[1], slotGeneration: parts[2] }
    : { poolType: "?", slotIndex: "?", slotGeneration: "?" };
};

// 获取池类型描述
const getPoolTypeDescription = (poolType: string) =>
  ({
    g: "全局对象",
    b: "基本块",
    i: "指令",
    e: "表达式",
    u: "使用",
    j: "跳转目标",
  })[poolType] || "未知类型";

// 格式化显示 ID
const formatID = (id: string) => {
  const { poolType, slotIndex, slotGeneration } = parseID(id);
  return `${poolType}:${slotIndex}:${slotGeneration}`;
};

// 获取子节点显示信息
const getChildDisplayInfo = (child: GuideNodeChildStat) =>
  child.expanded === false
    ? {
        label: child.label,
        ir_obj: null as IRArchObj,
        svgIcon: svgIconFromNodeType(child.typeid),
      }
    : {
        label: child.data.label,
        ir_obj: child.data.ir_obj,
        svgIcon: svgIconFromNodeType(getObjText(child.data.ir_obj)),
      };

// 获取子节点行背景色
const getChildRowBackgroundColor = (child: GuideNodeChildStat) =>
  child.expanded ? "#f3f4f6" : "#ffffff";

function svgIconFromNodeType(ty: IRArchObjTy): React.ReactElement {
  function getColor(ty: IRArchObjTy): string {
    switch (ty) {
      case "Module":
        return "#ef4444"; // 红色
      case "Global":
        return "#242480"; // 靛蓝
      case "ExternGlobal":
        return "#6b7280"; // 灰色
      case "Func":
        return "#fbbf24"; // 黄色
      case "ExternFunc":
        return "#6b7280"; // 灰色
      case "Block":
        return "#f97316"; // 橙色
      case "Inst":
        return "#22c55e"; // 绿色
      case "Phi":
        return "#38bdf8"; // 浅蓝色
      case "Terminator":
        return "#f97316"; // 橙色
      default:
        return "#6b7280"; // 默认灰色
    }
  }
  function getText(ty: IRArchObjTy): string {
    switch (ty) {
      case "Module":
        return "M";
      case "Global":
      case "ExternGlobal":
        return "Gv";
      case "Func":
      case "ExternFunc":
        return "Fx";
      case "Block":
        return "B";
      case "Inst":
        return "I";
      case "Phi":
        return "Φ";
      case "Terminator":
        return "Ti";
      default:
        return "?";
    }
  }
  function getTextColor(ty: IRArchObjTy): string {
    switch (ty) {
      case "Module":
      case "Global":
      case "ExternGlobal":
      case "Block":
      case "Terminator":
        return "white";
      case "Func":
      case "ExternFunc":
      case "Inst":
      case "Phi":
        return "black";
      default:
        return "white";
    }
  }
  const typeColor = getColor(ty);
  const typeText = getText(ty);
  const textColor = getTextColor(ty);
  return (
    <svg width="16" height="16" viewBox="0 0 16 16">
      <circle cx="8" cy="8" r="8" fill={typeColor} />
      <text
        x="8"
        y="10"
        textAnchor="middle"
        fill={textColor}
        fontSize="9"
        fontFamily='"Cascadia Mono", monospace'
        fontWeight="normal"
      >
        {typeText}
      </text>
    </svg>
  );
}

// 获取节点显示名称
const getNodeDisplayName = (ir_obj: IRArchObj, label: string) => {
  if (ir_obj === null) return label || "Module";

  switch (ir_obj.typeid) {
    case "Func":
      return ir_obj.name || label || "function";
    case "GlobalVar":
      return ir_obj.name || label || "global";
    case "Block":
      return ir_obj.name || label || "block";
    case "Inst":
    case "Phi":
    case "Terminator":
      return ir_obj.opcode || label || "instruction";
    default:
      return label;
  }
};

type GuideNodeProps = NodeProps<GuideNode>;

export const GuideNodeComp: React.FC<GuideNodeProps> = ({ data, id }) => {
  const { label, ir_obj, children } = data;
  const nodeType = getObjText(ir_obj);
  const displayName = getNodeDisplayName(ir_obj, label);
  const svgIcon = svgIconFromNodeType(nodeType);

  const handleMenuClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (nodeType === "Module") {
      alert(`操作菜单: ${displayName}\n类型: 模块`);
    } else {
      const { poolType, slotIndex, slotGeneration } = parseID(id);
      const poolDesc = getPoolTypeDescription(poolType);
      alert(
        `操作菜单: ${displayName}\nID: ${formatID(id)}\n类型: ${poolDesc}\n槽索引: ${slotIndex}\n槽代: ${slotGeneration}`,
      );
    }
  };

  const handleFocusClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (nodeType === "Module") {
      alert(`聚焦到: ${displayName}\n类型: 模块`);
    } else {
      const { poolType } = parseID(id);
      const poolDesc = getPoolTypeDescription(poolType);
      alert(`聚焦到: ${displayName}\nID: ${formatID(id)}\n类型: ${poolDesc}`);
    }
  };

  const handleChildClick = (index: number, e: React.MouseEvent) => {
    e.stopPropagation();
    const child = children[index];
    const message =
      child.expanded === false
        ? `展开子节点: ${child.label}\n点击整行任意位置展开`
        : `收起子节点: ${child.data.label}\n点击整行任意位置收起`;
    alert(message);
  };

  const listChildren = (child: GuideNodeChildStat, index: number) => {
    const childInfo = getChildDisplayInfo(child);
    const isExpanded = child.expanded;
    const title = `点击${isExpanded ? "收起" : "展开"}此节点`;

    const handleMouseEnter = (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isExpanded) e.currentTarget.style.backgroundColor = "#f9fafb";
    };

    const handleMouseLeave = (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isExpanded) e.currentTarget.style.backgroundColor = "#ffffff";
    };

    return (
      <div
        key={index}
        style={{
          display: "flex",
          alignItems: "center",
          padding: "6px 12px",
          cursor: "pointer",
          borderBottom:
            index < children.length - 1 ? "1px solid #f3f4f6" : "none",
          backgroundColor: getChildRowBackgroundColor(child),
          transition: "background-color 0.2s",
        }}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        onClick={(e) => handleChildClick(index, e)}
        title={title}
      >
        {/* 子节点类型图标 */}
        <div
          style={{
            marginRight: "8px",
            flexShrink: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
          title={isExpanded ? "已展开" : "未展开"}
        >
          {childInfo.svgIcon}
        </div>

        {/* 子节点名称 */}
        <div
          style={{
            flex: 1,
            fontSize: "12px",
            color: "#4b5563",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
          title={childInfo.label}
        >
          {childInfo.label}
        </div>

        {/* 展开/收起按钮 - radio button 样式 */}
        <div
          style={{
            width: "16px",
            height: "16px",
            borderRadius: "50%",
            border: "2px solid #d1d5db",
            backgroundColor: isExpanded ? "#3b82f6" : "transparent",
            marginLeft: "8px",
            flexShrink: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            position: "relative",
          }}
          title={isExpanded ? "已展开 (点击收起)" : "未展开 (点击展开)"}
        >
          {isExpanded && (
            <div
              style={{
                width: "8px",
                height: "8px",
                borderRadius: "50%",
                backgroundColor: "white",
              }}
            />
          )}
        </div>
      </div>
    );
  };

  return (
    <>
      <Handle type="target" position={Position.Left} isConnectable={true} />
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
          fontFamily: "system-ui, -apple-system, sans-serif",
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
            cursor: "pointer",
          }}
          onClick={handleFocusClick}
          onContextMenu={(e) => {
            e.preventDefault();
            handleMenuClick(e);
          }}
        >
          {/* 类型图标 */}
          <div
            style={{
              marginRight: "8px",
              flexShrink: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            {svgIcon}
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
            {displayName}
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
              lineHeight: "1",
            }}
            onClick={handleMenuClick}
          >
            ⋯
          </button>
        </div>

        {/* 子节点列表 */}
        <div
          ref={(el) => {
            if (el) {
              // 保存滚动容器的引用
              const scrollContainer = el;
              // 添加鼠标滚轮事件监听
              const handleWheel = (e: WheelEvent) => {
                // 如果容器有滚动条，则阻止事件冒泡，让容器自己滚动
                if (
                  scrollContainer.scrollHeight > scrollContainer.clientHeight
                ) {
                  e.stopPropagation();
                  // 计算新的滚动位置
                  const newScrollTop = scrollContainer.scrollTop + e.deltaY;
                  // 确保滚动在合理范围内
                  const maxScrollTop =
                    scrollContainer.scrollHeight - scrollContainer.clientHeight;
                  scrollContainer.scrollTop = Math.max(
                    0,
                    Math.min(newScrollTop, maxScrollTop),
                  );
                }
              };
              // 移除旧的事件监听器（如果有的话）
              scrollContainer.removeEventListener(
                "wheel",
                handleWheel as EventListener,
              );
              // 添加新的事件监听器
              scrollContainer.addEventListener(
                "wheel",
                handleWheel as EventListener,
                { passive: false },
              );
            }
          }}
          style={{
            padding: "8px 0",
            maxHeight: "120px",
            overflowY: "auto",
            // 添加平滑滚动
            scrollBehavior: "smooth",
          }}
          onWheel={(e) => {
            // React 事件处理，阻止事件冒泡到 React Flow
            e.stopPropagation();
          }}
        >
          {children.map(listChildren)}

          {children.length === 0 && (
            <div
              style={{
                padding: "8px 12px",
                fontSize: "11px",
                color: "#9ca3af",
                textAlign: "center",
                fontStyle: "italic",
              }}
            >
              无子节点
            </div>
          )}
        </div>
      </div>
      <Handle type="source" position={Position.Right} isConnectable={true} />
    </>
  );
};
