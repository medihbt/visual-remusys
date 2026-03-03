import type { ModuleCache } from "../ir/ir-state";
import * as ir from "../ir/ir";
import { IDCast } from "../ir/ir";

/**
 * 树节点标识符。
 *
 * 约定：
 * - 根节点：`module:<moduleId>`
 * - 其他节点：直接使用 IR 原生池 ID（如 `g:*` / `b:*` / `i:*`）
 */
export type TreeNodeId = string;

/**
 * Guide 树中的节点语义类型。
 *
 * 该类型仅表达“数据语义”，不绑定任何 UI 组件或图形库概念。
 */
export type TreeNodeKind =
  | "module"
  | "global-var"
  | "extern-global-var"
  | "func"
  | "extern-func"
  | "block"
  | "inst"
  | "phi"
  | "terminator";

/**
 * 节点引用到 IR 实体的轻量句柄。
 *
 * 作用：在树结构层保存“节点来自哪个 IR 对象”的最小信息，
 * 避免把完整 IR 对象直接耦合到树状态中。
 */
export type TreeNodeRef =
  | { type: "module"; moduleId: string }
  | { type: "global"; id: ir.GlobalID }
  | { type: "block"; id: ir.BlockID }
  | { type: "inst"; id: ir.InstID }
  ;

/**
 * 单个树节点的归一化数据。
 *
 * 不变量：
 * - `id` 在 `nodesById` 中唯一
 * - 若 `parentId !== null`，则父节点必须存在并包含当前 `id`
 * - `childIds` 仅保存直接子节点 ID，不保存 UI 展开状态
 */
export type GuideTreeNode = {
  id: TreeNodeId;
  kind: TreeNodeKind;
  ref: TreeNodeRef;
  parentId: TreeNodeId | null;
  childIds: TreeNodeId[];
  label: string;
  sourceLoc: ir.SourceLoc | null;
};

export type TreeNodeRecord = Record<TreeNodeId, GuideTreeNode>;

/**
 * Guide 树状态容器。
 *
 * 说明：
 * - `nodesById`：已物化节点集合（折叠策略会删除后代节点）
 * - `expanded`：展开节点集合（仅记录 true）
 * - `focusedId`：当前聚焦节点 ID，可为空
 */
export type GuideTreeState = {
  moduleId: string;
  rootId: TreeNodeId;
  nodesById: TreeNodeRecord;
  expanded: Record<TreeNodeId, true>;
  focusedId: TreeNodeId | null;
};

/** 初始化树状态时的可选参数。 */
export type BuildTreeOptions = {
  rootLabel?: string;
  focusRoot?: boolean;
};

/**
 * 面向渲染层的可见节点投影。
 *
 * 该类型故意不依赖 ReactFlow 等 UI 库，便于后续替换渲染器。
 */
export type VisibleTreeNode = {
  id: TreeNodeId;
  depth: number;
  parentId: TreeNodeId | null;
  kind: TreeNodeKind;
  label: string;
  hasChildren: boolean;
  expanded: boolean;
  focused: boolean;
};

/** 面向渲染层的可见边投影（父子层级边）。 */
export type VisibleTreeEdge = {
  id: string;
  parentId: TreeNodeId;
  childId: TreeNodeId;
};

/** 树状态校验问题代码。 */
export type TreeValidationIssueCode =
  | "root-missing"
  | "root-parent-not-null"
  | "root-module-id-mismatch"
  | "node-key-mismatch"
  | "parent-missing"
  | "parent-child-mismatch"
  | "child-missing"
  | "child-parent-mismatch"
  | "duplicate-child"
  | "expanded-node-missing"
  | "expanded-node-no-children"
  | "expanded-node-unreachable"
  | "focused-node-missing"
  | "cycle-detected"
  | "kind-ref-mismatch";

/** 单条树状态校验问题。 */
export type TreeValidationIssue = {
  code: TreeValidationIssueCode;
  nodeId?: TreeNodeId;
  relatedId?: TreeNodeId;
  message: string;
};

/** 树状态校验结果。 */
export type TreeValidationResult = {
  ok: boolean;
  issues: TreeValidationIssue[];
};

