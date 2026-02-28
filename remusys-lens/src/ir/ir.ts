import * as Wasm from "remusys-wasm";

/**
 * 池分配的 Indexed ID 类型, 格式: `{pool type}:{slot index}:{slot generation}`
 *
 * - `pool type`: 一个字符, 结构如下面的代码所示
 * - `slot index`: 十六进制无符号整数
 * - `slot genetation`: 十六进制无符号整数, 取值范围 `[1, FFFF]`. 0 代是无效值, 会被拦截.
 */
export type GlobalID = `g:${string}:${string}`;
export type BlockID = `b:${string}:${string}`;
export type InstID = `i:${string}:${string}`;
export type ExprID = `e:${string}:${string}`;
export type UseID = `u:${string}:${string}`;
export type JumpTargetID = `j:${string}:${string}`;

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

export type UserID = { Global: GlobalID } | { Expr: ExprID } | { Inst: InstID };
export type RefValueDt =
  | { Global: GlobalID }
  | { Block: BlockID }
  | { Inst: InstID }
  | { Expr: ExprID };

export type ValueDt =
  | "None"
  | { Undef: ValTypeID }
  | { I1: boolean }
  | { I8: number }
  | { I16: number }
  | { I32: number }
  | { I64: string } // use string to represent i64 literals, since bigint cannot transfer over JSON
  | { APInt: APIntDt }
  | { F32: number }
  | { F64: number }
  | { ZeroInit: AggrTypeID }
  | { FuncArg: [GlobalID, number] }
  | RefValueDt;
export type PoolAllocatedID =
  | RefValueDt
  | { Use: UseID }
  | { JumpTarget: JumpTargetID };

export type SourceTrackable = PoolAllocatedID | { FuncArg: [GlobalID, number] };

export function irTypeGetName(_t: ValTypeID): string {
  throw new Error("TODO");
}

export type ModuleBrief = { id: string };
export type SourceTy = "ir" | "sysy";
export function irCompileModule(source_ty: SourceTy, source: string): ModuleBrief {
  return Wasm.Api.compile_module(source_ty, source);
}

export type Linkage = "External" | "DSOLocal" | "Private";
export type SourcePos = {
  line: number; // 1-based
  column: number; // 1-based, UTF-16 code unit index
};
export type SourceLoc = {
  begin: SourcePos;
  end: SourcePos;
};
type GlobalObjBase = {
  id: GlobalID;
  name: string;
  linkage: Linkage;
  ty: ValTypeID;
  overview_loc: SourceLoc;
};
export type FuncArgDt = {
  name: string;
  ty: ValTypeID;
  source_loc: SourceLoc;
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
export function irGetModuleGlobalsBrief(module_id: string): ModuleGlobalsDt {
  return Wasm.Api.get_globals_brief(module_id);
}
export function irLoadGlobalObj(module_id: string, global_id: GlobalID): GlobalObjDt {
  return Wasm.Api.load_global_obj(module_id, global_id);
}
export function irLoadFuncObj(module_id: string, func_id: GlobalID): FuncObjDt {
  let obj = irLoadGlobalObj(module_id, func_id);
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
  name?: string;
  source_loc: SourceLoc;
  insts: InstDt[];
};
export function blockGetSuccs(block: BlockDt): JumpTargetDt[] {
  let last_inst = block.insts[block.insts.length - 1];
  if (last_inst.typeid === "Terminator") {
    return last_inst.succs;
  } else {
    return [];
  }
}

export type UseDt = {
  id: UseID;
  kind: UseKind;
  value: ValueDt;
  source_loc: SourceLoc;
};
type InstBase = {
  id: InstID;
  name?: string;
  opcode: Opcode;
  operands: UseDt[];
  source_loc: SourceLoc;
};
export type NormalInstDt = InstBase & { typeid: "Inst" };
export type TerminatorDt = InstBase & {
  typeid: "Terminator";
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
export function irUpdateFuncSource(module_id: string, func: GlobalID): SourceUpdates {
  return Wasm.Api.update_func_src(module_id, func);
}
export function irUpdateModuleOverviewSource(module_id: string): SourceUpdates {
  return Wasm.Api.update_overview_src(module_id);
}
export function irRenameID(module_id: string, id: SourceTrackable, new_name: string) {
  Wasm.Api.rename(module_id, id, new_name);
}
export function irValueGetUsedBy(module_id: string, val: ValueDt): UseID[] {
  return Wasm.Api.get_value_used_by(module_id, val);
}

export type FuncCloneInfo = {
  new_id: GlobalID;
  bb_map: [BlockID, BlockID][];
  inst_map: [InstID, InstID][];
};
export function irCloneFunction(module_id: string, func_id: GlobalID): FuncCloneInfo {
  return Wasm.Api.clone_function(module_id, func_id);
}
