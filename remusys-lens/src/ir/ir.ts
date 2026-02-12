import type * as api from "./api";
import * as remusys from "remusys-wasm";

/// Parse IR text and return IRTextInfo
function parseIRText(irText: string): api.IRTextInfo {
  return remusys.parse_ir_text(irText) as api.IRTextInfo;
}

/// Load global variable information for a modules
function loadIRGlobals(moduleId: string): api.GlobalInfo[] {
  return remusys.load_ir_globals(moduleId) as api.GlobalInfo[];
}

/// Load a ValueSSA value from remusys-wasm
function loadIRValue(moduleId: string, value: api.ValueDt): api.ModuleDelta {
  return remusys.load_value(moduleId, value) as api.ModuleDelta;
}

/// Get the name of a type by its ID
function irTypeGetName(moduleId: string, ty: api.ValTypeID): string {
  return remusys.ir_type_get_name(moduleId, ty);
}

/// Load the dominator tree for a function given its module ID and function index
function loadDominatorTree(moduleId: string, funcId: api.GlobalIndex): api.DominatorTreeDt {
  return remusys.get_dominator_tree(moduleId, funcId) as api.DominatorTreeDt;
}

/**
 * The main class representing an IR module, containing maps for types,
 * instructions, blocks, functions, global variables, and expressions.
 * It provides methods to load values and update the module state
 * based on deltas from remusys-wasm.
 */
export class IRModule {
  moduleId: string;
  instMap: Map<api.InstIndex, IRInst> = new Map();
  blockMap: Map<api.BlockIndex, IRBlock> = new Map();
  funcMap: Map<api.GlobalIndex, IRFunc> = new Map();
  gvarMap: Map<api.GlobalIndex, IRGVar> = new Map();
  exprMap: Map<api.ExprIndex, IRExpr> = new Map();

  private _typeMap: Map<api.ValTypeID, string> = new Map();
  private _symbolIds: Map<string, api.GlobalInfo> = new Map();

  private constructor(moduleId: string) {
    this.moduleId = moduleId;
    let globals = loadIRGlobals(this.moduleId);
    for (let globl of globals) {
      this._symbolIds.set(globl.name, globl);
    }
  }
  static fromIRText(irText: string): [IRModule, api.IRSourceMappingDt] {
    let info = parseIRText(irText);
    let module = new IRModule(info.module_id);
    return [module, info.src_mapping];
  }

  getTypeName(ty: api.ValTypeID): string {
    switch (ty) {
      case "void": case "ptr":
      case "i1": case "i8": case "i16": case "i32": case "i64": case "i128":
      case "float": case "double":
        return ty;
      default:
        break;
    }
    let oldtyName = this._typeMap.get(ty);
    if (oldtyName !== undefined) {
      return oldtyName;
    }
    let tyName = irTypeGetName(this.moduleId, ty);
    this._typeMap.set(ty, tyName);
    return tyName;
  }

