import type { BlockID, GlobalID, InstID, ModuleInfo } from "remusys-wasm-b2";
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
export function graphTypeEq(a: GraphType, b: GraphType): boolean {
    if (a.type !== b.type) return false;
    switch (a.type) {
        case "Empty":
        case "Focus":
        case "CallGraph":
            return true;
        case "Error":
            return a.message === (b as any).message && a.backtrace === (b as any).backtrace;
        case "FuncCfg":
        case "FuncDom":
            return a.func === (b as any).func;
        case "BlockDfg":
            return a.block === (b as any).block;
        case "DefUse":
            return a.center === (b as any).center;
    }
}

export type GraphState = {
    graphType: GraphType;
    moduleID?: number;
}
export type GraphAction = {
    setGraphType: (type: GraphType) => void;
    getGraphType: () => GraphType;
    getRealGraphType: (irState: IRState) => GraphType;
    initModule: (module?: ModuleInfo) => void;
}
export type GraphStore = GraphState & GraphAction;

export const useGraphState = create<GraphStore>()(devtools(immer((set, get) => ({
    graphType: { type: "Focus" },
    moduleID: undefined,

    initModule(module) {
        if (module && module.get_id() !== get().moduleID) {
            set({
                moduleID: module.get_id(),
                graphType: { type: "Focus" },
            });
        }
    },
    getGraphType: () => get().graphType,
    setGraphType(type) {
        // Check equality to avoid unnecessary updates
        if (!graphTypeEq(get().graphType, type)) {
            set({ graphType: type });
        }
    },
    getRealGraphType(irState) {
        const graphType = get().graphType;
        if (get().moduleID !== irState.getModule().get_id()) {
            get().initModule(irState.getModule());
            return getFocusGraphType(irState);
        }
        if (graphType.type !== "Focus")
            return graphType;
        else
            return getFocusGraphType(irState);
    },
}))));

function getFocusGraphType(irState: IRState): GraphType {
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
}