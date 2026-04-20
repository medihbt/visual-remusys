import { ModuleInfo } from "remusys-wasm-b2";
import type {
    IRTreeNodeDt, IRObjPath, MonacoSrcRange, SourceTy, CallGraphDt,
    FuncCfgDt, GlobalID, DomTreeDt, BlockDfg, BlockID
} from "remusys-wasm-b2";

import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

export interface IRStorage {
    module?: ModuleInfo;
    source: string;
    focus: IRObjPath;
}

export interface IRActions {
    compile: (src_kind: SourceTy, src: string) => void;
    getFocusSrcRange: () => MonacoSrcRange;
    getModule: () => ModuleInfo;
    setFocus: (path: IRObjPath) => void;
    clearFocus: () => void;

    getTreeChildren(path: IRObjPath): IRTreeNodeDt[];

    // IR module as graph generator
    getCallGraph(): CallGraphDt;
    getFuncCfg(func: GlobalID): FuncCfgDt;
    getFuncDominance(func: GlobalID): DomTreeDt;
    getBlockDfg(block: BlockID): BlockDfg;
}

export type IRState = IRStorage & IRActions;

export const useIRStore = create<IRState>()(devtools(immer((set, get) => ({
    module: undefined,
    source: "",
    focus: [{ type: "Module" }],
    compile(src_kind, src) {
        const module = ModuleInfo.compile_from(src_kind, src);
        set({ module, source: module.dump_source(), focus: [{ type: "Module" }] });
    },
    getFocusSrcRange(): MonacoSrcRange {
        const { module, focus } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        const node: IRTreeNodeDt = module.path_get_node(focus);
        return node.src_range;
    },
    getModule(): ModuleInfo {
        const { module } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        return module;
    },
    setFocus(path) { set({ focus: path }); },
    clearFocus() { set({ focus: [{ type: "Module" }] }); },
    getTreeChildren(path) {
        const { module } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        return module.ir_tree_get_children(path);
    },

    getCallGraph(): CallGraphDt {
        const { module } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        return module.get_call_graph();
    },
    getFuncCfg(funcID): FuncCfgDt {
        const { module } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        return module.get_func_cfg(funcID)
    },
    getFuncDominance(func) {
        const { module } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        return module.get_func_dom_tree(func)
    },
    getBlockDfg(block) {
        const { module } = get();
        if (!module) {
            throw new Error("module not loaded");
        }
        return module.get_block_dfg(block)
    },
}))));

export function useIRFocus(): IRObjPath { return useIRStore().focus }
export function useIRModule(): ModuleInfo { return useIRStore().getModule() }
export function useIRFocusSrcRange(): MonacoSrcRange { return useIRStore().getFocusSrcRange() }