  loadIRValue(value: api.ValueDt): IRValue | null {
    if (value == "None") {
      return null;
    } else if (value == "PtrNull") {
      return { type: "null", valType: "ptr" };
    } else if ("Undef" in value) {
      return { type: "undef", valType: value.Undef };
    } else if ("I1" in value) {
      return { type: "int", valType: "i1", value: value.I1 ? 1n : 0n };
    } else if ("I8" in value) {
      return { type: "int", valType: "i8", value: BigInt(value.I8) };
    } else if ("I16" in value) {
      return { type: "int", valType: "i16", value: BigInt(value.I16) };
    } else if ("I32" in value) {
      return { type: "int", valType: "i32", value: BigInt(value.I32) };
    } else if ("I64" in value) {
      return { type: "int", valType: "i64", value: BigInt(value.I64) };
    } else if ("AP" in value) {
      let ap: string = value.AP; // syntax: "i{bitWidth}:{value}"
      let match = ap.match(/^i(\d+):(-?\d+)$/);
      if (match) {
        let bitWidth = parseInt(match[1]);
        let intValue = BigInt(match[2]);
        return { type: "int", valType: `i${bitWidth}` as api.ValTypeID, value: intValue };
      } else {
        throw new Error(`Invalid APIntCodec format: ${ap}`);
      }
    } else if ("F32" in value) {
      return { type: "float", valType: "float", value: value.F32 };
    } else if ("F64" in value) {
      return { type: "float", valType: "double", value: value.F64 };
    } else if ("AggrZero" in value) {
      return { type: "zinit", valType: value.AggrZero };
    } else if ("Global" in value) {
      let [global, isFunc] = this.loadGlobal(value.Global);
      if (isFunc) {
        return { type: "func", repr: global as IRFunc };
      } else {
        return { type: "gvar", repr: global as IRGVar };
      }
    } else if ("Block" in value) {
      let block = this.loadBlock(value.Block);
      return { type: "block", repr: block };
    } else if ("Expr" in value) {
      let expr = this.loadExpr(value.Expr);
      return { type: "expr", repr: expr };
    } else if ("Inst" in value) {
      let inst = this.loadInst(value.Inst);
      return { type: "inst", repr: inst };
    } else if ("Arg" in value) {
      let funcIndex = value.Arg.func;
      let argIndex = value.Arg.index;
      let [global, isFunc] = this.loadGlobal(funcIndex);
      if (!isFunc) {
        throw new Error(`Global with index ${funcIndex} is not a function`);
      }
      let func = global as IRFunc;
      let operand = func.operandAt(argIndex);
      if (operand === null) {
        throw new Error(`Failed to load operand at index ${argIndex} for function with index ${funcIndex}`);
      }
      return operand;
    } else {
      throw new Error(`Unsupported ValueDt: ${JSON.stringify(value)}`);
    }
  }
  loadIRFunc(funcid: api.GlobalIndex): IRFunc {
    let [global, isFunc] = this.loadGlobal(funcid);
    if (!isFunc) {
      throw new Error(`Global with index ${funcid} is not a function`);
    }
    return global as IRFunc;
  }

  loadGlobal(index: api.GlobalIndex): [IRGlobalObj, boolean] {
    if (this.funcMap.has(index)) {
      return [this.funcMap.get(index), true] as [IRFunc, boolean];
    } else if (this.gvarMap.has(index)) {
      return [this.gvarMap.get(index), false] as [IRGVar, boolean];
    }
    this.updateValueDt({ Global: index });
    if (this.funcMap.has(index)) {
      return [this.funcMap.get(index), true] as [IRFunc, boolean];
    } else if (this.gvarMap.has(index)) {
      return [this.gvarMap.get(index), false] as [IRGVar, boolean];
    } else {
      throw new Error(`Failed to load global with index ${index}`);
    }
  }
  loadBlock(index: api.BlockIndex): IRBlock {
    if (this.blockMap.has(index)) {
      return this.blockMap.get(index) as IRBlock;
    }
    this.updateValueDt({ Block: index });
    if (this.blockMap.has(index)) {
      return this.blockMap.get(index) as IRBlock;
    } else {
      throw new Error(`Failed to load block with index ${index}`);
    }
  }
  loadExpr(index: api.ExprIndex): IRExpr {
    if (this.exprMap.has(index)) {
      return this.exprMap.get(index) as IRExpr;
    }
    this.updateValueDt({ Expr: index });
    if (this.exprMap.has(index)) {
      return this.exprMap.get(index) as IRExpr;
    } else {
      throw new Error(`Failed to load expression with index ${index}`);
    }
  }
  loadInst(index: api.InstIndex): IRInst {
    if (this.instMap.has(index)) {
      return this.instMap.get(index) as IRInst;
    }
    this.updateValueDt({ Inst: index });
    if (this.instMap.has(index)) {
      return this.instMap.get(index) as IRInst;
    } else {
      throw new Error(`Failed to load instruction with index ${index}`);
    }
  }

