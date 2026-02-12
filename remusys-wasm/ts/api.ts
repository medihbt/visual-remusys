/* All indexed IDs are strings */
export type GlobalIndex = string;
export type UseIndex = string;
export type JumpTargetIndex = string;
export type BlockIndex = string;
export type InstIndex = string;
export type ExprIndex = string;

/* UseKind and JTKind are strings */
export type UseKind = string;
export type JTKind = string;
export type JumpTargetKind = JTKind;
export type Opcode = string;
export type Linkage = "external" | "dso_local" | "private";

/* remusys-ir type IDs are serialized as strings */
export type ValTypeID = string;
export type AggrType = ValTypeID;

/* Numeric codecs serialized as strings */
export type I64Codec = string;
export type APIntCodec = string;

export interface SourceSpan {
    /** the begin line of the source location (1-based) */
    begin_line: number;
    /** the begin column of the source location in UTF-16 code units (0-based) */
    begin_column: number;
    /** the end line of the source location (1-based) */
    end_line: number;
    /** the end column of the source location in UTF-16 code units (0-based) */
    end_column: number;
}

/// Source mapping for a function
export interface IRFuncSrcMappingDt {
    /** span of the function header */
    head_span: SourceSpan;
    /** span of the full function including body */
    full_span: SourceSpan;
    id: GlobalIndex;
    args: SourceSpan[];
}

/// Source mapping for a module
export interface IRSourceMappingDt {
    source: string;
    uses: Array<[SourceSpan, UseIndex]>;
    jts: Array<[SourceSpan, JumpTargetIndex]>;
    blocks: Array<[SourceSpan, BlockIndex]>;
    insts: Array<[SourceSpan, InstIndex]>;
    gvars: Array<[SourceSpan, GlobalIndex]>;
    funcs: IRFuncSrcMappingDt[];
}

/// Information about the IR text compilation result
export interface IRTextInfo {
    module_id: string;
    src_mapping: IRSourceMappingDt;
}

/// DTO of ValueSSA.
export type ValueDt =
    | "None"
    | "PtrNull"
    | { Undef: ValTypeID }
    | { I1: boolean }
    | { I8: number }
    | { I16: number }
    | { I32: number }
    | { I64: I64Codec }
    | { AP: APIntCodec }
    | { F32: number }
    | { F64: number }
    | { AggrZero: AggrType }
    | { Global: GlobalIndex }
    | { Block: BlockIndex }
    | { Inst: InstIndex }
    | { Expr: ExprIndex }
    | { Arg: { func: GlobalIndex; index: number } };

export interface ArrayTypeDt {
    id: ValTypeID;
    elem: ValTypeID;
    len: number;
}

export interface StructTypeDt {
    id: ValTypeID;
    fields: ValTypeID[];
}

export interface AliasTypeDt {
    id: ValTypeID;
    name: string;
    aliased: AggrType;
}

export interface FuncTypeDt {
    id: ValTypeID;
    args: ValTypeID[];
    ret: ValTypeID;
}

export interface UseDt {
    kind: UseKind;
    value: ValueDt;
}

export interface JTDt {
    kind: JumpTargetKind;
    block: BlockIndex;
}

export interface InstDt {
    id: InstIndex;
    repr: string;
    opcode: Opcode;
    ty: ValTypeID;
    operands: UseDt[];
}

export interface ExprDt {
    id: ExprIndex;
    repr: string;
    ty: ValTypeID;
    operands: UseDt[];
}

export interface BlockDt {
    id: BlockIndex;
    instrs: InstIndex[];
    targets: JTDt[];
}

export interface FuncDt {
    id: GlobalIndex;
    linkage: Linkage;
    name: string;
    ty: ValTypeID;
    ret: ValTypeID;
    args: ValTypeID[];
    blocks: BlockIndex[] | undefined;
}

export interface GVarDt {
    id: GlobalIndex;
    linkage: Linkage;
    name: string;
    ty: ValTypeID;
    init: ValueDt | undefined;
}

export interface TypeCtxDelta {
    structs: StructTypeDt[];
    aliases: AliasTypeDt[];
    arrays: ArrayTypeDt[];
    funcs: FuncTypeDt[];
}

export interface ModuleDelta {
    tctx: TypeCtxDelta | undefined;
    dels: ModuleDel | undefined;
    adds: ModuleAdd | undefined;
}

export interface ModuleDel {
    inst: InstIndex[];
    expr: ExprIndex[];
    globl: GlobalIndex[];
    block: BlockIndex[];
}

export interface ModuleAdd {
    inst: InstDt[];
    expr: ExprDt[];
    block: BlockDt[];
    func: FuncDt[];
    gvar: GVarDt[];
}

export interface GlobalInfo {
    name: string;
    is_func: boolean;
    id: GlobalIndex;
}

// DTO of DominatorTree
export type DominatorNodeRepr = "VExit" | { BB: BlockIndex };

export interface DominatorNodeDt {
    repr: DominatorNodeRepr;
    idom?: DominatorNodeRepr;
    semidom?: DominatorNodeRepr;
}
export interface DominatorTreeDt {
    func_id: GlobalIndex;
    nodes: DominatorNodeDt[];
    is_postdom: boolean;
}