function cloneNodesById(nodesById: TreeNodeRecord): TreeNodeRecord {
  return Object.assign({}, nodesById);
}
function cloneExpanded(expanded: Record<TreeNodeId, true>): Record<TreeNodeId, true> {
  return Object.assign({}, expanded);
}

/** 生成模块根节点 ID。 */
function moduleNodeId(moduleId: string): TreeNodeId {
  return `module:${moduleId}`;
}

/**
 * 判断树节点 ID 在当前 IR 缓存中是否仍然有效。
 *
 * 用于重建/恢复阶段过滤过期 ID（例如重命名或源码更新后被移除的节点）。
 */
function isSourceNodePresent(cache: ModuleCache, nodeId: TreeNodeId): boolean {
  if (nodeId === moduleNodeId(cache.moduleId)) {
    return true;
  }
  if (IDCast.asGlobal(nodeId)) {
    return cache.brief.globals.some((g) => g.id === nodeId);
  }
  if (IDCast.asBlock(nodeId)) {
    return ir.irFuncScopeOfId(cache.moduleId, { Block: nodeId }) !== undefined;
  }
  if (IDCast.asInst(nodeId)) {
    return ir.irFuncScopeOfId(cache.moduleId, { Inst: nodeId }) !== undefined;
  }
  return false;
}

/** 根据 ModuleCache 生成根节点（module）。 */
function buildModuleNode(cache: ModuleCache, labelOverride?: string): GuideTreeNode {
  const rootId = moduleNodeId(cache.moduleId);
  return {
    id: rootId,
    kind: "module",
    ref: { type: "module", moduleId: cache.moduleId },
    parentId: null,
    childIds: cache.brief.globals.map((g) => g.id),
    label: labelOverride ?? "module",
    sourceLoc: null,
  };
}

/** 根据全局对象 ID 构建树节点（函数/全局变量）。 */
function buildGlobalNode(cache: ModuleCache, id: ir.GlobalID): GuideTreeNode {
  const obj = cache.loadGlobal(id);
  if (obj.typeid === "Func") {
    return {
      id,
      kind: obj.blocks ? "func" : "extern-func",
      ref: { type: "global", id },
      parentId: moduleNodeId(cache.moduleId),
      childIds: obj.blocks ? obj.blocks.map((bb) => bb.id) : [],
      label: obj.name,
      sourceLoc: obj.overview_loc,
    };
  }
  return {
    id,
    kind: obj.init === "None" ? "extern-global-var" : "global-var",
    ref: { type: "global", id },
    parentId: moduleNodeId(cache.moduleId),
    childIds: [],
    label: obj.name,
    sourceLoc: obj.overview_loc,
  };
}

/** 根据基本块 ID 构建树节点。 */
function buildBlockNode(cache: ModuleCache, id: ir.BlockID): GuideTreeNode | null {
  let bb: ir.BlockDt;
  try {
    bb = cache.loadBlock(id);
  } catch {
    return null;
  }
  return {
    id,
    kind: "block",
    ref: { type: "block", id },
    parentId: bb.parent,
    childIds: bb.insts.map((inst) => inst.id),
    label: `%${bb.name ?? bb.id}`,
    sourceLoc: bb.source_loc,
  };
}

/** 根据指令 ID 构建树节点。 */
function buildInstNode(cache: ModuleCache, id: ir.InstID): GuideTreeNode | null {
  let inst: ir.InstDt;
  try {
    inst = cache.loadInst(id);
  } catch {
    return null;
  }
  const kind: TreeNodeKind =
    inst.typeid === "Terminator"
      ? "terminator"
      : inst.typeid === "Phi"
        ? "phi"
        : "inst";
  return {
    id,
    kind,
    ref: { type: "inst", id },
    parentId: inst.parent,
    childIds: [],
    label: inst.name ?? `${inst.id} (${inst.opcode})`,
    sourceLoc: inst.source_loc,
  };
}

/**
 * ID 分派入口：根据节点 ID 类型将其物化为对应树节点。
 *
 * 分派规则：module -> global -> block -> inst。
 */
