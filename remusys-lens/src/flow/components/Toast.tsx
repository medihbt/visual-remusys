import { useFlowStore } from "../flow-stat";

export function FlowToast() {
  const flowStore = useFlowStore();
  const graphType = flowStore.graphType;

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
        backgroundColor: "rgba(256, 256, 256, 0.7)",
        border: "1px solid #ccc",
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
      }}
    >
      <div style={{ flex: 1 }}>
        {graphType.type === "Empty" && "No function selected"}
        {graphType.type === "CallGraph" && "Call Graph view"}
        {graphType.type === "ItemReference" &&
          `Item Reference view for ${graphType.item}`}
        {graphType.type === "FuncCfg" &&
          `Function CFG view for ${graphType.func}`}
        {graphType.type === "FuncDom" &&
          `Function Dominator Tree view for ${graphType.func}`}
        {graphType.type === "BlockDfg" &&
          `Block DFG view for ${graphType.block}`}
        {graphType.type === "DefUse" &&
          `Def-Use view centered on ${JSON.stringify(graphType.center)}`}
      </div>
      <button
        onClick={() => flowStore.restoreGraphType()}
        type="button"
        aria-label="Close"
        style={{
          position: "absolute",
          right: "10px",
          cursor: "pointer",
          fontSize: "16px",
          color: "#666",
          width: "20px",
          height: "20px",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.backgroundColor = "rgba(0,0,0,0.1)";
          e.currentTarget.style.color = "#333";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.backgroundColor = "transparent";
          e.currentTarget.style.color = "#666";
        }}
        title="Close"
      >
        ×
      </button>
    </div>
  );
}
