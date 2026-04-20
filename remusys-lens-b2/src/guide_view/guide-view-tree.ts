/**
 * # Guide View 树加载与最小状态
 *
 * 一个 IR Module 下的树结构非常庞大. 倘若全部展示, 且不说性能问题, 那么多结点会直接
 * 淹没用户. 因此 Remusys-Lens 的树导航视图不会一次性全部展示所有结点. 在启动时,
 * 视图只展示一个表示 Module 的根节点. 每个树导航视图控件都有一个子结点列表, 用户可以
 * 点击列表中表示子结点的子项展开或者收起这个子结点.
 * 
 * 那...这些结点的数据从哪里来?
 * 
 * 在旧版本 b1 分支中, 前端不仅有完整的 IR 树缓存, 还缓存了整个中间代码系统的各种详细
 * 信息. wasm 服务层会把 IR 对象的具体类型等信息做一些概括后按需发送到前端缓存, JS 侧的
 * `TreeNodeStorage` 再从这些具体的 IR 对象类型中获取内容, 按不同对象类型组织成树结构,
 * 点击展开时还可以根据关联的子结点 IR 对象类型分派不同的结点构造逻辑和子结点加载逻辑.
 * 这样当然很好用, 但它依赖于“前端持有一份可长期信任的实体缓存”这个前提.
 *
 * b2 要求放弃这个前提. 这一版不允许前端缓存 IR 实体树, 所有对象数据都直接从 WASM 服务层
 * 获取. Guide View 也一样: 它唯一可信的数据来源是 IR Tree, 但只截取
 * Module-Global/Func-Block-Inst-(Inst 的直接 Use) 这几层有一一映射的主干树. 前端负责的
 * 不是“保存一棵树”, 而只是保存最小交互状态, 例如哪些结点当前处于展开态.
 *
 * 这里要严格区分两件事:
 *
 * 1. 同一 Module 生命周期内的 IR 树变化
 *    例如展开、收起、重命名、局部插入、局部删除、局部替换. 这些变化发生时, 前端不会继承
 *    b1 那种实体缓存, 而是基于新的 IR Tree 重新构造当前可见主干树. 但用户的展开状态不能
 *    平白丢失, 因此前端仍需要维护少量 UI 状态, 并在树刷新时做 reconciliation:
 *    尽量保留仍然合法的展开状态; 当前焦点若失效, 则向上回退到最近仍存在的祖先.
 *
 * 2. 重新编译 / 重新加载 Module
 *    这不属于“IR 树变化”, 而是整个数据宇宙被替换掉. 旧 Module 被丢弃后, 旧的树路径、
 *    旧的对象 ID、旧的展开状态和旧的焦点都不再具有语义合法性. 因此这里不做连续性保留,
 *    而是直接废弃全部前端状态, 从新的 Module 根重新初始化 Guide View.
 *
 * 因而本文件要解决的问题不是“如何在前端缓存一棵完整 IR 树”, 而是:
 *
 * 1. 如何只根据当前展开集按需从 wasm 拉取可见树切片;
 * 2. 如何把这些切片组织成 React Flow 需要的主干树;
 * 3. 如何在同一 Module 内的树刷新后尽量保持展示不突兀.
 */

import { useIRStore } from "../ir/state";
import type {
    GuideNodeBase,
    GuideNodeData,
    GuideNodeExpand,
    IRObjPath,
    IRTreeObjID
} from "remusys-wasm-b2";
import { IRExpandTree, IRTreeCursor } from "remusys-wasm-b2";

import { create } from "zustand";
import { devtools } from "zustand/middleware";

const MODULE_ID: IRTreeObjID = { type: "Module" };
const MODULE_PATH: IRObjPath = [MODULE_ID];

/**
 * 一次 GuideView 树加载/重建的结果。
 */
export type GuideTreeBuildResult = {
    /** 可直接交给 React Flow 拍平的展开树根节点。 */
    root: GuideNodeExpand;
    /**
     * 焦点路径在当前树上的对齐结果。
     *
     * 若原焦点不可达，会回退到最近可达祖先，最差回退到 Module 根。
     */
    nextFocusPath: IRObjPath;
    /**
     * 可选：本次动作目标结点的已解析路径。
     */
    resolvedPath?: IRObjPath;
};

/**
 * GuideView 本地状态（最小 UI 状态）。
 *
 * 注意：焦点是全局状态，由 `IRStore.focus` 统一维护，这里不再保存本地副本。
 */
