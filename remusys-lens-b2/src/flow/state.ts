import type { BlockID, GlobalID, InstID } from "remusys-wasm-b2";
import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

export type GraphType =
    | { type: "Empty" }
    | { type: "Error", message: string, backtrace?: string }
    | { type: "Focus" }
    | { type: "CallGraph" }
    | { type: "FuncCfg", func: GlobalID }
    | { type: "FuncDom", func: GlobalID }
    | { type: "BlockDfg", block: BlockID }
    | { type: "DefUse", center: InstID }
    ;

export type GraphState = {
    graphType: GraphType;
}
export type GraphAction = {
    setGraphType: (type: GraphType) => void;
    getGraphType: () => GraphType;
}
export type GraphStore = GraphState & GraphAction;

export const useGraphState = create<GraphStore>()(devtools(immer((set, get) => ({
    graphType: { type: "Focus" },
    getGraphType: () => get().graphType,
    setGraphType(type) { set({ graphType: type }); }
}))));
