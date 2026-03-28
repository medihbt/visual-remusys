import { Api } from "remusys-wasm";

/**
 * 池分配的 Indexed ID 类型, 格式: `g:{slot index}:{slot generation}`
 *
 * - `pool type` = 'g': 该 ID 用于 "全局对象内存池"
 * - `slot index`: 十六进制无符号整数
 * - `slot genetation`: 十六进制无符号整数, 取值范围 `[1, FFFF]`. 0 代是无效值, 会被拦截.
 */
export type GlobalID = `g:${string}:${string}`;

/**
 * 池分配的 Indexed ID 类型, 格式: `b:{slot index}:{slot generation}`
 *
 * - `pool type` = 'b': 该 ID 用于 "基本块内存池"
 * - `slot index`: 十六进制无符号整数
 * - `slot genetation`: 十六进制无符号整数, 取值范围 `[1, FFFF]`. 0 代是无效值, 会被拦截.
 */
export type BlockID = `b:${string}:${string}`;

/**
 * 池分配的 Indexed ID 类型, 格式: `i:{slot index}:{slot generation}`
 *
 * - `pool type` = 'i': 该 ID 用于 "指令内存池"
 * - `slot index`: 十六进制无符号整数
 * - `slot genetation`: 十六进制无符号整数, 取值范围 `[1, FFFF]`. 0 代是无效值, 会被拦截.
 */
export type InstID = `i:${string}:${string}`;

/**
 * 池分配的 Indexed ID 类型, 格式: `e:{slot index}:{slot generation}`
 *
 * - `pool type` = 'e': 该 ID 用于 "表达式内存池"
 * - `slot index`: 十六进制无符号整数
 * - `slot genetation`: 十六进制无符号整数, 取值范围 `[1, FFFF]`. 0 代是无效值, 会被拦截.
 */
export type ExprID = `e:${string}:${string}`;
export type UseID = `u:${string}:${string}`;
export type JumpTargetID = `j:${string}:${string}`;

export type ModuleID = `module-${number}`;

export type PoolStrID =
  | GlobalID
  | BlockID
  | InstID
  | ExprID
  | UseID
  | JumpTargetID;

export type BlockDfgNodeID =
  | InstID
  | ExprID
  | BlockID
  | GlobalID
  | UseID
  | `FuncArg(${GlobalID}, ${number})`;

export class IDCast {
  static asGlobal(id: string): id is GlobalID {
    return /^g:[0-9a-fA-F]+:[0-9a-fA-F]+$/.test(id);
  }
  static asBlock(id: string): id is BlockID {
    return /^b:[0-9a-fA-F]+:[0-9a-fA-F]+$/.test(id);
  }
  static asInst(id: string): id is InstID {
    return /^i:[0-9a-fA-F]+:[0-9a-fA-F]+$/.test(id);
  }
  static asExpr(id: string): id is ExprID {
    return /^e:[0-9a-fA-F]+:[0-9a-fA-F]+$/.test(id);
  }
  static asUse(id: string): id is UseID {
    return /^u:[0-9a-fA-F]+:[0-9a-fA-F]+$/.test(id);
  }
  static asJumpTarget(id: string): id is JumpTargetID {
    return /^j:[0-9a-fA-F]+:[0-9a-fA-F]+$/.test(id);
  }
  static asPoolStrID(id: string): id is PoolStrID {
    return (
      IDCast.asGlobal(id) ||
      IDCast.asBlock(id) ||
      IDCast.asInst(id) ||
      IDCast.asExpr(id) ||
      IDCast.asUse(id) ||
      IDCast.asJumpTarget(id)
    );
  }
  static asSourceTrackable(id: string): SourceTrackable | null {
    if (IDCast.asGlobal(id)) {
      return { type: "Global", value: id };
    } else if (IDCast.asBlock(id)) {
      return { type: "Block", value: id };
    } else if (IDCast.asInst(id)) {
      return { type: "Inst", value: id };
    } else if (IDCast.asExpr(id)) {
      return { type: "Expr", value: id };
    } else if (IDCast.asUse(id)) {
      return { type: "Use", value: id };
    } else if (IDCast.asJumpTarget(id)) {
      return { type: "JumpTarget", value: id };
    }
    return null;
  }
  static asBlockDfgNodeID(id: string): id is BlockDfgNodeID {
    return (
      IDCast.asInst(id) ||
      IDCast.asExpr(id) ||
      IDCast.asBlock(id) ||
      IDCast.asGlobal(id) ||
      IDCast.asUse(id) ||
      /^FuncArg\(.+\)$/.test(id)
    );
  }
}