export type GuideViewTreeState = {
    /** 当前 `IRExpandTree` 所属 module 的临时唯一 ID。 */
    moduleId?: number;
    /** wasm 侧展开状态树实例（权威展开状态存储）。 */
    expandTree?: IRExpandTree;
    /** 每次重建树后自增的版本号，可用于防陈旧异步结果。 */
    treeEpoch: number;
    /** 最近一次构建得到的树根。 */
    root?: GuideNodeExpand;
};

/**
 * GuideView 树状态动作集合。
 */
export type GuideViewTreeActions = {
    /** 同一 Module 生命周期内刷新可见树并做状态对齐。 */
    refreshSameModule: () => GuideTreeBuildResult;
    /** 重新编译/重载 Module 后的硬重置入口。 */
    resetForNewModule: () => GuideTreeBuildResult;
    /** 请求设置焦点路径，并基于新焦点重建树。 */
    requestFocusPath: (path: IRObjPath) => GuideTreeBuildResult;
    /** 展开指定节点。 */
    expand(node: GuideNodeData): GuideTreeBuildResult;
    /** 收起指定节点及其可达子树。 */
    collapse(node: GuideNodeData): GuideTreeBuildResult;
    /** 通过节点请求焦点路径（路径由当前树重建）。 */
    requestFocus(node: GuideNodeData): GuideTreeBuildResult;
};

/**
 * GuideView 树状态仓库类型。
 */
export type GuideViewTreeStore = GuideViewTreeState & GuideViewTreeActions;

function clonePath(path: IRObjPath): IRObjPath {
    return path.map((x) => ({ ...x })) as IRObjPath;
}

function getGlobalFocusPath(): IRObjPath {
    return clonePath(useIRStore.getState().focus);
}

function reconcileFocusPath(focusPath: IRObjPath): IRObjPath {
    const module = useIRStore.getState().getModule();
    const cursor = new IRTreeCursor(module);
    let currentPath: IRObjPath = [];
    try {
        for (const obj of focusPath) {
            if (obj.type === "Module") {
                continue;
            }
            if (!cursor.has_child(module, obj)) {
                break;
            }
            cursor.goto_child(module, obj);
        }
        currentPath = cursor.emit_path(module);
    } finally {
        cursor.free();
    }
    return currentPath;
}

function ensureNonEmptyPath(path: IRObjPath): IRObjPath {
    return path.length > 0 ? path : clonePath(MODULE_PATH);
}

/** wasm 侧构建好根节点以后, 受限于 Rust 的所有权机制, 不能连接父结点. 这里把父结点连接补上. */
function connectGuideTree(node: GuideNodeExpand) {
    for (const child of node.children) {
        child.parent = node;
        if (child.children) {
            connectGuideTree(child);
        }
    }
}
function buildGuideTreeFromWasm(root: GuideNodeData, focusPath: IRObjPath): GuideTreeBuildResult {
    if (!root.children) {
        throw new Error("root node must have children");
    }
    const nextFocusPath = reconcileFocusPath(focusPath);
    connectGuideTree(root);
    return { root, nextFocusPath };
}

function irObjEq(a: IRTreeObjID, b: IRTreeObjID): boolean {
    return JSON.stringify(a) === JSON.stringify(b);
}

function pathOfNode(node: GuideNodeData): IRObjPath {
    const path: IRTreeObjID[] = [];
    let current: GuideNodeBase | undefined = node;
    while (current) {
        path.push(current.irObject);
        current = current.parent;
    }
    path.reverse();
    return path;
}

function reloadTreeWithWasm(expandTree: IRExpandTree, requestedFocusPath: IRObjPath): GuideTreeBuildResult {
    const irStore = useIRStore.getState();
    const module = irStore.getModule();
    const nextFocusPath = ensureNonEmptyPath(reconcileFocusPath(requestedFocusPath));
    if (nextFocusPath !== requestedFocusPath) {
        irStore.setFocus(nextFocusPath);
    }
    const root = expandTree.load_tree(module, nextFocusPath);
    const res = buildGuideTreeFromWasm(root, nextFocusPath);
    if (res.nextFocusPath !== nextFocusPath) {
        irStore.setFocus(res.nextFocusPath);
    }
    return res;
}

function ensureExpandTree(state: GuideViewTreeState): { moduleId: number; expandTree: IRExpandTree } {
    const module = useIRStore.getState().getModule();
    const moduleId = module.get_id();
    if (state.expandTree && state.moduleId === moduleId) {
        return { moduleId, expandTree: state.expandTree };
    }
    state.expandTree?.free();
    return {
        moduleId,
        expandTree: IRExpandTree.new(module),
    };
}

