// components/SimpleMenu.tsx
import React from "react";
import type { TreeNodeKind } from "../guide-view-tree";

interface SimpleMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  onAction: (action: string) => void;
  kind: TreeNodeKind;
}

export const SimpleMenu: React.FC<SimpleMenuProps> = ({ x, y, onClose, onAction }) => {
  // 1. 事件处理：防止菜单点击穿透
  const handleMenuClick = (e: React.MouseEvent) => {
    e.stopPropagation();
  };

  // 2. 位置计算：确保菜单不会超出屏幕
  const calculatePosition = () => {
    // 限制菜单在屏幕范围内
    const screenWidth = window.innerWidth;
    const screenHeight = window.innerHeight;
    
    let left = x;
    let top = y;
    
    // 如果超出右侧边界，向左偏移
    if (x + 200 > screenWidth) {
      left = screenWidth - 200;
    }
    
    // 如果超出底部边界，向上偏移
    if (y + 150 > screenHeight) {
      top = screenHeight - 150;
    }
    
    return { left, top };
  };

  const { left, top } = calculatePosition();

  return (
    <div
      style={{
        position: "fixed",
        left: `${left}px`,
        top: `${top}px`,
        backgroundColor: "#fff",
        border: "1px solid #e5e7eb",
        borderRadius: "8px",
        boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
        width: "200px",
        zIndex: 1000,
        fontFamily: "system-ui, sans-serif",
        overflow: "hidden",
        userSelect: "none"
      }}
      onClick={handleMenuClick}
    >
      {/* 菜单项 */}
      <div
        onClick={() => onAction('expand-all')}
        style={{
          padding: "10px 15px",
          cursor: "pointer",
          fontSize: "13px",
          color: "#4b5563",
          transition: "background-color 0.1s"
        }}
        onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
        onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "#fff")}
      >
        展开全部子节点
      </div>
      
      <div
        onClick={() => onAction('focus')}
        style={{
          padding: "10px 15px",
          cursor: "pointer",
          fontSize: "13px",
          color: "#4b5563",
          borderTop: "1px solid #e5e7eb",
          transition: "background-color 0.1s"
        }}
        onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "#f3f4f6")}
        onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "#fff")}
      >
        聚焦此处
      </div>
      
      {/* 关闭按钮（可选） */}
      <div
        onClick={onClose}
        style={{
          padding: "10px 15px",
          cursor: "pointer",
          fontSize: "13px",
          color: "#9ca3af",
          borderTop: "1px solid #e5e7eb",
          textAlign: "center"
        }}
      >
        取消
      </div>
    </div>
  );
};