function materializeNode(cache: ModuleCache, nodeId: TreeNodeId): GuideTreeNode | null {
  if (nodeId === moduleNodeId(cache.moduleId)) {
    return buildModuleNode(cache);
  } else if (IDCast.asGlobal(nodeId)) {
    return buildGlobalNode(cache, nodeId);
  } else if (IDCast.asBlock(nodeId)) {
    return buildBlockNode(cache, nodeId);
  } else if (IDCast.asInst(nodeId)) {
    return buildInstNode(cache, nodeId);
  }
  return null;
}

/**
 * 收集当前状态中“已物化”的后代节点。
 *
 * 注意：只遍历 `nodesById` 里真实存在的节点，不会凭 `childIds` 自动物化。
 */
function collectMaterializedDescendants(
  state: GuideTreeState,
  nodeId: TreeNodeId,
): TreeNodeId[] {
  const root = state.nodesById[nodeId];
  if (!root) {
    return [];
  }
  const out: TreeNodeId[] = [];
  const stack = [...root.childIds];
  while (stack.length > 0) {
    const current = stack.pop() as TreeNodeId;
    const currentNode = state.nodesById[current];
    if (!currentNode) {
      continue;
    }
    out.push(current);
    for (const childId of currentNode.childIds) {
      stack.push(childId);
    }
  }
  return out;
}

/** 计算节点在 IR 源关系中的深度（用于重建时稳定排序）。 */
function getSourceDepth(cache: ModuleCache, nodeId: TreeNodeId): number {
  let depth = 0;
  let current: TreeNodeId | null = nodeId;
  while (current) {
    const parent = getParentIdFromSource(cache, current);
    if (!parent) {
      break;
    }
    depth += 1;
    current = parent;
  }
  return depth;
}

/**
 * 创建初始树状态。
 *
 * 初始状态仅物化根节点；子节点在展开时按需物化。
 */
export function buildGuideTreeState(
  cache: ModuleCache,
  options?: BuildTreeOptions,
): GuideTreeState {
  const root = buildModuleNode(cache, options?.rootLabel);
  const nodesById: TreeNodeRecord = Object.create(null) as Record<
    TreeNodeId,
    GuideTreeNode
  >;
  nodesById[root.id] = root;
  return {
    moduleId: cache.moduleId,
    rootId: root.id,
    nodesById,
    expanded: Object.create(null) as Record<TreeNodeId, true>,
    focusedId: options?.focusRoot === false ? null : root.id,
  };
}

/** 读取指定节点，不存在时返回 `null`。 */
export function getNode(state: GuideTreeState, nodeId: TreeNodeId): GuideTreeNode | null {
  return state.nodesById[nodeId] ?? null;
}

/** 读取已物化的直接子节点列表（未物化子节点会被忽略）。 */
export function getChildren(state: GuideTreeState, nodeId: TreeNodeId): GuideTreeNode[] {
  const node = state.nodesById[nodeId];
  if (!node) {
    return [];
  }
  const out: GuideTreeNode[] = [];
  for (const childId of node.childIds) {
    const child = state.nodesById[childId];
    if (child) {
      out.push(child);
    }
  }
  return out;
}

/** 获取祖先链（从父节点开始，向上直到根）。 */
export function getAncestors(state: GuideTreeState, nodeId: TreeNodeId): GuideTreeNode[] {
  const result: GuideTreeNode[] = [];
  let current = state.nodesById[nodeId];
  while (current && current.parentId) {
    const parent = state.nodesById[current.parentId];
    if (!parent) {
      break;
    }
    result.push(parent);
    current = parent;
  }
  return result;
}

/** 判断节点是否处于展开状态。 */
export function isExpanded(state: GuideTreeState, nodeId: TreeNodeId): boolean {
  return Boolean(state.expanded[nodeId]);
}

/** 设置焦点到指定节点；若节点不存在则保持原状态。 */
export function focusNode(state: GuideTreeState, nodeId: TreeNodeId): GuideTreeState {
  if (!state.nodesById[nodeId]) {
    return state;
  }
  if (state.focusedId === nodeId) {
    return state;
  }
  return {
    ...state,
    focusedId: nodeId,
  };
}