/** IR opcode representation */
export type Opcode =
  | "and"
  | "or"
  | "xor"
  | "shl"
  | "lshr"
  | "ashr"
  | "add"
  | "sub"
  | "mul"
  | "sdiv"
  | "udiv"
  | "srem"
  | "urem"
  | "fadd"
  | "fsub"
  | "fmul"
  | "fdiv"
  | "frem"
  | "jmp"
  | "br"
  | "switch"
  | "ret"
  | "unreachable"
  | "sitofp"
  | "uitofp"
  | "fptosi"
  | "fptoui"
  | "zext"
  | "sext"
  | "trunc"
  | "fpext"
  | "fptrunc"
  | "bitcast"
  | "inttoptr"
  | "ptrtoint"
  | "select"
  | "extractelement"
  | "extractvalue"
  | "insertelement"
  | "insertvalue"
  | "getelementptr"
  | "offsetof"
  | "load"
  | "store"
  | "alloca"
  | "dyn-alloca"
  | "call"
  | "phi"
  | "icmp"
  | "fcmp"
  | "amo.xchg"
  | "amo.add"
  | "amo.sub"
  | "amo.and"
  | "amo.nand"
  | "amo.or"
  | "amo.xor"
  | "amo.max"
  | "amo.min"
  | "amo.umax"
  | "amo.umin"
  | "amo.fadd"
  | "amo.fsub"
  | "amo.fmax"
  | "amo.fmin"
  | "amo.uinc_wrap"
  | "amo.udec_wrap"
  | "amo.usub_cond"
  | "amo.sub_stat"
  | "constarray"
  | "conststruct"
  | "constvec"
  | "constptrnull"
  | "intrin"
  | "guide-node"
  | "phi-end";

export type APIntDt = {
  bits: number;
  /**
   * Use string to represent APInt literals, since bigint cannot transfer over JSON
   * The string should be a decimal representation of the integer, and can be negative for signed integers.
   * Note that the value should fit in the specified number of bits
   * (e.g. for a 8-bit APInt, the value should be in the range [-128, 255]).
   */
  value: string;
};

export type UseKind = string; // too complex to enumerate
export type JTKind =
  | "None"
  | "Jump"
  | "BrThen"
  | "BrElse"
  | "SwitchDefault"
  | `SwitchCase:${bigint}`;

export type AggrTypeID =
  | `vec:${string}`
  | `arr:${string}`
  | `struct:${string}`
  | `alias:${string}`;
export type ValTypeID =
  | "void"
  | "ptr"
  | "float"
  | "double"
  | AggrTypeID
  | `i${number}`
  | `func:${string}`;

export type UserID =
  | { type: "Inst"; value: InstID }
  | { type: "Global"; value: GlobalID }
  | { type: "Expr"; value: ExprID };
export type RefValueDt = UserID | { type: "Block"; value: BlockID };
export type ValueDt =
  | { type: "None" }
  | { type: "Undef"; value: ValTypeID }
  | { type: "I1"; value: boolean }
  | { type: "I8"; value: number }
  | { type: "I16"; value: number }
  | { type: "I32"; value: number }
  | { type: "I64"; value: string } // use string to represent i64 literals, since bigint cannot transfer over JSON
  | { type: "APInt"; value: APIntDt }
  | { type: "F32"; value: number }
  | { type: "F64"; value: number }
  | { type: "ZeroInit"; value: AggrTypeID }
  | { type: "FuncArg"; value: [GlobalID, number] }
  | RefValueDt;
export type PoolAllocatedID =
  | RefValueDt
  | { type: "Use"; value: UseID }
  | { type: "JumpTarget"; value: JumpTargetID };
export type SourceTrackable =
  | PoolAllocatedID
  | { type: "FuncArg"; value: [GlobalID, number] }
  | { type: "Module" };