function applyBuildResult(result: GuideTreeBuildResult) {
    useIRStore.getState().setFocus(result.nextFocusPath);
    return {
        root: result.root,
    };
}

/**
 * GuideView 树状态仓库。
 *
 * 该仓库只管理最小 UI 状态（展开集合、可见树、路径索引、epoch）。焦点始终由全局 `IRStore`
 * 统一管理，避免跨视图同步问题。
 */
export const useGuideViewTreeStore = create<GuideViewTreeStore>()(devtools((set, get) => ({
    moduleId: undefined,
    expandTree: undefined,
    treeEpoch: 0,
    root: undefined,

    refreshSameModule() {
        const st = get();
        const { moduleId, expandTree } = ensureExpandTree(st);
        const result = reloadTreeWithWasm(expandTree, getGlobalFocusPath());
        set((prev) => ({
            ...prev,
            moduleId,
            expandTree,
            ...applyBuildResult(result),
            treeEpoch: prev.treeEpoch + 1,
        }));
        return result;
    },

    resetForNewModule() {
        const module = useIRStore.getState().getModule();
        const moduleId = module.get_id();
        const expandTree = IRExpandTree.new(module);
        const resetFocus = clonePath(MODULE_PATH);
        useIRStore.getState().setFocus(resetFocus);
        const result = reloadTreeWithWasm(expandTree, resetFocus);
        get().expandTree?.free();
        set((prev) => ({
            ...prev,
            moduleId,
            expandTree,
            ...applyBuildResult(result),
            treeEpoch: prev.treeEpoch + 1,
        }));
        return result;
    },

    expand(node) {
        const st = get();
        const { moduleId, expandTree } = ensureExpandTree(st);
        const path = pathOfNode(node);
        const module = useIRStore.getState().getModule();
        expandTree.expand_one(module, path);
        const result: GuideTreeBuildResult = {
            ...reloadTreeWithWasm(expandTree, getGlobalFocusPath()),
            resolvedPath: path,
        };
        set((prev) => ({
            ...prev,
            moduleId,
            expandTree,
            ...applyBuildResult(result),
            treeEpoch: prev.treeEpoch + 1,
        }));
        return result;
    },

    collapse(node) {
        const st = get();
        const { moduleId, expandTree } = ensureExpandTree(st);
        const path = pathOfNode(node);
        if (path.length === 1 && path[0].type === "Module") {
            return get().refreshSameModule();
        }

        const globalFocus = getGlobalFocusPath();
        const collapsePath = path;
        const focusInsideCollapsed =
            collapsePath.length <= globalFocus.length &&
            collapsePath.every((obj, idx) => irObjEq(obj, globalFocus[idx]));
        if (focusInsideCollapsed) {
            useIRStore.getState().setFocus(clonePath(collapsePath));
        }

        const module = useIRStore.getState().getModule();
        expandTree.collapse(module, collapsePath);
        const result: GuideTreeBuildResult = {
            ...reloadTreeWithWasm(expandTree, getGlobalFocusPath()),
            resolvedPath: collapsePath,
        };
        set((prev) => ({
            ...prev,
            moduleId,
            expandTree,
            ...applyBuildResult(result),
            treeEpoch: prev.treeEpoch + 1,
        }));
        return result;
    },

    requestFocus(node) {
        return get().requestFocusPath(pathOfNode(node));
    },

    requestFocusPath(requestedPath) {
        const st = get();
        const { moduleId, expandTree } = ensureExpandTree(st);
        const nextFocusPath = ensureNonEmptyPath(reconcileFocusPath(requestedPath));
        useIRStore.getState().setFocus(nextFocusPath);
        const result: GuideTreeBuildResult = {
            ...reloadTreeWithWasm(expandTree, nextFocusPath),
            resolvedPath: nextFocusPath,
        };
        set((prev) => ({
            ...prev,
            moduleId,
            expandTree,
            ...applyBuildResult(result),
            treeEpoch: prev.treeEpoch + 1,
        }));
        return result;
    },
}), { name: "GuideViewTreeStore" }));

/**
 * 在同一 Module 生命周期内刷新并返回可见树根。
 *
 * 行为等价于 `useGuideViewTreeStore.getState().refreshSameModule().root`。
 */
export function loadExpandedGuideTree(): GuideNodeExpand {
    return useGuideViewTreeStore.getState().refreshSameModule().root;
}
