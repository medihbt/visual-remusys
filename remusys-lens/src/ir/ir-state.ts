import type { WritableDraft } from "immer";
import * as ir from "./ir";
import type { ModuleID } from "./ir";
import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

export class ModuleCache {
  readonly moduleId: ModuleID;
  globals: Map<ir.GlobalID, ir.GlobalObjDt>;
  blocks: Map<ir.BlockID, ir.BlockDt>;
  insts: Map<ir.InstID, ir.InstDt>;
  jts: Map<ir.JumpTargetID, ir.JumpTargetDt>;
  uses: Map<ir.UseID, ir.UseDt>;
  private _brief?: ir.ModuleGlobalsDt;

  get brief(): ir.ModuleGlobalsDt {
    if (!this._brief) {
      this._brief = ir.irGetModuleGlobalsBrief(this.moduleId);
    }
    return this._brief;
  }

  constructor(id: ModuleID) {
    this.moduleId = id;
    this.globals = new Map();
    this.blocks = new Map();
    this.insts = new Map();
    this.jts = new Map();
    this.uses = new Map();
  }

  static compileFrom(srcKind: ir.SourceTy, src: string): ModuleCache {
    const moduleBrief = ir.irCompileModule(srcKind, src);
    const cache = new ModuleCache(moduleBrief.id);
    cache.refreshBrief();
    return cache;
  }

  refreshBrief(): ir.ModuleGlobalsDt {
    this._brief = ir.irGetModuleGlobalsBrief(this.moduleId);
    return this._brief;
  }

  loadAllGlobals(): void {
    for (const g of this.brief.globals) {
      this.loadGlobal(g.id);
    }
  }

  reloadGlobal(id: ir.GlobalID): ir.GlobalObjDt {
    const dt = ir.irLoadGlobalObj(this.moduleId, id);
    this._registerGlobal(dt);
    return dt;
  }

  getOwningFunc(id: ir.SourceTrackable): ir.GlobalID | null {
    return ir.irFuncScopeOfId(this.moduleId, id) ?? null;
  }

  findSourceLoc(id: ir.SourceTrackable): ir.SourceLoc | null {
    switch (id.type) {
      case "Global":
        return this.globals.get(id.value)?.overview_loc ?? null;
      case "FuncArg": {
        const [fid, idx] = id.value;
        const f = this.globals.get(fid);
        if (f?.typeid !== "Func") {
          return null;
        }
        return f.args[idx]?.source_loc ?? null;
      }
      case "Block":
        return this.blocks.get(id.value)?.source_loc ?? null;
      case "Inst":
        return this.insts.get(id.value)?.source_loc ?? null;
      case "Use":
        return this.uses.get(id.value)?.source_loc ?? null;
      case "JumpTarget":
        return this.jts.get(id.value)?.source_loc ?? null;
      default:
        return null;
    }
  }

  loadGlobal(id: ir.GlobalID): ir.GlobalObjDt {
    let dt = this.globals.get(id);
    if (!dt) {
      const brief = this.brief.globals.find((g) => g.id === id);
      if (!brief) {
        throw new Error(`Global with ID ${id} not found in module brief`);
      }
      dt = ir.irLoadGlobalObj(this.moduleId, id);
      this._registerGlobal(dt);
    }
    return dt;
  }
  loadFunc(id: ir.GlobalID): ir.FuncObjDt {
    const obj = this.loadGlobal(id);
    if (obj.typeid !== "Func") {
      throw new Error(`Global ${id} is not a function`);
    }
    return obj as ir.FuncObjDt;
  }
  private _loadLocal<I extends ir.PoolStrID, T>(
    id: I,
    map: Map<I, T>,
    name: string,
  ): T {
    let dt = map.get(id);
    if (!dt) {
      this._registerGlobal(ir.irLoadFuncOfScope(this.moduleId, id)!);
      dt = map.get(id);
      if (!dt)
        throw new Error(`${name} ${id} not found after loading its function`, {
          cause: { id, name },
        });
    }
    return dt;
  }
  loadBlock(id: ir.BlockID): ir.BlockDt {
    return this._loadLocal(id, this.blocks, "Block");
  }
  loadInst(id: ir.InstID): ir.InstDt {
    return this._loadLocal(id, this.insts, "Inst");
  }
  loadUse(id: ir.UseID): ir.UseDt {
    return this._loadLocal(id, this.uses, "Use");
  }
  loadJumpTarget(id: ir.JumpTargetID): ir.JumpTargetDt {
    return this._loadLocal(id, this.jts, "JumpTarget");
  }
  private _registerGlobal(dt: ir.GlobalObjDt) {
    this.globals.set(dt.id, dt);
    switch (dt.typeid) {
      case "Func":
        this._registerFunc(dt);
        break;
      case "GlobalVar":
        break;
    }
  }
  private _registerFunc(func: ir.FuncObjDt) {
    if (!func.blocks) return;
    for (const bb of func.blocks) {
      this._registerBlock(bb);
    }
  }
  private _registerBlock(bb: ir.BlockDt) {
    this.blocks.set(bb.id, bb);
    for (const inst of bb.insts) this._registerInst(inst);
  }
  private _registerInst(inst: ir.InstDt) {
    this.insts.set(inst.id, inst);
    for (const use of inst.operands) this.uses.set(use.id, use);
    if (inst.typeid === "Terminator") {
      for (const jt of inst.succs) this.jts.set(jt.id, jt);
    }
  }

