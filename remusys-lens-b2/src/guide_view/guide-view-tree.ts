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

import type {
    GuideNodeBase,
    GuideNodeData,
    GuideNodeExpand,
    IRObjPath,
    IRTreeObjID
} from "remusys-wasm-b2";
import { IRExpandTree, IRTreeCursor } from "remusys-wasm-b2";

import { type IRState } from "../ir/state";

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
 * GuideView 私有控制器状态。
 *
 * 这是 GuideView 组件内部的可变状态容器，不是全局 store。
 */
export type GuideTreeController = {
    /** 当前 `IRExpandTree` 所属 module 的临时唯一 ID。 */
    moduleId?: number;
    /** wasm 侧展开状态树实例（权威展开状态存储）。 */
    expandTree?: IRExpandTree;
};

export function createGuideTreeController(): GuideTreeController {
    return {};
}

export function disposeGuideTreeController(controller: GuideTreeController) {
    controller.expandTree?.free();
    controller.expandTree = undefined;
    controller.moduleId = undefined;
}

function clonePath(path: IRObjPath): IRObjPath {
    return path.map((x) => ({ ...x })) as IRObjPath;
}

function getGlobalFocusPath(irStore: IRState): IRObjPath {
    return clonePath(irStore.focus);
}

function reconcileFocusPath(irStore: IRState, focusPath: IRObjPath): IRObjPath {
    const module = irStore.getModule();
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
function buildGuideTreeFromWasm(
    irStore: IRState,
    root: GuideNodeData,
    focusPath: IRObjPath
): GuideTreeBuildResult {
    if (!root.children) {
        throw new Error("root node must have children");
    }
    const nextFocusPath = reconcileFocusPath(irStore, focusPath);
    connectGuideTree(root);
    return { root, nextFocusPath };
}

function irObjEq(a: IRTreeObjID, b: IRTreeObjID): boolean {
    return JSON.stringify(a) === JSON.stringify(b);
}

export function pathOfNode(node: GuideNodeData): IRObjPath {
    const path: IRTreeObjID[] = [];
    let current: GuideNodeBase | undefined = node;
    while (current) {
        path.push(current.irObject);
        current = current.parent;
    }
    path.reverse();
    return path;
}

function reloadTreeWithWasm(
    irStore: IRState,
    expandTree: IRExpandTree,
    requestedFocusPath: IRObjPath
): GuideTreeBuildResult {
    const module = irStore.getModule();
    const nextFocusPath = ensureNonEmptyPath(reconcileFocusPath(irStore, requestedFocusPath));
    const root = expandTree.load_tree(module, nextFocusPath);
    const res = buildGuideTreeFromWasm(irStore, root, nextFocusPath);
    // irStore 自己内部会做一次路径相等性检查, 避免循环状态更新
    irStore.setFocus(res.nextFocusPath);
    return res;
}

function ensureExpandTree(irStore: IRState, state: GuideTreeController): { moduleId: number; expandTree: IRExpandTree } {
    const module = irStore.getModule();
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

/** 同一 Module 生命周期内刷新可见树并做状态对齐。 */
export function refreshSameModule(controller: GuideTreeController, irStore: IRState): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return reloadTreeWithWasm(irStore, expandTree, getGlobalFocusPath(irStore));
}

/** 重新编译/重载 Module 后的硬重置入口。 */
export function resetForNewModule(controller: GuideTreeController, irStore: IRState): GuideTreeBuildResult {
    const module = irStore.getModule();
    const moduleId = module.get_id();
    const expandTree = IRExpandTree.new(module);
    const resetFocus = clonePath(MODULE_PATH);
    const result = reloadTreeWithWasm(irStore, expandTree, resetFocus);
    controller.expandTree?.free();
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return result;
}

/** 展开指定节点。 */
export function expandNode(controller: GuideTreeController, irStore: IRState, node: GuideNodeData): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    const path = pathOfNode(node);
    const module = irStore.getModule();
    expandTree.expand_one(module, path);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return {
        ...reloadTreeWithWasm(irStore, expandTree, getGlobalFocusPath(irStore)),
        resolvedPath: path,
    };
}

/** 展开这个结点和它的一层子结点（占位）。 */
export function expandChildrenNode(controller: GuideTreeController, irStore: IRState, node: GuideNodeData): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    const path = pathOfNode(node);
    const module = irStore.getModule();
    expandTree.expand_two(module, path);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return refreshSameModule(controller, irStore);
}

/** 深度优先展开指定节点及其子树（占位）。 */
export function dfsExpandNode(controller: GuideTreeController, irStore: IRState, node: GuideNodeData): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    const path = pathOfNode(node);
    const module = irStore.getModule();
    expandTree.expand_all(module, path);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return refreshSameModule(controller, irStore);
}

/** 收起指定节点及其可达子树。 */
export function collapseNode(controller: GuideTreeController, irStore: IRState, node: GuideNodeData): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    const path = pathOfNode(node);
    if (path.length === 1 && path[0].type === "Module") {
        return refreshSameModule(controller, irStore);
    }

    const globalFocus = getGlobalFocusPath(irStore);
    const collapsePath = path;
    const focusInsideCollapsed =
        collapsePath.length <= globalFocus.length &&
        collapsePath.every((obj, idx) => irObjEq(obj, globalFocus[idx]));
    if (focusInsideCollapsed) {
        irStore.setFocus(clonePath(collapsePath));
    }

    const module = irStore.getModule();
    expandTree.collapse(module, collapsePath);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return {
        ...reloadTreeWithWasm(irStore, expandTree, getGlobalFocusPath(irStore)),
        resolvedPath: collapsePath,
    };
}

export function collapseChildrenNode(controller: GuideTreeController, irStore: IRState, node: GuideNodeData): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    const path = pathOfNode(node);

    const module = irStore.getModule();
    console.log("Collapsing children of node at path:", path);
    expandTree.collapse_children(module, path);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return refreshSameModule(controller, irStore);
}

/** 通过节点请求焦点路径（路径由当前树重建）。 */
export function requestFocusNode(controller: GuideTreeController, irStore: IRState, node: GuideNodeData): GuideTreeBuildResult {
    return requestFocusPath(controller, irStore, pathOfNode(node));
}

/** 请求设置焦点路径，并基于新焦点重建树。 */
export function requestFocusPath(controller: GuideTreeController, irStore: IRState, requestedPath: IRObjPath): GuideTreeBuildResult {
    const { moduleId, expandTree } = ensureExpandTree(irStore, controller);
    const nextFocusPath = ensureNonEmptyPath(reconcileFocusPath(irStore, requestedPath));
    irStore.setFocus(nextFocusPath);
    controller.moduleId = moduleId;
    controller.expandTree = expandTree;
    return {
        ...reloadTreeWithWasm(irStore, expandTree, nextFocusPath),
        resolvedPath: nextFocusPath,
    };
}

/**
 * 在同一 Module 生命周期内刷新并返回可见树根。
 *
 * 行为等价于 `refreshSameModule(controller, irStore).root`。
 */
export function loadExpandedGuideTree(controller: GuideTreeController, irStore: IRState): GuideNodeExpand {
    return refreshSameModule(controller, irStore).root;
}
