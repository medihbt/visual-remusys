/**
 * # FlowViewer Toolbar -- 流图工具栏
 * 
 * 位于流图底部的一个简单小白条, 只有当前图类型不是焦点图时才会显示, 显示当前图的类型, 以及一个关闭按钮可以切换回焦点图.
 */

import type { ModuleInfo } from "remusys-wasm";
import { useGraphState, type GraphType } from "./state";
import { useIRStore } from "../ir/state";

function toolbarTitle(module: ModuleInfo, graphType: GraphType): string {
  switch (graphType.type) {
    case "Empty": return "No function selected";
    case "Error": return `Error: ${graphType.message}`;
    case "CallGraph": return "Call Graph view";
    case "FuncCfg": {
      const funcName = module.get_object_display_name({
        type: "Global", value: graphType.func,
      })
      return `Function CFG view for ${funcName}`
    };
    case "FuncDom": {
      const funcName = module.get_object_display_name({
        type: "Global", value: graphType.func,
      })
      return `Function Dominator Tree view for ${funcName}`
    }
    case "BlockDfg": {
      const blockName = module.get_object_display_name({
        type: "Block", value: graphType.block,
      })
      return `Block DFG view for ${blockName}`
    }
    case "DefUse": {
      const instName = module.get_object_display_name({
        type: "Inst", value: graphType.center,
      })
      return `Def-Use view centered on ${instName}`
    }
    default: return "Flow view";
  }
}

export default function FlowToolbar() {
  const graphType = useGraphState((state) => state.graphType);
  const setGraphType = useGraphState((state) => state.setGraphType);

  const module = useIRStore((state) => state.module);
  if (!module) {
    return <></>;
  }

  if (graphType.type === "Focus") {
    return <></>;
  }

  return (
    <div
      style={{
        position: "absolute",
        bottom: "20px",
        left: "50%",
        transform: "translateX(-50%)",
        backgroundColor: "rgba(255, 255, 255, 0.72)",
        border: "1px solid #d1d5db",
        padding: "10px 20px",
        borderRadius: "10px",
        zIndex: 1000,
        width: "70%",
        maxWidth: 800,
        minHeight: 20,
        textAlign: "center",
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        fontSize: "12px",
        backdropFilter: "blur(2px)",
      }}
    >
      <div style={{ flex: 1 }}>{toolbarTitle(module, graphType)}</div>

      <button
        type="button"
        aria-label="Back to focus graph"
        title="Back to focus graph"
        onClick={() => setGraphType({ type: "Focus" })}
        style={{
          position: "absolute",
          right: "10px",
          cursor: "pointer",
          fontSize: "16px",
          color: "#666",
          width: "20px",
          height: "20px",
          border: "none",
          background: "transparent",
          borderRadius: "4px",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.backgroundColor = "rgba(0,0,0,0.08)";
          e.currentTarget.style.color = "#333";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.backgroundColor = "transparent";
          e.currentTarget.style.color = "#666";
        }}
      >
        ×
      </button>
    </div>
  );
}