  applySourceUpdates(
    updates: ir.SourceUpdates,
    maybeFunc: ir.GlobalID | null = null,
  ): void {
    if (updates.scope === "Func" && maybeFunc === null) {
      throw new Error("Func scope updates must provide the func id");
    }
    switch (updates.scope) {
      case "Module":
        this.brief.overview_src = updates.source;
        break;
      case "Func": {
        const func = this.loadGlobal(maybeFunc!) as ir.FuncObjDt;
        func.source = updates.source;
        break;
      }
    }

    const defaultRange: ir.SourceLoc = {
      begin: { line: 0, column: 0 },
      end: { line: 0, column: 0 },
    };

    // Handle eliminated items by removing them from caches or resetting locations
    for (const removed of updates.elliminated) {
      switch (removed.type) {
        case "Global":
          this.globals.delete(removed.value);
          break;
        case "Block":
          this.blocks.delete(removed.value);
          break;
        case "Inst":
          this.insts.delete(removed.value);
          break;
        case "Use":
          this.uses.delete(removed.value);
          break;
        case "JumpTarget":
          this.jts.delete(removed.value);
          break;
        case "FuncArg": {
          const [fid, idx] = removed.value;
          const f = this.globals.get(fid);
          if (f && f.typeid === "Func" && f.args && f.args[idx]) {
            f.args[idx].source_loc = defaultRange;
          }
          break;
        }
        case "Expr":
          break; // untracked, ignore
        default:
          throw new Error(`Unknown SourceTrackable type: ${removed.type}`);
      }
    }
    // Then apply location updates to existing items
    for (const r of updates.ranges) {
      const id = r.id;
      const new_loc = r.new_loc;
      switch (id.type) {
        case "Global": {
          const g = this.globals.get(id.value);
          if (g) g.overview_loc = new_loc;
          break;
        }
        case "FuncArg": {
          const [fid, idx] = id.value;
          const f = this.globals.get(fid);
          if (f && f.typeid === "Func" && f.args && f.args[idx])
            f.args[idx].source_loc = new_loc;
          break;
        }
        case "Block": {
          const bb = this.blocks.get(id.value);
          if (bb) bb.source_loc = new_loc;
          break;
        }
        case "Inst": {
          const inst = this.insts.get(id.value);
          if (inst) inst.source_loc = new_loc;
          break;
        }
        case "Use": {
          const u = this.uses.get(id.value);
          if (u) u.source_loc = new_loc;
          break;
        }
        case "JumpTarget": {
          const jt = this.jts.get(id.value);
          if (jt) jt.source_loc = new_loc;
          break;
        }
      }
    }
  }

  hasId(
    id: ir.GlobalID | ir.BlockID | ir.InstID | ir.JumpTargetID | ir.UseID,
  ): boolean {
    switch (id[0]) {
      case "g":
        return this.globals.has(id as ir.GlobalID);
      case "b":
        return this.blocks.has(id as ir.BlockID);
      case "i":
        return this.insts.has(id as ir.InstID);
      case "j":
        return this.jts.has(id as ir.JumpTargetID);
      case "u":
        return this.uses.has(id as ir.UseID);
      default:
        return false;
    }
  }

  getBlockSuccessors(block: ir.BlockDt): ir.JumpTargetDt[] {
    const insts = block.insts;
    const last = insts[insts.length - 1];
    if (last.typeid !== "Terminator") return [];
    return last.succs;
  }