  updateValueDt(value: api.ValueDt): void {
    let delta = loadIRValue(this.moduleId, value);
    this.loadDelta(delta);
  }
  private loadDelta(delta: api.ModuleDelta): void {
    if (delta.dels !== undefined) {
      this.handleDeletions(delta.dels);
    }
    if (delta.adds !== undefined) {
      this.handleAdditions(delta.adds);
    }
  }
  private handleDeletions(dels: api.ModuleDel): void {
    for (let del of dels.inst) {
      this.instMap.delete(del);
    }
    for (let del of dels.block) {
      this.blockMap.delete(del);
    }
    for (let del of dels.globl) {
      if (this.funcMap.has(del)) {
        this.funcMap.delete(del);
      } else {
        this.gvarMap.delete(del);
      }
    }
    for (let del of dels.expr) {
      this.exprMap.delete(del);
    }
  }
  private handleAdditions(adds: api.ModuleAdd): void {
    for (let inst of adds.inst) {
      let instObj = new IRInst(this, inst.id, inst.opcode, inst.ty);
      instObj.useDt = inst.operands;
      this.instMap.set(inst.id, instObj);
    }
    for (let block of adds.block) {
      let blockObj = new IRBlock(this, block);
      this.blockMap.set(block.id, blockObj);
    }
    for (let globl of adds.gvar) {
      let gvarObj = new IRGVar(this, globl);
      this.gvarMap.set(globl.id, gvarObj);
    }
    for (let globl of adds.func) {
      let funcObj = new IRFunc(this, globl);
      this.funcMap.set(globl.id, funcObj);
    }
    for (let expr of adds.expr) {
      let exprObj = new IRExpr(this, expr);
      this.exprMap.set(expr.id, exprObj);
    }
  }
}

export type IRConstData =
  | { type: "int", valType: api.ValTypeID, value: bigint }
  | { type: "float", valType: api.ValTypeID, value: number }
  | { type: "null", valType: "ptr" }
  | { type: "undef", valType: api.ValTypeID };

export type IRValue =
  | IRConstData
  | { type: "zinit", valType: api.ValTypeID }
  | { type: "expr", repr: IRExpr }
  | { type: "inst", repr: IRInst }
  | { type: "gvar", repr: IRGVar }
  | { type: "func", repr: IRFunc }
  | { type: "block", repr: IRBlock };

export function irValueToDt(value: IRValue): api.ValueDt {
  switch (value.type) {
    case "null": return "PtrNull";
    case "undef": return { Undef: value.valType };
    case "int":
      switch (value.valType) {
        case "i1": return { I1: value.value !== 0n };
        case "i8": return { I8: Number(value.value) };
        case "i16": return { I16: Number(value.value) };
        case "i32": return { I32: Number(value.value) };
        case "i64": return { I64: value.value.toString() };
        default:
          if (value.valType.startsWith("i")) {
            return { AP: `i${value.valType.slice(1)}:${value.value.toString()}` };
          } else {
            throw new Error(`Unsupported integer type: ${value.valType}`);
          }
      }
    case "float":
      if (value.valType === "float") {
        return { F32: value.value };
      } else if (value.valType === "double") {
        return { F64: value.value };
      } else {
        throw new Error(`Unsupported float type: ${value.valType}`);
      }
    case "zinit":
      return { AggrZero: value.valType };
    case "gvar":
      return { Global: value.repr.id };
    case "func":
      return { Global: value.repr.id };
    case "block":
      return { Block: value.repr.index };
    case "expr":
      return { Expr: value.repr.index };
    case "inst":
      return { Inst: value.repr.index };
    default:
      throw new Error(`Unsupported IRValue type: ${(value as any).type}`);
  }
}

export interface TraceableValue {
  toValue(): IRValue;
  readonly ty: api.ValTypeID;
}

export interface IUser extends TraceableValue {
  readonly operands: Map<api.UseKind, IRValue>;
  operandAt(index: number): IRValue | null;
}

export class IRInst implements IUser, TraceableValue {
  module: IRModule;
  index: api.InstIndex;
  opcode: api.Opcode;
  ty: api.ValTypeID;
  private _use_dt: api.UseDt[];
  private _operands?: Map<api.UseKind, IRValue>;

  constructor(module: IRModule, index: api.InstIndex, opcode: api.Opcode, ty: api.ValTypeID) {
    this.module = module;
    this.index = index;
    this.opcode = opcode;
    this.ty = ty;
    this._use_dt = [];
  }

  toValue(): IRValue {
    return { type: "inst", repr: this };
  }

  set useDt(use_dt: api.UseDt[]) {
    this._use_dt = use_dt;
    this._operands = undefined; // Invalidate cached operands
  }

  get operands(): Map<api.UseKind, IRValue> {
    if (this._operands !== undefined) {
      return this._operands;
    }
    let operands = new Map<api.UseKind, IRValue>();
    for (let use_dt of this._use_dt) {
      let val = this.module.loadIRValue(use_dt.value);
      if (val !== null) {
        operands.set(use_dt.kind, val);
      }
    }
    this._operands = operands;
    return operands;
  }

