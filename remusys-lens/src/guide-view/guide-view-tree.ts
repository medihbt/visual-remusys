import type { ModuleCache } from "../ir/ir-state";
import * as ir from "../ir/ir";

/**
 * Guide 树中的节点语义类型。
 *
 * 该类型仅表达“数据语义”，不绑定任何 UI 组件或图形库概念。
 */
export type TreeNodeKind =
  | "Module"
  | "GlobalVar" | "ExternGlobalVar"
  | "Func" | "ExternFunc"
  | "Block"
  | "Inst" | "Phi" | "Terminator"
  ;
export function irObjectGetKind(obj: ir.IRValueObjectDt | null | undefined): TreeNodeKind {
  if (!obj) {
    return "Module";
  }
  switch (obj.typeid) {
    case "GlobalVar":
      return obj.linkage == "External" ? "ExternGlobalVar" : "GlobalVar";
    case "Func":
      return obj.blocks ? "Func" : "ExternFunc";
    default:
      return obj.typeid;
  }
}

export type TreeNodeRef =
  | { type: "Module" }
  | { type: "GlobalObj"; global_id: ir.GlobalID }
  | { type: "Block"; block_id: ir.BlockID }
  | { type: "Inst"; inst_id: ir.InstID }
  ;

/**
 * 单个树节点的归一化数据。
 *
 * 不变量：
 * - `id` 在 `nodesById` 中唯一
 * - 若 `parentId !== null`，则父节点必须存在并包含当前 `id`
 * - `childIds` 仅保存直接子节点 ID，不保存 UI 展开状态
 */
export interface GuideTreeNode {
  readonly moduleId: ir.ModuleID;
  readonly selfId: TreeNodeRef;
  readonly kind: TreeNodeKind;
  readonly parentId: TreeNodeRef | null;
  readonly childIds: TreeNodeRef[];
  readonly label: string;
  readonly sourceLoc: ir.SourceLoc | null;
}

export class TreeNodeStorage {
  readonly moduleId: ir.ModuleID;
  private globalNode: GuideTreeNode | null = null;
  private nodesById: Map<ir.PoolStrID, GuideTreeNode> = new Map();

  constructor(moduleId: ir.ModuleID) {
    this.moduleId = moduleId;
  }
  private postDfs(node: GuideTreeNode, visit: (node: GuideTreeNode) => void) {
    for (const childId of node.childIds) {
      const childNode = this.get(childId);
      if (childNode) {
        this.postDfs(childNode, visit);
      }
    }
    visit(node);
  }
  private dfsRemove(id: TreeNodeRef) {
    const node = this.get(id);
    if (!node) return;
    this.postDfs(node, n => {
      switch (n.selfId.type) {
        case "Module":
          this.globalNode = null;
          break;
        case "GlobalObj":
          this.nodesById.delete(n.selfId.global_id);
          break;
        case "Block":
          this.nodesById.delete(n.selfId.block_id);
          break;
        case "Inst":
          this.nodesById.delete(n.selfId.inst_id);
          break;
      }
    });
  }