  getValueOperands(value: ir.ValueDt): ir.UseDt[] {
    switch (value.type) {
      case "Inst": {
        const inst = this.loadInst(value.value);
        return inst.operands.map((u) => this.loadUse(u.id));
      }
      default:
        // TODO: add use support for other users
        return [];
    }
  }
  getValueUsers(value: ir.ValueDt): ir.UseDt[] {
    const users = ir.irValueGetUsedBy(this.moduleId, value);
    return users.map((u) => this.loadUse(u));
  }
  typeGetName(ty: ir.ValTypeID): string {
    return ir.irTypeGetName(this.moduleId, ty);
  }
  valueGetName(value: ir.ValueDt): string {
    switch (value.type) {
      case "None":
        return "None";
      case "I1":
        return `I1(${value.value})`;
      case "I8":
        return `I8(${value.value})`;
      case "I16":
        return `I16(${value.value})`;
      case "I32":
        return `I32(${value.value})`;
      case "I64":
        return `I64(${value.value})`;
      case "APInt": {
        const { bits, value: val } = value.value;
        return `I${bits}(${val})`;
      }
      case "F32":
        return `F32(${value.value})`;
      case "F64":
        return `F64(${value.value})`;
      case "Undef":
        return `Undef(type = ${this.typeGetName(value.value)})`;
      case "Block": {
        const block = this.loadBlock(value.value);
        return block.name ? `label %${block.name}` : `label ${value.value}`;
      }
      case "Expr": {
        return `Expr(${value.value})`;
      }
      case "Inst": {
        const inst = this.loadInst(value.value);
        return inst.name ? `%${inst.name}` : `${inst.opcode} ${value.value}`;
      }
      case "ZeroInit":
        return `ZeroInit(type = ${this.typeGetName(value.value)})`;
      case "FuncArg": {
        const [fid, idx] = value.value;
        const func = this.loadGlobal(fid);
        if (func.typeid !== "Func") {
          throw new Error(`Global ${fid} is not a function`);
        }
        return func.args[idx].name ?? `@${func.name} arg${idx}`;
      }
      case "Global": {
        const g = this.loadGlobal(value.value);
        return g.name ? `@${g.name}` : value.value;
      }
    }
  }

  makeBlockDfg(blockID: ir.BlockID): ir.BlockDfgDt {
    return ir.irMakeBlockDfg(this.moduleId, blockID);
  }
  makeCallGraph(): ir.CallGraphDt {
    return ir.irMakeCallGraph(this.moduleId);
  }
}

export type IRStoreStatus = "idle" | "ready" | "error";

export type IRStoreState = {
  module: ModuleCache | null;
  sourceKind: ir.SourceTy | null;
  sourceText: string;
  status: IRStoreStatus;
  error: string | null;
  revision: number;
  focusedId: ir.SourceTrackable | null;
  focusInfo: FocusSourceInfo | null;
  focusSince: number | null;
};

export type IRStoreActions = {
  compileModule: (kind: ir.SourceTy, source: string) => ModuleID | null;
  attachModule: (
    module: ModuleCache,
    kind: ir.SourceTy,
    source: string,
  ) => void;
  clear: () => void;
  refreshModuleSourceMappings: () => ir.SourceUpdates | null;
  refreshFuncSourceMappings: (funcId: ir.GlobalID) => ir.SourceUpdates | null;
  loadGlobal: (id: ir.GlobalID) => ir.GlobalObjDt | null;
  loadAllGlobals: () => void;
  getGlobal: (id: ir.GlobalID) => ir.GlobalObjDt | null;
  getBlock: (id: ir.BlockID) => ir.BlockDt | null;
  getInst: (id: ir.InstID) => ir.InstDt | null;
  getUse: (id: ir.UseID) => ir.UseDt | null;
  getJumpTarget: (id: ir.JumpTargetID) => ir.JumpTargetDt | null;
  getSourceLoc: (id: ir.SourceTrackable) => ir.SourceLoc | null;
  renameSymbol: (
    id: ir.SourceTrackable,
    newName: string,
  ) => ir.SourceUpdates | null;
  getActiveModuleId: () => ModuleID | null;
  focusOn: (id: ir.SourceTrackable) => void;
  clearFocus: () => void;
};

export type IRStore = IRStoreState & IRStoreActions;

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
// operate on draft, not IRState
function idleState(state: WritableDraft<IRStore>) {
  state.module = null;
  state.sourceKind = null;
  state.sourceText = "";
  state.status = "idle";
  state.error = null;
  state.focusedId = null;
  state.focusInfo = null;
  state.focusSince = null;
  state.revision += 1;
}

const DEFAULT_RANGE: ir.SourceLoc = {
  begin: { line: 0, column: 0 },
  end: { line: 0, column: 0 },
};

