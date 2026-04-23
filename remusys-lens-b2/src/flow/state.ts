import type { BlockID, GlobalID, InstID } from "remusys-wasm-b2";
import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import type { IRState } from "../ir/state";

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
    getRealGraphType: (irState: IRState) => GraphType;
}
export type GraphStore = GraphState & GraphAction;

export const useGraphState = create<GraphStore>()(devtools(immer((set, get) => ({
    graphType: { type: "Focus" },
    getGraphType: () => get().graphType,
    setGraphType(type) { set({ graphType: type }); },
    getRealGraphType(irState) {
        const graphType = get().graphType;
        if (graphType.type !== "Focus") return graphType;

        const focus = irState.focus;
        if (focus.length === 0) {
            return { type: "Empty" };
        } else if (focus.length === 1) {
            return { type: "CallGraph" };
        }

        const scope = irState.getModule().get_path_scope(focus);
        if (!scope) {
            // 模块全局的
            return { type: "CallGraph" };
        } else {
            // 函数内部的
            return { type: "FuncCfg", func: scope };
        }
    },
}))));