  shareClone(): TreeNodeStorage {
    const newMap = new TreeNodeStorage(this.moduleId);
    for (const [id, node] of this.nodesById.entries()) {
      newMap.nodesById.set(id, node);
    }
    newMap.globalNode = this.globalNode;
    return newMap;
  }
  /** insert a node and return the previous node if any */
  set(node: GuideTreeNode): GuideTreeNode | null {
    return this.setOrReplace(node, false);
  }
  replace(node: GuideTreeNode): GuideTreeNode | null {
    return this.setOrReplace(node, true);
  }
  setOrReplace(node: GuideTreeNode, force: boolean): GuideTreeNode | null {
    if (node.moduleId !== this.moduleId) {
      throw new Error(`Node module ID ${node.moduleId} does not match TreeNodeStorage module ID ${this.moduleId}`);
    }
    let id: ir.PoolStrID;
    let selfId = node.selfId;
    switch (selfId.type) {
      case "Module":
        if (!this.globalNode || force) {
          const oldNode = this.globalNode;
          this.globalNode = node;
          return oldNode || null;
        } else {
          return this.globalNode;
        }
      case "GlobalObj":
        id = selfId.global_id;
        break;
      case "Block":
        id = selfId.block_id;
        break;
      case "Inst":
        id = selfId.inst_id;
        break;
    }

    const oldNode = this.nodesById.get(id) || null;
    if (oldNode && !force) {
      return oldNode;
    }
    this.nodesById.set(id, node);
    return oldNode;
  }
  expand(id: TreeNodeRef, module: ModuleCache): GuideTreeNode {
    if (module.moduleId !== this.moduleId) {
      throw new Error(`Module ID mismatch: expected ${this.moduleId}, got ${module.moduleId}`);
    }
    const node = this.get(id);
    if (node) return node;
    let newNode: GuideTreeNode;
    switch (id.type) {
      case "Module": {
        const moduleBrief = module.brief;
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: "Module",
          parentId: null,
          childIds: moduleBrief.globals.map(gid => ({ type: "GlobalObj", global_id: gid.id })),
          label: this.moduleId,
          sourceLoc: null,
        };
        break;
      }
      case "GlobalObj": {
        const gobj = module.loadGlobal(id.global_id);
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: irObjectGetKind(gobj),
          parentId: { type: "Module" },
          childIds: gobj.typeid === "Func" && gobj.blocks ? gobj.blocks.map(bid => ({ type: "Block", block_id: bid.id })) : [],
          label: gobj.name || `${gobj.typeid} ${id.global_id}`,
          sourceLoc: gobj.overview_loc,
        };
        break;
      }
      case "Block": {
        const block = module.loadBlock(id.block_id);
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: "Block",
          parentId: { type: "GlobalObj", global_id: block.parent },
          childIds: block.insts.map(iid => ({ type: "Inst", inst_id: iid.id })),
          label: block.name || `Block ${id.block_id}`,
          sourceLoc: block.source_loc,
        };
        break;
      }
      case "Inst": {
        const inst = module.loadInst(id.inst_id);
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: inst.typeid,
          parentId: { type: "Block", block_id: inst.parent },
          childIds: [], // instructions don't have children in the tree structure
          label: `${inst.opcode} ${id.inst_id}`,
          sourceLoc: inst.source_loc,
        };
        break;
      }
    }
    this.set(newNode);
    return newNode;
  }
  expandChildren(id: TreeNodeRef, module: ModuleCache): GuideTreeNode[] {
    const node = this.get(id);
    if (!node) {
      throw new Error(`Node with ID ${JSON.stringify(id)} not found in TreeNodeStorage`);
    }
    return node.childIds.map(childId => this.expand(childId, module));
  }
  dfsExpand(id: TreeNodeRef, module: ModuleCache): GuideTreeNode[] {
    const rootNode = this.expand(id, module);
    const result: GuideTreeNode[] = [];
    this.postDfs(rootNode, n => result.push(n));
    return result;
  }
  collapse(id: TreeNodeRef): void {
    const node = this.get(id);
    if (!node) return;
    switch (id.type) {
      case "Module":
        throw new Error("Cannot collapse module node");
      default:
        this.dfsRemove(id);
        break;
    }
  }

  constReplace(node: GuideTreeNode): TreeNodeStorage {
    let newMap = this.shareClone();
    newMap.replace(node);
    return newMap;
  }
  constExpand(id: TreeNodeRef, module: ModuleCache): [GuideTreeNode, TreeNodeStorage] {
    const node = this.get(id);
    if (node)
      return [node, this];
    const newMap = this.shareClone();
    return [newMap.expand(id, module), newMap];
  }

  get(id: TreeNodeRef): GuideTreeNode | null {
    switch (id.type) {
      case "Module":
        return this.globalNode;
      case "GlobalObj":
        return this.nodesById.get(id.global_id) || null;
      case "Block":
        return this.nodesById.get(id.block_id) || null;
      case "Inst":
        return this.nodesById.get(id.inst_id) || null;
    }
  }
}