export function sourceTrackableToString(st: SourceTrackable): string {
  switch (st.type) {
    case "FuncArg":
      return `FuncArg(${st.value[0]}, ${st.value[1]})`;
    case "Module":
      return "Module";
    case "Global":
    case "Block":
    case "Inst":
    case "Expr":
    case "Use":
    case "JumpTarget":
      return st.value;
    default:
      throw new Error(`Unknown SourceTrackable: ${JSON.stringify(st)}`);
  }
}

export function irTypeGetName(module_id: string, t: ValTypeID): string {
  switch (t) {
    case "void":
    case "ptr":
    case "float":
    case "double":
    case "i1":
    case "i8":
    case "i16":
    case "i32":
    case "i64":
      return t;
    default:
      return t.startsWith("i") ? t : Api.type_get_name(module_id, t);
  }
}

export type ModuleBrief = { id: ModuleID };
export type SourceTy = "ir" | "sysy";
export function irCompileModule(
  source_ty: SourceTy,
  source: string,
): ModuleBrief {
  return Api.compile_module(source_ty, source);
}

export type Linkage = "External" | "DSOLocal" | "Private";
export type SourcePos = {
  line: number; // 1-based
  column: number; // 1-based, UTF-16 code unit index
};
export type SourceLoc = { begin: SourcePos; end: SourcePos };
export type GlobalObjBase = {
  id: GlobalID;
  name: string;
  linkage: Linkage;
  ty: ValTypeID;
  overview_loc: SourceLoc;
};
export type FuncArgDt = {
  name: string;
  ty: ValTypeID;
  source_loc?: SourceLoc;
};
export type FuncObjDt = GlobalObjBase & {
  typeid: "Func";
  blocks?: BlockDt[];
  source: string;
  ret_ty: ValTypeID;
  args: FuncArgDt[];
};
export type GlobalVarObjDt = GlobalObjBase & {
  typeid: "GlobalVar";
  init: ValueDt;
};
export type GlobalObjDt = FuncObjDt | GlobalVarObjDt;
export type ModuleGlobalsDt = {
  overview_src: string;
  globals: GlobalObjBase[];
};
export function irGetModuleGlobalsBrief(module_id: ModuleID): ModuleGlobalsDt {
  return Api.get_globals_brief(module_id);
}
export function irLoadGlobalObj(
  module_id: ModuleID,
  global_id: GlobalID,
): GlobalObjDt {
  return Api.load_global_obj(module_id, global_id);
}

/// 如果 ID 是函数定义作用域内的东西, 就返回这个函数定义的 ID. 否则返回 undefined.
export function irFuncScopeOfId(
  module_id: ModuleID,
  id: SourceTrackable | PoolStrID,
): GlobalID | undefined {
  return Api.func_scope_of_id(module_id, id);
}
/// 如果 ID 是函数定义作用域内的东西, 就把这个函数定义加载上来. 否则返回 undefined.
export function irLoadFuncOfScope(
  module_id: ModuleID,
  id: SourceTrackable | PoolStrID,
): GlobalObjDt | undefined {
  return Api.load_func_of_scope(module_id, id);
}
export function irLoadFuncObj(
  module_id: ModuleID,
  func_id: GlobalID,
): FuncObjDt {
  const obj = irLoadGlobalObj(module_id, func_id);
  if (obj.typeid !== "Func") {
    throw new Error(`Global ${func_id} is not a function`);
  }
  return obj as FuncObjDt;
}

export type JumpTargetDt = {
  id: JumpTargetID;
  kind: JTKind;
  target: BlockID;
  source_loc: SourceLoc;
};
export type BlockDt = {
  typeid: "Block";
  id: BlockID;
  parent: GlobalID;
  name?: string;
  source_loc: SourceLoc;
  insts: InstDt[];
};
export function blockGetSuccs(block: BlockDt): JumpTargetDt[] {
  const last_inst = block.insts[block.insts.length - 1];
  if (last_inst.typeid === "Terminator") {
    return last_inst.succs;
  } else {
    return [];
  }
}