/** 清除当前焦点。 */
export function clearFocus(state: GuideTreeState): GuideTreeState {
  if (state.focusedId === null) {
    return state;
  }
  return {
    ...state,
    focusedId: null,
  };
}

/**
 * 展开节点并按需物化其直接子节点。
 *
 * 不会递归展开后代；如需递归请使用 `expandAllDescendants`。
 */
export function expandNode(
  state: GuideTreeState,
  cache: ModuleCache,
  nodeId: TreeNodeId,
): GuideTreeState {
  const node = state.nodesById[nodeId];
  if (!node) {
    return state;
  }
  const nextNodes = cloneNodesById(state.nodesById);
  let changed = false;
  for (const childId of node.childIds) {
    if (nextNodes[childId]) {
      continue;
    }
    const child = materializeNode(cache, childId);
    if (!child) {
      continue;
    }
    nextNodes[child.id] = child;
    changed = true;
  }
  const alreadyExpanded = Boolean(state.expanded[nodeId]);
  if (!changed && alreadyExpanded) {
    return state;
  }
  const nextExpanded = cloneExpanded(state.expanded);
  nextExpanded[nodeId] = true;
  return {
    ...state,
    nodesById: nextNodes,
    expanded: nextExpanded,
  };
}

/**
 * 递归展开节点及其全部后代（深度优先可达范围）。
 *
 * 该操作会触发大量物化，适合“展开全部”命令。
 */
export function expandAllDescendants(
  state: GuideTreeState,
  cache: ModuleCache,
  nodeId: TreeNodeId,
): GuideTreeState {
  let next = expandNode(state, cache, nodeId);
  const queue: TreeNodeId[] = [nodeId];
  while (queue.length > 0) {
    const current = queue.shift() as TreeNodeId;
    const currentNode = next.nodesById[current];
    if (!currentNode) {
      continue;
    }
    for (const childId of currentNode.childIds) {
      if (!next.nodesById[childId]) {
        next = expandNode(next, cache, current);
      }
      if (next.nodesById[childId] && next.nodesById[childId].childIds.length > 0) {
        next = expandNode(next, cache, childId);
        queue.push(childId);
      }
    }
  }
  return next;
}

/** 切换节点展开状态：已展开则折叠，否则展开。 */
export function toggleNodeExpanded(
  state: GuideTreeState,
  cache: ModuleCache,
  nodeId: TreeNodeId,
): GuideTreeState {
  if (isExpanded(state, nodeId)) {
    return collapseNode(state, nodeId);
  }
  return expandNode(state, cache, nodeId);
}

/**
 * 折叠节点。
 *
 * 当前策略 `drop-descendants`：删除已物化后代节点，并清理其展开标记。
 */
export function collapseNode(
  state: GuideTreeState,
  nodeId: TreeNodeId,
): GuideTreeState {
  if (!state.nodesById[nodeId]) {
    return state;
  }

  const descendants = collectMaterializedDescendants(state, nodeId);
  if (!state.expanded[nodeId] && descendants.length === 0) {
    return state;
  }

  const nextNodes = cloneNodesById(state.nodesById);
  const nextExpanded = cloneExpanded(state.expanded);
  delete nextExpanded[nodeId];
  for (const childId of descendants) {
    delete nextNodes[childId];
    delete nextExpanded[childId];
  }

  const nextFocused =
    state.focusedId && !nextNodes[state.focusedId] ? nodeId : state.focusedId;

  return {
    ...state,
    nodesById: nextNodes,
    expanded: nextExpanded,
    focusedId: nextFocused,
  };
}

/** 按 IR 实体 ID 查询节点（仅在节点已物化时命中）。 */
export function findByIrId(
  state: GuideTreeState,
  irId: ir.GlobalID | ir.BlockID | ir.InstID,
): GuideTreeNode | null {
  return state.nodesById[irId] ?? null;
}