export const useIRStore = create<IRStore>()(
  devtools(
    immer((set, get) => ({
      module: null as null | ModuleCache,
      sourceKind: null as null | ir.SourceTy,
      sourceText: "",
      status: "idle",
      error: null,
      revision: 0,
      focusedId: { type: "Module" } as ir.SourceTrackable,
      focusInfo: null as FocusSourceInfo | null,
      focusSince: null as number | null,

      clear: () => {
        set(idleState, false, "ir/clear");
      },

      compileModule: (kind, source) => {
        try {
          const module = ModuleCache.compileFrom(kind, source);
          set(
            (state) => {
              state.module = module;
              state.sourceKind = kind;
              state.sourceText = source;
              state.status = "ready";
              state.error = null;
              state.focusedId = null;
              state.focusInfo = null;
              state.focusSince = null;
              state.revision += 1;
            },
            false,
            "ir/compile-module",
          );
          return module.moduleId;
        } catch (error) {
          alert(`Failed to compile module: ${normalizeError(error)}`);
          console.error(
            "Module compilation error:",
            error instanceof Error ? error.stack : error,
          );
          set(
            (state) => {
              state.module = null;
              state.sourceKind = kind;
              state.sourceText = source;
              state.status = "error";
              state.error = normalizeError(error);
              state.focusedId = null;
              state.focusInfo = null;
              state.focusSince = null;
              state.revision += 1;
            },
            false,
            "ir/compile-module/error",
          );
          return null;
        }
      },

      attachModule: (module, kind, source) => {
        set(
          (state) => {
            state.module = module;
            state.sourceKind = kind;
            state.sourceText = source;
            state.status = "ready";
            state.error = null;
            state.focusedId = null;
            state.focusInfo = null;
            state.focusSince = null;
            state.revision += 1;
          },
          false,
          "ir/attach-module",
        );
      },

      loadGlobal(id) {
        const module = get().module;
        if (!module) {
          return null;
        }
        const existed = module.globals.has(id);
        const dt = module.loadGlobal(id);
        if (!existed) {
          set(
            (state) => {
              state.revision += 1;
            },
            false,
            "ir/load-global",
          );
        }
        return dt;
      },

      loadAllGlobals() {
        const module = get().module;
        if (!module) {
          return;
        }
        module.loadAllGlobals();
        set(
          (state) => {
            state.revision += 1;
          },
          false,
          "ir/load-all-globals",
        );
      },

      refreshModuleSourceMappings() {
        const module = get().module;
        if (!module) {
          return null;
        }
        try {
          const updates = ir.irUpdateModuleOverviewSource(module.moduleId);
          set(
            (state) => {
              if (!state.module) {
                return;
              }
              state.module.applySourceUpdates(updates, null);
              state.module.refreshBrief();
              state.sourceText = updates.source;
              state.status = "ready";
              state.error = null;
              state.revision += 1;
            },
            false,
            "ir/refresh-module-source-mappings",
          );
          return updates;
        } catch (error) {
          set(
            (state) => {
              state.status = "error";
              state.error = normalizeError(error);
              state.revision += 1;
            },
            false,
            "ir/refresh-module-source-mappings/error",
          );
          return null;
        }
      },

      refreshFuncSourceMappings(funcId: ir.GlobalID): ir.SourceUpdates | null {
        const module = get().module;
        if (!module) {
          return null;
        }
        try {
          const updates = ir.irUpdateFuncSource(module.moduleId, funcId);
          set(
            (state) => {
              if (!state.module) {
                return;
              }
              state.module.applySourceUpdates(updates, funcId);
              state.module.reloadGlobal(funcId);
              state.sourceText = updates.source;
              state.status = "ready";
              state.error = null;
              state.revision += 1;
            },
            false,
            "ir/refresh-func-source-mappings",
          );
          return updates;
        } catch (error) {
          set(
            (state) => {
              state.status = "error";
              state.error = normalizeError(error);
              state.revision += 1;
            },
            false,
            "ir/refresh-func-source-mappings/error",
          );
          return null;
        }
      },

      focusOn(id: ir.SourceTrackable) {
        try {
          const info = focusSource(get() as IRStore, id);
          set(
            (state) => {
              if (!info) {
                state.focusedId = null;
                state.focusInfo = null;
                state.focusSince = null;
              } else {
                state.focusedId = id;
                state.focusInfo = info;
                state.focusSince = Date.now();
              }
              state.revision += 1;
            },
            false,
            "ir/focus-on",
          );
        } catch (error) {
          console.error("ir-state: focusOn error", error);
          set(
            (state) => {
              state.error = normalizeError(error);
              state.revision += 1;
            },
            false,
            "ir/focus-on/error",
          );
        }
      },

      clearFocus() {
        set(
          (state) => {
            state.focusedId = null;
            state.focusInfo = null;
            state.focusSince = null;
            state.revision += 1;
          },
          false,
          "ir/clear-focus",
        );
      },

      renameSymbol(id, newName) {
        throw new Error(
          `Renaming(${id} to ${newName}) not supported yet: waiting for WASM`,
        );
      },

      getGlobal: (id) => get().module?.globals.get(id) ?? null,
      getBlock: (id) => get().module?.blocks.get(id) ?? null,
      getInst: (id) => get().module?.insts.get(id) ?? null,
      getUse: (id) => get().module?.uses.get(id) ?? null,
      getJumpTarget: (id) => get().module?.jts.get(id) ?? null,

      getSourceLoc(id) {
        const module = get().module;
        if (!module) {
          return null;
        }
        return module.findSourceLoc(id) ?? DEFAULT_RANGE;
      },

      getActiveModuleId() {
        return get().module?.moduleId ?? null;
      },
    })),
    { name: "ir-store" },
  ),
);