export type UseDt = {
  id: UseID;
  user: UserID;
  kind: UseKind;
  value: ValueDt;
  source_loc?: SourceLoc;
};
type InstBase = {
  id: InstID;
  parent: BlockID;
  name?: string;
  opcode: Opcode;
  operands: UseDt[];
  source_loc: SourceLoc;
};
export type NormalInstDt = InstBase & { typeid: "Inst" };
export type TerminatorDt = InstBase & {
  typeid: "Terminator";
  terminator: InstID;
  succs: JumpTargetDt[];
};
export type PhiInstDt = InstBase & {
  typeid: "Phi";
  incomings: { value: ValueDt; from: BlockID }[];
};
export type InstDt = NormalInstDt | TerminatorDt | PhiInstDt;

export type SourceLocUpdate = {
  id: SourceTrackable;
  new_loc: SourceLoc;
};
export type SourceUpdates = {
  scope: "Func" | "Module";
  source: string;
  ranges: SourceLocUpdate[];
  elliminated: SourceTrackable[];
};
export function irUpdateFuncSource(
  module_id: ModuleID,
  func: GlobalID,
): SourceUpdates {
  return Api.update_func_src(module_id, func);
}
export function irUpdateModuleOverviewSource(
  module_id: ModuleID,
): SourceUpdates {
  return Api.update_overview_src(module_id);
}
export function irRenameID(
  module_id: ModuleID,
  id: SourceTrackable,
  new_name: string,
) {
  Api.rename(module_id, id, new_name);
}
export function irValueGetUsedBy(module_id: ModuleID, val: ValueDt): UseID[] {
  return Api.get_value_used_by(module_id, val);
}

export type FuncCloneInfo = {
  new_id: GlobalID;
  bb_map: [BlockID, BlockID][];
  inst_map: [InstID, InstID][];
};
export function irCloneFunction(
  module_id: ModuleID,
  func_id: GlobalID,
): FuncCloneInfo {
  return Api.clone_function(module_id, func_id);
}

export type IRValueObjectDt = GlobalObjDt | BlockDt | InstDt;

export type CfgNodeKind = "Entry" | "Control" | "Exit";
export type CfgEdgeClass = "SelfRing" | "Tree" | "Back" | "Forward" | "Cross" | "Unreachable";
export type CfgNode = {
  id: BlockID;
  label: string;
  kind: CfgNodeKind;
};
export type CfgEdge = {
  id: JumpTargetID;
  from: BlockID;
  to: BlockID;
  kind: JTKind;
  is_critical: boolean;
  edge_class: CfgEdgeClass;
};
export type FuncCfgDt = {
  nodes: CfgNode[];
  edges: CfgEdge[];
};
export function irMakeCfg(module_id: string, func: GlobalID): FuncCfgDt {
  return Api.make_func_cfg(module_id, func);
}


export type DomTreeDt = {
  nodes: BlockID[];
  edges: [BlockID, BlockID][];
};
export function makeDominatorTree(
  module_id: ModuleID,
  func_id: GlobalID,
): DomTreeDt {
  return Api.make_dominator_tree(module_id, func_id);
}

export type BlockDfgNodeDt = {
  id: BlockDfgNodeID;
  value: ValueDt;
};
export type BlockDfgEdgeDt = {
  id: UseID;
  kind: UseKind;
  user: BlockDfgNodeID;
  operand: BlockDfgNodeID;
  section_id?: number;
};
export type BlockDfgSectionKind = "Pure" | "Effect" | "Income" | "Outcome";
export type BlockDfgSectionDt = {
  id: number;
  nodes: BlockDfgNodeDt[];
  kind: BlockDfgSectionKind;
};
export type BlockDfgDt = {
  nodes: BlockDfgSectionDt[];
  edges: BlockDfgEdgeDt[];
};

export function irMakeBlockDfg(
  module_id: ModuleID,
  block_id: BlockID,
): BlockDfgDt {
  return Api.make_block_dfg(module_id, block_id);
}
export type CallNodeRole = "Root" | "Live" | "Indirect" | "Unreachable";

export type CallGraphNode = {
  id: GlobalID;
  role: CallNodeRole;
};

export type CallGraphEdge = {
  id: UseID;
  caller: GlobalID;
  callee: GlobalID;
};

export type CallGraphDt = {
  nodes: CallGraphNode[];
  edges: CallGraphEdge[];
};

export function irMakeCallGraph(module_id: ModuleID): CallGraphDt {
  return Api.make_call_graph(module_id);
}
