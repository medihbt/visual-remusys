// Auto-aligned with remusys-wasm-b2 DTO/module serialization shapes.

export type SourceTy = "ir" | "sysy";

export type GlobalID = `g:${string}:${string}`;
export type BlockID = `b:${string}:${string}`;
export type InstID = `i:${string}:${string}`;
export type ExprID = `e:${string}:${string}`;
export type UseID = `u:${string}:${string}`;
export type JumpTargetID = `j:${string}:${string}`;

export type PoolStrID =
    | GlobalID
    | BlockID
    | InstID
    | ExprID
    | UseID
    | JumpTargetID;

export interface MonacoSrcPos {
    // 1-based line number
    line: number;
    // 1-based UTF-16 column
    column: number;
}

export interface MonacoSrcRange {
    start: MonacoSrcPos;
    end: MonacoSrcPos;
}

// remusys-ir::base::APInt serde payload.
export interface APIntDt {
    bits: number;
    // decimal string
    value: string;
}

// In b2, type IDs come from remusys-ir serde and are best treated as opaque string IDs.
export type ValTypeID = string;
export type AggrType = string;

export type ValueDt =
    | { type: "None" }
    | { type: "Undef"; value: ValTypeID }
    | { type: "PtrNull" }
    | { type: "I1"; value: boolean }
    | { type: "I8"; value: number }
    | { type: "I16"; value: number }
    | { type: "I32"; value: number }
    // Rust StrI64 serializes as decimal string.
    | { type: "I64"; value: string }
    | { type: "APInt"; value: APIntDt }
    | { type: "F32"; value: number }
    | { type: "F64"; value: number }
    | { type: "ZeroInit"; value: AggrType }
    | { type: "FuncArg"; value: [GlobalID, number] }
    | { type: "Global"; value: GlobalID }
    | { type: "Block"; value: BlockID }
    | { type: "Inst"; value: InstID }
    | { type: "Expr"; value: ExprID };

export type IRTreeObjID =
    | { type: "Module" }
    | { type: "Global"; value: GlobalID }
    | { type: "FuncArg"; value: [GlobalID, number] }
    | { type: "Block"; value: BlockID }
    | { type: "Inst"; value: InstID }
    | { type: "Use"; value: UseID }
    | { type: "JumpTarget"; value: JumpTargetID }
    | { type: "FuncHeader"; value: GlobalID }
    | { type: "BlockIdent"; value: BlockID }
    ;
export function irTreeObjIdToStr(id: IRTreeObjID): string {
    return JSON.stringify(id);
}
export function irTreeObjIdFromStr(s: string): IRTreeObjID {
    return JSON.parse(s) as IRTreeObjID;
}

export type IRObjPath = IRTreeObjID[];

export type IRTreeNodeClass =
    | "Module"
    | "GlobalVar"
    | "ExternFunc"
    | "Func"
    | "FuncArg"
    | "Block"
    | "PhiInst"
    | "NormalInst"
    | "TerminatorInst"
    | "Use"
    | "JumpTarget";

export interface IRTreeNodeDt {
    obj: IRTreeObjID;
    kind: IRTreeNodeClass;
    label: string;
    src_range: MonacoSrcRange;
}

export type CfgNodeRole = "Entry" | "Branch" | "Exit";

export interface CfgNodeDt {
    role: CfgNodeRole;
    block: BlockID;
    label: string;
}

export type CfgEdgeDfsRole = "Tree" | "Back" | "SelfRing" | "Forward" | "Cross";

// remusys-ir::JumpTargetKind serializes as display string.
export type JumpTargetKind =
    | "None"
    | "Jump"
    | "BrThen"
    | "BrElse"
    | "SwitchDefault"
    | `SwitchCase:${number}`
    | "Disposed";

export interface CfgEdgeDt {
    id: JumpTargetID;
    from: BlockID;
    to: BlockID;
    dfs_role: CfgEdgeDfsRole;
    jt_kind: JumpTargetKind;
}

export interface FuncCfgDt {
    nodes: CfgNodeDt[];
    edges: CfgEdgeDt[];
}

export type DfgNodeID =
    | InstID
    | ExprID
    | BlockID
    | GlobalID
    | UseID
    | `FuncArg(${GlobalID}, ${number})`;

export type DfgNodeRole =
    | "Income"
    | "Outgo"
    | "Phi"
    | "Pure"
    | "Effect"
    | "Terminator";

export interface DfgNode {
    id: DfgNodeID;
    value: ValueDt;
    role: DfgNodeRole;
}

export interface DfgSection {
    kind: DfgNodeRole;
    nodes: DfgNode[];
}

export interface DfgEdge {
    id: UseID;
    from: DfgNodeID;
    to: DfgNodeID;
}

export interface BlockDfg {
    sections: DfgSection[];
    edges: DfgEdge[];
}

export type CallGraphNodeRole = "Public" | "Private" | "Extern";

export interface CallGraphNodeDt {
    id: GlobalID;
    label: string;
    role: CallGraphNodeRole;
}

export interface CallGraphEdgeDt {
    from: GlobalID;
    to: GlobalID;
}

export interface CallGraphDt {
    nodes: CallGraphNodeDt[];
    edges: CallGraphEdgeDt[];
}

export interface DomTreeDt {
    nodes: BlockID[];
    edges: Array<[BlockID, BlockID]>;
}

// module/rename.rs #[serde(tag = "type")]
export type RenameRes =
    | { type: "Renamed" }
    | { type: "NoChange" }
    | { type: "GlobalNameConflict"; name: string }
    | { type: "LocalNameConflict"; name: string }
    | { type: "UnnamedObject" };