  operandAt(index: number): IRValue | null {
    if (index < 0 || index >= this._use_dt.length) {
      return null;
    }
    let use_dt = this._use_dt[index];
    return this.operands.get(use_dt.kind) || null;
  }
}

export class IRBlock implements TraceableValue {
  ty: api.ValTypeID = "void";

  module: IRModule;
  index: api.BlockIndex;
  private _targetsDt: api.JTDt[];
  private _targets?: Map<api.JumpTargetKind, api.BlockIndex> = undefined;
  private _instIds: api.InstIndex[];
  private _insts?: IRInst[];

  constructor(module: IRModule, dt: api.BlockDt) {
    this.module = module;
    this.index = dt.id;
    this._targetsDt = dt.targets;
    this._instIds = dt.instrs;
  }
  toValue(): IRValue {
    return { type: "block", repr: this };
  }

  get targets(): Map<api.JumpTargetKind, api.BlockIndex> {
    if (this._targets !== undefined) {
      return this._targets;
    }
    let targets = new Map<api.JumpTargetKind, api.BlockIndex>();
    for (let jt_dt of this._targetsDt) {
      targets.set(jt_dt.kind, jt_dt.block);
    }
    this._targets = targets;
    return targets;
  }
  get insts(): IRInst[] {
    if (this._insts !== undefined) {
      return this._insts;
    }
    let insts: IRInst[] = [];
    for (let instId of this._instIds) {
      let value = this.module.loadIRValue({ Inst: instId });
      if (value === null || value.type !== "inst") {
        throw new Error(`Failed to load instruction with index ${instId}`);
      }
      insts.push(value.repr);
    }
    this._insts = insts;
    return insts;
  }
}
export class IRExpr implements TraceableValue, IUser {
  module: IRModule;
  index: api.ExprIndex;
  repr: string;
  ty: api.ValTypeID;
  private _use_dt: api.UseDt[] = [];
  private _operands?: Map<api.UseKind, IRValue>;

  get operands(): Map<api.UseKind, IRValue> {
    if (this._operands !== undefined) {
      return this._operands;
    }
    let operands = new Map<api.UseKind, IRValue>();
    for (let use_dt of this._use_dt) {
      let val = this.module.loadIRValue(use_dt.value);
      if (val !== null) {
        operands.set(use_dt.kind, val);
      }
    }
    this._operands = operands;
    return operands;
  }
  operandAt(index: number): IRValue | null {
    if (index < 0 || index >= this._use_dt.length) {
      return null;
    }
    let use_dt = this._use_dt[index];
    return this.operands.get(use_dt.kind) || null;
  }
  toValue(): IRValue {
    return { type: "expr", repr: this };
  }

  constructor(module: IRModule, dt: api.ExprDt) {
    this.module = module;
    this.index = dt.id;
    this.repr = dt.repr;
    this.ty = dt.ty;
    this._use_dt = dt.operands;
  }
}
export abstract class IRGlobalObj implements TraceableValue, IUser {
  ty: api.ValTypeID = "ptr";
  readonly linkage: api.Linkage;
  abstract operandAt(index: number): IRValue | null;
  abstract readonly operands: Map<api.ValTypeID, IRValue>;
  abstract toValue(): IRValue;
  abstract isFunc(): boolean;

  module: IRModule;
  name: string = "";
  id: api.GlobalIndex;

  protected constructor(module: IRModule, id: api.GlobalIndex, linkage: api.Linkage) {
    this.module = module;
    this.id = id;
    this.linkage = linkage;
  }
}
export class IRFunc extends IRGlobalObj {
  readonly operands: Map<string, IRValue> = new Map();
  toValue(): IRValue {
    return { type: "func", repr: this };
  }
  operandAt(_: number): IRValue | null {
    return null;
  }
  isFunc(): boolean {
    return true;
  }

  private _blocks_dt: api.BlockIndex[] | null;
  private _blocks?: IRBlock[];

