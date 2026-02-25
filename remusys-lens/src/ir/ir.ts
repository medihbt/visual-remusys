export type GlobalID = `g:${string}:${string}`;
export type BlockID = `b:${string}:${string}`;
export type InstID = `i:${string}:${string}`;
export type ExprID = `e:${string}:${string}`;
export type UseID = `u:${string}:${string}`;
export type JumpTargetID = `j:${string}:${string}`;

/** IR opcode representation */
export type Opcode =
    | "and" | "or" | "xor" | "shl" | "lshr" | "ashr"
    | "add" | "sub" | "mul" | "sdiv" | "udiv" | "srem" | "urem"
    | "fadd" | "fsub" | "fmul" | "fdiv" | "frem"
    | "jmp" | "br" | "switch" | "ret" | "unreachable"
    | "sitofp" | "uitofp" | "fptosi" | "fptoui" | "zext" | "sext" | "trunc" | "fpext" | "fptrunc"
    | "bitcast" | "inttoptr" | "ptrtoint"
    | "select" | "extractelement" | "extractvalue" | "insertelement" | "insertvalue" | "getelementptr" | "offsetof"
    | "load" | "store" | "alloca" | "dyn-alloca"
    | "call" | "phi" | "icmp" | "fcmp"

    | "amo.xchg" | "amo.add" | "amo.sub" | "amo.and" | "amo.nand" | "amo.or" | "amo.xor"
    | "amo.max" | "amo.min" | "amo.umax" | "amo.umin"
    | "amo.fadd" | "amo.fsub" | "amo.fmax" | "amo.fmin"
    | "amo.uinc_wrap" | "amo.udec_wrap" | "amo.usub_cond" | "amo.sub_stat"

    | "constarray" | "conststruct" | "constvec" | "constptrnull"
    | "intrin" | "guide-node" | "phi-end"
    ;


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
    | "None" | "Jump" | "BrThen" | "BrElse" | "SwitchDefault"
    | `SwitchCase:${bigint}`
    ;

export type AggrTypeID =
    | `vec:${string}` | `arr:${string}`
    | `struct:${string}` | `alias:${string}`
    ;
export type ValTypeID =
    | "void" | "ptr" | "float" | "double" | AggrTypeID
    | `i${number}`
    | `func:${string}`
    ;

export type UserID =
    | { Global: GlobalID }
    | { Expr: ExprID }
    | { Inst: InstID }
    ;
export type RefValueDt =
    | { Global: GlobalID }
    | { Block: BlockID }
    | { Inst: InstID }
    | { Expr: ExprID }
    ;

export type ValueDt =
    | "None"
    | { Undef: ValTypeID }
    | { I1: boolean }
    | { I8: number } | { I16: number } | { I32: number }
    | { I64: string } // use string to represent i64 literals, since bigint cannot transfer over JSON
    | { APInt: APIntDt }
    | { F32: number } | { F64: number }
    | { ZeroInit: AggrTypeID }
    | { FuncArg: [GlobalID, number] }
    | RefValueDt
    ;
export type ReferenceDt =
    | RefValueDt
    | { Use: UseID }
    | { JumpTarget: JumpTargetID }
    ;

export type IDWithSourceLoc = ReferenceDt | { FuncArg: [GlobalID, number] };

export function irTypeGetName(t: ValTypeID): string {
    throw new Error("TODO");
}

export type ModuleBrief = { id: string };
export type SourceTy = "IR" | "SysY";
export function irCompileModule(source_ty: SourceTy, source: string): ModuleBrief {
    throw new Error("TODO");
}

export type Linkage = "External" | "DSOLocal" | "Private";
export type SourcePos = {
    line: number;   // 1-based
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
    loc: SourceLoc;
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
export function irGetModuleGlobals(module_id: string): ModuleGlobalsDt {
    throw new Error("TODO");
}
export function irLoadGlobalObj(module_id: string, global_id: GlobalID): GlobalObjDt {
    throw new Error("TODO");
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
    id: BlockID;
    name?: string;
    source_loc: SourceLoc;
    insts: InstDt[];
};
export function irLoadBlocks(module_id: string, blocks: BlockID[]): BlockDt[] {
    throw new Error("TODO");
}
export function irLoadBlock(module_id: string, block_id: BlockID): BlockDt {
    throw new Error("TODO");
}
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
    opcode: Opcode;
    operands: UseDt[];
    source_loc: SourceLoc;
};
export type NormalInstDt = InstBase & { typeid: "Inst"; };
export type TerminatorDt = InstBase & {
    typeid: "Terminator";
    succs: JumpTargetDt[];
};
export type PhiInstDt = InstBase & {
    typeid: "Phi";
    incomings: { value: ValueDt; from: BlockID; }[];
};
export type InstDt = NormalInstDt | TerminatorDt | PhiInstDt;
export function irLoadInst(inst_id: InstID): InstDt {
    throw new Error("TODO");
}

export type SourceRangeUpdate = {
    id: IDWithSourceLoc;
    new_loc: SourceLoc;
}
export type SourceUpdate = {
    scope: "Func" | "Module",
    source: string;
    ranges: SourceRangeUpdate[];
};
export function irUpdateFuncSource(module_id: string, func: GlobalID): SourceUpdate {
    throw new Error("TODO");
}
export function irUpdateModuleOverviewSource(module_id: string): SourceUpdate {
    throw new Error("TODO");
}
export function irRenameID(module_id: string, id: IDWithSourceLoc, new_name: string): SourceUpdate {
    throw new Error("TODO");
}
export function irValueGetUsedBy(module_id: string, val: ValueDt): UseID[] {
    throw new Error("TODO");
}

export type FuncCloneInfo = {
    new_id: GlobalID;
    bb_map: [BlockID, BlockID][];
    inst_map: [InstID, InstID][];
};
export function irCloneFunction(module_id: string, func_id: GlobalID): FuncCloneInfo {
    throw new Error("TODO");
}