/** 获取从根到目标节点的路径（节点 ID 列表）。 */
export function getNodePathToRoot(
  state: GuideTreeState,
  nodeId: TreeNodeId,
): TreeNodeId[] {
  const path: TreeNodeId[] = [];
  let current = state.nodesById[nodeId];
  while (current) {
    path.push(current.id);
    if (!current.parentId) {
      break;
    }
    current = state.nodesById[current.parentId];
  }
  return path.reverse();
}

/**
 * 将当前树状态投影为“可见节点 + 可见边”。
 *
 * 该结果是渲染无关的，可由任意图形层/列表层消费。
 */
export function projectVisibleTree(
  state: GuideTreeState,
): { nodes: VisibleTreeNode[]; edges: VisibleTreeEdge[] } {
  const nodes: VisibleTreeNode[] = [];
  const edges: VisibleTreeEdge[] = [];

  const walk = (nodeId: TreeNodeId, depth: number): void => {
    const node = state.nodesById[nodeId];
    if (!node) {
      return;
    }
    const currentExpanded = Boolean(state.expanded[nodeId]);
    nodes.push({
      id: node.id,
      depth,
      parentId: node.parentId,
      kind: node.kind,
      label: node.label,
      hasChildren: node.childIds.length > 0,
      expanded: currentExpanded,
      focused: state.focusedId === node.id,
    });

    if (!currentExpanded) {
      return;
    }

    for (const childId of node.childIds) {
      if (!state.nodesById[childId]) {
        continue;
      }
      edges.push({
        id: `${node.id}->${childId}`,
        parentId: node.id,
        childId,
      });
      walk(childId, depth + 1);
    }
  };

  walk(state.rootId, 0);
  return { nodes, edges };
}

/**
 * 在 IR 缓存更新后重建树状态，并尽量恢复旧的展开与焦点。
 *
 * 恢复策略：
 * - 仅保留在新缓存中仍存在的节点
 * - 按真实父链深度从浅到深恢复展开
 * - 尽量恢复焦点；若焦点未物化则先物化其祖先路径
 */
export function rebuildFromCache(
  previous: GuideTreeState,
  cache: ModuleCache,
  options?: BuildTreeOptions,
): GuideTreeState {
  let next = buildGuideTreeState(cache, options);
  const expandedIds = Object.keys(previous.expanded).filter((id) =>
    isSourceNodePresent(cache, id),
  );
  expandedIds.sort((a, b) => getSourceDepth(cache, a) - getSourceDepth(cache, b));
  for (const id of expandedIds) {
    next = expandNode(next, cache, id);
  }
  if (previous.focusedId && isSourceNodePresent(cache, previous.focusedId)) {
    const focused = materializeNode(cache, previous.focusedId);
    if (focused && !next.nodesById[focused.id]) {
      next = ensureNodePathMaterialized(next, cache, focused.id);
    }
    if (next.nodesById[previous.focusedId]) {
      next = focusNode(next, previous.focusedId);
    }
  }
  return next;
}

/**
 * 确保指定节点从根到父节点的路径都已物化并展开。
 *
 * 典型场景：恢复焦点、跳转到某节点前先保证其可见。
 */
export function ensureNodePathMaterialized(
  state: GuideTreeState,
  cache: ModuleCache,
  nodeId: TreeNodeId,
): GuideTreeState {
  if (!isSourceNodePresent(cache, nodeId)) {
    return state;
  }
  if (nodeId === state.rootId) {
    return state;
  }

  const chain: TreeNodeId[] = [];
  let currentId: TreeNodeId | null = nodeId;

  while (currentId) {
    chain.push(currentId);
    if (currentId === moduleNodeId(cache.moduleId)) {
      break;
    }
    if (IDCast.asGlobal(currentId)) {
      currentId = moduleNodeId(cache.moduleId);
      continue;
    }
    if (IDCast.asBlock(currentId)) {
      let block: ir.BlockDt | null = null;
      try {
        block = cache.loadBlock(currentId);
      } catch {
        block = null;
      }
      currentId = block?.parent ?? null;
      continue;
    }
    if (IDCast.asInst(currentId)) {
      let inst: ir.InstDt | null = null;
      try {
        inst = cache.loadInst(currentId);
      } catch {
        inst = null;
      }
      currentId = inst?.parent ?? null;
      continue;
    }
    currentId = null;
  }

  chain.reverse();
  let next = state;
  for (const id of chain) {
    if (id === next.rootId) {
      continue;
    }
    const parentId = getParentIdFromSource(cache, id);
    if (!parentId) {
      continue;
    }
    if (!next.nodesById[parentId]) {
      next = ensureNodePathMaterialized(next, cache, parentId);
    }
    next = expandNode(next, cache, parentId);
  }
  return next;
}

