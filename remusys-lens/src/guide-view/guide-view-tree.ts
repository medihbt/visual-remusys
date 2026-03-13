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
export function irIdGetKind(module: ModuleCache, id: ir.SourceTrackable): TreeNodeKind {
  switch (id.type) {
    case "Module":
      return "Module";
    case "Global": {
      const gobj = module.loadGlobal(id.value);
      switch (gobj.typeid) {
        case "GlobalVar":
          return gobj.linkage == "External" ? "ExternGlobalVar" : "GlobalVar";
        case "Func":
          return gobj.blocks ? "Func" : "ExternFunc";
      }
    }
    case "Block": return "Block";
    case "Inst": {
      const inst = module.loadInst(id.value);
      return inst.typeid;
    }
    default:
      throw new Error(`Unknown TreeNodeRef type: ${(id as any).type}`);
  }
}
export function getNodeIdLabel(module: ModuleCache, id: ir.SourceTrackable): string {
  switch (id.type) {
    case "Module":
      return module.moduleId;
    case "Global": {
      const gobj = module.loadGlobal(id.value);
      return gobj.name;
    }
    case "Block": {
      const block = module.loadBlock(id.value);
      return block.name || `Block ${id.value}`;
    }
    case "Inst": {
      const inst = module.loadInst(id.value);
      return inst.name || `${inst.opcode} ${id.value}`;
    }
    default:
      throw new Error(`Unknown TreeNodeRef type: ${(id as any).type}`);
  }
}


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
  readonly selfId: ir.SourceTrackable;
  readonly kind: TreeNodeKind;
  readonly parentId: ir.SourceTrackable | null;
  readonly childIds: ir.SourceTrackable[];
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
  private dfsRemove(id: ir.SourceTrackable) {
    const node = this.get(id);
    if (!node) return;
    this.postDfs(node, n => {
      switch (n.selfId.type) {
        case "Module":
          this.globalNode = null;
          break;
        case "Global":
          this.nodesById.delete(n.selfId.value);
          break;
        case "Block":
          this.nodesById.delete(n.selfId.value);
          break;
        case "Inst":
          this.nodesById.delete(n.selfId.value);
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
      case "Global": case "Block": case "Inst":
        id = selfId.value;
        break;
      default:
        throw new Error(`Source trackable ID ${JSON.stringify(selfId)} is not compatible with GuideTreeNode`);
    }

    const oldNode = this.nodesById.get(id) || null;
    if (oldNode && !force) {
      return oldNode;
    }
    this.nodesById.set(id, node);
    return oldNode;
  }
  expand(id: ir.SourceTrackable, module: ModuleCache): GuideTreeNode {
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
          childIds: moduleBrief.globals.map(gid => ({ type: "Global", value: gid.id })),
          label: this.moduleId,
          sourceLoc: null,
        };
        break;
      }
      case "Global": {
        const gobj = module.loadGlobal(id.value);
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: irObjectGetKind(gobj),
          parentId: { type: "Module" },
          childIds: gobj.typeid === "Func" && gobj.blocks ? gobj.blocks.map(bid => ({ type: "Block", value: bid.id })) : [],
          label: gobj.name || `${gobj.typeid} ${id.value}`,
          sourceLoc: gobj.overview_loc,
        };
        break;
      }
      case "Block": {
        const block = module.loadBlock(id.value);
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: "Block",
          parentId: { type: "Global", value: block.parent },
          childIds: block.insts.map(iid => ({ type: "Inst", value: iid.id })),
          label: block.name || `Block ${id.value}`,
          sourceLoc: block.source_loc,
        };
        break;
      }
      case "Inst": {
        const inst = module.loadInst(id.value);
        newNode = {
          moduleId: this.moduleId,
          selfId: id,
          kind: inst.typeid,
          parentId: { type: "Block", value: inst.parent },
          childIds: [], // instructions don't have children in the tree structure
          label: inst.name || `${inst.opcode} ${id.value}`,
          sourceLoc: inst.source_loc,
        };
        break;
      }
      default: {
        throw new Error(`Unknown SourceTrackable type: ${(id as any).type}`);
      }
    }
    console.debug(
      'TreeNodeStorage.expand: adding node',
      ir.sourceTrackableToString(newNode.selfId),
      'kind=', newNode.kind,
      'children=', newNode.childIds.length
    );
    this.set(newNode);
    return newNode;
  }
  expandChildren(id: ir.SourceTrackable, module: ModuleCache): GuideTreeNode[] {
    const node = this.get(id);
    const idStr = ir.sourceTrackableToString(id);
    if (!node) {
      throw new Error(`Node with ID ${idStr} not found in TreeNodeStorage`);
    }
    console.debug('TreeNodeStorage.expandChildren:', idStr, 'childCount=', node.childIds.length);
    return node.childIds.map(childId => {
      console.debug('TreeNodeStorage.expandChildren: expanding child', ir.sourceTrackableToString(childId));
      return this.expand(childId, module);
    });
  }
  dfsExpand(id: ir.SourceTrackable, module: ModuleCache): GuideTreeNode[] {
    console.debug('TreeNodeStorage.dfsExpand: start', ir.sourceTrackableToString(id));
    const result: GuideTreeNode[] = [];

    const expandRec = (ref: ir.SourceTrackable) => {
      const node = this.expand(ref, module);
      result.push(node);
      for (const childId of node.childIds) {
        expandRec(childId);
      }
    };

    expandRec(id);
    console.debug('TreeNodeStorage.dfsExpand: result count', result.length);
    return result;
  }
  collapse(id: ir.SourceTrackable): void {
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
  collapseChildren(id: ir.SourceTrackable): void {
    const node = this.get(id);
    if (!node) return;
    for (const childId of node.childIds) {
      this.collapse(childId);
    }
  }

  constReplace(node: GuideTreeNode): TreeNodeStorage {
    let newMap = this.shareClone();
    newMap.replace(node);
    return newMap;
  }
  constExpand(id: ir.SourceTrackable, module: ModuleCache): [GuideTreeNode, TreeNodeStorage] {
    const node = this.get(id);
    if (node)
      return [node, this];
    const newMap = this.shareClone();
    return [newMap.expand(id, module), newMap];
  }

  get(id: ir.SourceTrackable): GuideTreeNode | null {
    switch (id.type) {
      case "Module":
        return this.globalNode;
      case "Global": case "Block": case "Inst":
        return this.nodesById.get(id.value) || null;
      default:
        throw new Error(`Unknown SourceTrackable type: ${(id as any).type}`);
    }
  }

  export(module: ModuleCache): Exported.NodesAndEdges {
    const nodes: Exported.NodeData[] = [];
    const edges: Exported.EdgeData[] = [];
    if (!this.globalNode) {
      throw new Error("Cannot export TreeNodeStorage without global node");
    }

    function dfs(storj: TreeNodeStorage, node: GuideTreeNode): Exported.ExpandedNode {
      let children: Exported.NodeData[] = [];
      for (const childId of node.childIds) {
        const childNode = storj.get(childId);
        if (childNode) {
          let childExpanded = dfs(storj, childNode);
          children.push(childExpanded);
        } else {
          let unexpanded: Exported.NodeData = {
            expanded: false,
            label: getNodeIdLabel(module, childId),
            kind: irIdGetKind(module, childId),
            treeNode: childId,
          };
          children.push(unexpanded);
        }
      }
      let exportNode: Exported.ExpandedNode = {
        expanded: true,
        label: node.label,
        kind: node.kind,
        treeNode: node,
        children: children,
      };
      nodes.push(exportNode);
      const nodeIdStr = ir.sourceTrackableToString(node.selfId);
      for (const child of children) {
        if (!child.expanded)
          continue;
        let treeNodeIdStr = ir.sourceTrackableToString(child.treeNode.selfId);
        edges.push({
          id: `${nodeIdStr}->${treeNodeIdStr}`,
          source: exportNode,
          target: child,
        });
      }
      return exportNode;
    }

    dfs(this, this.globalNode);

    // remove all unexpanded nodes from nodesById, since they are not included in the export
    let newMap: Map<ir.PoolStrID, GuideTreeNode> = new Map();
    nodes.forEach(n => {
      if (!n.expanded)
        return;
      newMap.set(ir.sourceTrackableToString(n.treeNode.selfId) as ir.PoolStrID, n.treeNode);
    })
    this.nodesById = newMap;
    return {
      moduleId: this.moduleId,
      nodes,
      edges,
    };
  }
}

export namespace Exported {
  export type ExpandedNode = {
    expanded: true;
    label: string;
    kind: TreeNodeKind;
    treeNode: GuideTreeNode;
    children: NodeData[];
  };
  export type CollapsedNode = {
    expanded: false;
    label: string;
    kind: TreeNodeKind;
    treeNode: ir.SourceTrackable;
  };
  export type NodeData = ExpandedNode | CollapsedNode;

  export type EdgeData = {
    id: string;
    source: ExpandedNode;
    target: ExpandedNode;
  };
  export type NodesAndEdges = {
    moduleId: ir.ModuleID;
    nodes: NodeData[];
    edges: EdgeData[];
  };
}