  get blocks(): IRBlock[] | null {
    if (this._blocks !== undefined) {
      return this._blocks;
    }
    if (this._blocks_dt === null) {
      return null;
    }
    let blocks: IRBlock[] = [];
    for (let blockId of this._blocks_dt) {
      let value = this.module.loadIRValue({ Block: blockId });
      if (value === null || value.type !== "block") {
        throw new Error(`Failed to load block with index ${blockId}`);
      }
      blocks.push(value.repr);
    }
    this._blocks = blocks;
    return blocks;
  }

  constructor(module: IRModule, dt: api.FuncDt) {
    super(module, dt.id, dt.linkage);
    this._blocks_dt = dt.blocks !== undefined ? dt.blocks : null;
  }

  getDominatorTreeDt(): DominatorTree | null {
    if (this.blocks === null) {
      return null;
    }
    let dt = loadDominatorTree(this.module.moduleId, this.id);
    if (dt === null) {
      return null;
    } else {
      return new DominatorTree(this.module, dt);
    }
  }
}
export class IRGVar extends IRGlobalObj {
  operandAt(index: number): IRValue | null {
    if (index !== 0) {
      return null;
    }
    return this.initval;
  }

  private _operands?: Map<api.UseKind, IRValue>;
  private _initval?:
    | { type: "uninit", value: api.ValueDt }
    | { type: "resolve", value: IRValue };

  get initval(): IRValue | null {
    if (this._initval === undefined) {
      return null;
    }
    if (this._initval.type === "uninit") {
      let val = this.module.loadIRValue(this._initval.value);
      if (val !== null) {
        this._initval = { type: "resolve", value: val };
        return val;
      } else {
        return null;
      }
    } else {
      return this._initval.value;
    }
  }
  get operands(): Map<api.UseKind, IRValue> {
    if (this._operands !== undefined) {
      return this._operands;
    }
    let operands = new Map<api.UseKind, IRValue>();
    let initval = this.initval;
    if (initval !== null) {
      operands.set("initval", initval);
    }
    this._operands = operands;
    return operands;
  }

  isFunc(): boolean {
    return false;
  }

  toValue(): IRValue {
    return { type: "gvar", repr: this };
  }

  constructor(module: IRModule, dt: api.GVarDt) {
    super(module, dt.id, dt.linkage);
    this.name = dt.name;
    this._initval = dt.init !== undefined ?
      { type: "uninit", value: dt.init } :
      undefined;
  }
}

export type CfgNode =
  | { type: "block", repr: IRBlock }
  | { type: "vexit" }
  ;
export type DomTreeNode = {
  node: CfgNode;
  dfn: number;
  idom: DomTreeNode | null;
  children: DomTreeNode[];
};

export class DominatorTree {
  readonly module: IRModule;
  readonly nodes: DomTreeNode[];
  readonly nodeMap: Map<IRBlock, DomTreeNode>;
  readonly func: IRFunc;
  readonly isPostDom: boolean;

  constructor(module: IRModule, delta: api.DominatorTreeDt) {
    this.module = module;
    this.func = module.loadIRFunc(delta.func_id);
    this.isPostDom = delta.is_postdom;

    function nodeRepr(repr: api.DominatorNodeRepr): CfgNode {
      if (repr === "VExit") {
        return { type: "vexit" };
      } else {
        let block = module.loadBlock(repr.BB);
        return { type: "block", repr: block };
      }
    }

    let nodes: DomTreeNode[] = [];
    let nodeMap = new Map<IRBlock, DomTreeNode>();
    for (let node_dt of delta.nodes) {
      let node: DomTreeNode = {
        node: nodeRepr(node_dt.repr),
        dfn: nodes.length,
        idom: null,
        children: [],
      };
      nodes.push(node);
      if (node.node.type === "block") {
        nodeMap.set(node.node.repr, node);
      }
    }

    for (let node of nodes) {
      let dfn = node.dfn;
      let node_dt = delta.nodes[dfn];
      if (node_dt.idom !== undefined) {
        let idomRepr = node_dt.idom;
        let idomNode: DomTreeNode | null;
        if (idomRepr === "VExit") {
          idomNode = null;
        } else {
          let idomBlock = module.loadBlock(idomRepr.BB);
          idomNode = nodeMap.get(idomBlock) || null;
        }
        node.idom = idomNode;
      }
    }

    this.nodes = nodes;
    this.nodeMap = nodeMap;
  }
}