/**
 * 基于源关系解析节点的父节点 ID。
 *
 * - Global -> module
 * - Block  -> parent(Global)
 * - Inst   -> parent(Block)
 */
function getParentIdFromSource(cache: ModuleCache, nodeId: TreeNodeId): TreeNodeId | null {
  if (nodeId === moduleNodeId(cache.moduleId)) {
    return null;
  }
  if (IDCast.asGlobal(nodeId)) {
    return moduleNodeId(cache.moduleId);
  }
  if (IDCast.asBlock(nodeId)) {
    try {
      return cache.loadBlock(nodeId).parent;
    } catch {
      return null;
    }
  }
  if (IDCast.asInst(nodeId)) {
    try {
      return cache.loadInst(nodeId).parent;
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * 校验节点 `kind` 与 `ref.type` 是否匹配。
 *
 * 该检查用于发现“节点语义与引用类型不一致”的状态污染。
 */
function validateNodeKindRef(node: GuideTreeNode): TreeValidationIssue | null {
  switch (node.kind) {
    case "module":
      if (node.ref.type !== "module") {
        return {
          code: "kind-ref-mismatch",
          nodeId: node.id,
          message: `Node ${node.id} kind=module requires ref.type=module`,
        };
      }
      return null;
    case "global-var":
    case "extern-global-var":
    case "func":
    case "extern-func":
      if (node.ref.type !== "global") {
        return {
          code: "kind-ref-mismatch",
          nodeId: node.id,
          message: `Node ${node.id} kind=${node.kind} requires ref.type=global`,
        };
      }
      return null;
    case "block":
      if (node.ref.type !== "block") {
        return {
          code: "kind-ref-mismatch",
          nodeId: node.id,
          message: `Node ${node.id} kind=block requires ref.type=block`,
        };
      }
      return null;
    case "inst":
    case "phi":
    case "terminator":
      if (node.ref.type !== "inst") {
        return {
          code: "kind-ref-mismatch",
          nodeId: node.id,
          message: `Node ${node.id} kind=${node.kind} requires ref.type=inst`,
        };
      }
      return null;
    default:
      return null;
  }
}

/**
 * 校验树状态不变量并返回问题列表。
 *
 * 覆盖项：
 * - 根节点合法性
 * - 父子双向一致性
 * - 环检测
 * - expanded/focused 有效性
 * - kind/ref 匹配关系
 */
export function validateGuideTreeState(state: GuideTreeState): TreeValidationResult {
  const issues: TreeValidationIssue[] = [];
  const root = state.nodesById[state.rootId];

  if (!root) {
    issues.push({
      code: "root-missing",
      nodeId: state.rootId,
      message: `Root node ${state.rootId} is missing in nodesById`,
    });
  } else {
    if (root.parentId !== null) {
      issues.push({
        code: "root-parent-not-null",
        nodeId: root.id,
        message: `Root node ${root.id} must have parentId=null`,
      });
    }

    const expectedRootId = moduleNodeId(state.moduleId);
    if (root.id !== expectedRootId) {
      issues.push({
        code: "root-module-id-mismatch",
        nodeId: root.id,
        message: `Root id ${root.id} does not match moduleId ${state.moduleId}`,
      });
    }
  }

  const reachable = new Set<TreeNodeId>();
  const onStack = new Set<TreeNodeId>();

  const visit = (nodeId: TreeNodeId): void => {
    const node = state.nodesById[nodeId];
    if (!node) {
      return;
    }
    if (onStack.has(nodeId)) {
      issues.push({
        code: "cycle-detected",
        nodeId,
        message: `Cycle detected at node ${nodeId}`,
      });
      return;
    }
    if (reachable.has(nodeId)) {
      return;
    }
    reachable.add(nodeId);
    onStack.add(nodeId);
    for (const childId of node.childIds) {
      const child = state.nodesById[childId];
      if (!child) {
        issues.push({
          code: "child-missing",
          nodeId,
          relatedId: childId,
          message: `Node ${nodeId} references missing child ${childId}`,
        });
        continue;
      }
      if (child.parentId !== nodeId) {
        issues.push({
          code: "child-parent-mismatch",
          nodeId,
          relatedId: childId,
          message: `Child ${childId} parentId mismatch: expected ${nodeId}, got ${child.parentId}`,
        });
      }
      visit(childId);
    }
    onStack.delete(nodeId);
  };

  if (root) {
    visit(root.id);
  }

  for (const [key, node] of Object.entries(state.nodesById)) {
    if (key !== node.id) {
      issues.push({
        code: "node-key-mismatch",
        nodeId: key,
        relatedId: node.id,
        message: `nodesById key ${key} differs from node.id ${node.id}`,
      });
    }

    const kindIssue = validateNodeKindRef(node);
    if (kindIssue) {
      issues.push(kindIssue);
    }

    if (node.parentId !== null) {
      const parent = state.nodesById[node.parentId];
      if (!parent) {
        issues.push({
          code: "parent-missing",
          nodeId: node.id,
          relatedId: node.parentId,
          message: `Node ${node.id} references missing parent ${node.parentId}`,
        });
      } else if (!parent.childIds.includes(node.id)) {
        issues.push({
          code: "parent-child-mismatch",
          nodeId: node.id,
          relatedId: parent.id,
          message: `Parent ${parent.id} does not include child ${node.id}`,
        });
      }
    }

    const seenChildren = new Set<TreeNodeId>();
    for (const childId of node.childIds) {
      if (seenChildren.has(childId)) {
        issues.push({
          code: "duplicate-child",
          nodeId: node.id,
          relatedId: childId,
          message: `Node ${node.id} has duplicate child ${childId}`,
        });
      } else {
        seenChildren.add(childId);
      }
    }
  }

  for (const expandedId of Object.keys(state.expanded)) {
    const expandedNode = state.nodesById[expandedId];
    if (!expandedNode) {
      issues.push({
        code: "expanded-node-missing",
        nodeId: expandedId,
        message: `Expanded node ${expandedId} does not exist in nodesById`,
      });
      continue;
    }
    if (expandedNode.childIds.length === 0) {
      issues.push({
        code: "expanded-node-no-children",
        nodeId: expandedId,
        message: `Expanded node ${expandedId} has no children`,
      });
    }
    if (!reachable.has(expandedId)) {
      issues.push({
        code: "expanded-node-unreachable",
        nodeId: expandedId,
        message: `Expanded node ${expandedId} is unreachable from root ${state.rootId}`,
      });
    }
  }

  if (state.focusedId !== null && !state.nodesById[state.focusedId]) {
    issues.push({
      code: "focused-node-missing",
      nodeId: state.focusedId,
      message: `Focused node ${state.focusedId} does not exist in nodesById`,
    });
  }

  return {
    ok: issues.length === 0,
    issues,
  };
}

/**
 * 断言树状态合法。
 *
 * 若存在校验问题，抛出包含全部问题明细的 Error，
 * 适合开发期或测试期快速失败。
 */
export function assertGuideTreeState(
  state: GuideTreeState,
  context?: string,
): void {
  const result = validateGuideTreeState(state);
  if (result.ok) {
    return;
  }

  const header = context
    ? `Guide tree state assertion failed (${context})`
    : "Guide tree state assertion failed";
  const detail = result.issues
    .map((issue, idx) => {
      const prefix = `${idx + 1}. [${issue.code}]`;
      const at = issue.nodeId ? ` node=${issue.nodeId}` : "";
      const related = issue.relatedId ? ` related=${issue.relatedId}` : "";
      return `${prefix}${at}${related} ${issue.message}`;
    })
    .join("\n");

  throw new Error(`${header}\n${detail}`);
}