export function selectIRModule(state: IRStore): ModuleCache | null {
  return state.module;
}
export function selectIRStatus(state: IRStore): IRStoreStatus {
  return state.status;
}
export function selectIRError(state: IRStore): string | null {
  return state.error;
}
export function selectIRRevision(state: IRStore): number {
  return state.revision;
}
export function selectIRBrief(state: IRStore): ir.ModuleGlobalsDt | null {
  return state.module?.brief ?? null;
}

export type FocusSourceInfo = {
  id: ir.SourceTrackable;
  scopeId: ir.GlobalID | null;
  sourceText: string;
  highlightLoc: ir.SourceLoc;
};
export function focusSource(
  state: IRStore,
  id: ir.SourceTrackable,
): FocusSourceInfo | null {
  try {
    console.debug("ir-state.focusSource: called with", id);
    const module = state.module;
    if (!module) {
      console.debug("ir-state.focusSource: no module");
      return null;
    }
    console.debug("ir-state.focusSource: moduleId=", module.moduleId);

    // Determine scope: if the id belongs to a function scope, use that function's GlobalID;
    // otherwise scopeId is null (module scope).
    let scopeId: ir.GlobalID | null = null;
    // Module-level focus sentinel
    switch (id.type) {
      case "Module":
        scopeId = null;
        break;
      case "Global": {
        const g = module.loadGlobal(id.value);
        if (g && g.typeid === "Func" && g.blocks) scopeId = id.value;
        break;
      }
      default: {
        // If we still don't have a scope, ask the module for the owning function (for blocks/insts/uses)
        const owning = module.getOwningFunc(id);
        if (owning) scopeId = owning;
        break;
      }
    }

    // Determine source text to show: module overview or the function source (if available)
    let sourceText: string = module.brief.overview_src;
    if (scopeId !== null) {
      const f = module.globals.get(scopeId) as ir.FuncObjDt | undefined;
      if (f && f.typeid === "Func" && typeof f.source === "string") {
        sourceText = f.source;
      }
    }

    // Determine highlight location. Prefer precise item source_loc; for focusing a function
    // itself, aggregate its blocks' ranges if possible.
    let highlightLoc: ir.SourceLoc | null = null;

    // If focusing a Global function and we have loaded func blocks, compute bounding range
    if ("Global" in id && scopeId !== null && id.Global === scopeId) {
      const f = module.globals.get(scopeId) as ir.FuncObjDt | undefined;
      if (f && f.typeid === "Func" && f.blocks && f.blocks.length > 0) {
        let begin = {
          line: Number.MAX_SAFE_INTEGER,
          column: Number.MAX_SAFE_INTEGER,
        };
        let end = { line: 0, column: 0 };
        for (const bb of f.blocks) {
          const loc = bb.source_loc;
          if (!loc) continue;
          if (
            loc.begin.line < begin.line ||
            (loc.begin.line === begin.line && loc.begin.column < begin.column)
          )
            begin = { ...loc.begin };
          if (
            loc.end.line > end.line ||
            (loc.end.line === end.line && loc.end.column > end.column)
          )
            end = { ...loc.end };
        }
        if (begin.line !== Number.MAX_SAFE_INTEGER) {
          highlightLoc = { begin, end };
        }
      }
    }

    // Fallback to item's recorded source location
    if (!highlightLoc) {
      const loc = module.findSourceLoc(id as ir.SourceTrackable);
      highlightLoc = loc ?? DEFAULT_RANGE;
    }

    const res: FocusSourceInfo = {
      id,
      scopeId,
      sourceText,
      highlightLoc,
    };
    return res;
  } catch (e) {
    console.warn("ir-state.focusSource: error", e);
    return null;
  }
}
