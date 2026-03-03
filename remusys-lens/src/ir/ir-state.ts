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
    let moduleBrief = ir.irCompileModule(srcKind, src);
    let cache = new ModuleCache(moduleBrief.id);
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
    if ("Global" in id) {
      const g = this.globals.get(id.Global);
      return g?.overview_loc ?? null;
    }
    if ("FuncArg" in id) {
      const [fid, idx] = id.FuncArg;
      const f = this.globals.get(fid);
      if (f?.typeid !== "Func") {
        return null;
      }
      return f.args[idx]?.source_loc ?? null;
    }
    if ("Block" in id)
      return this.blocks.get(id.Block)?.source_loc ?? null;
    if ("Inst" in id)
      return this.insts.get(id.Inst)?.source_loc ?? null;
    if ("Use" in id)
      return this.uses.get(id.Use)?.source_loc ?? null;
    if ("JumpTarget" in id)
      return this.jts.get(id.JumpTarget)?.source_loc ?? null;
    return null;
  }

  loadGlobal(id: ir.GlobalID): ir.GlobalObjDt {
    let dt = this.globals.get(id);
    if (!dt) {
      dt = ir.irLoadGlobalObj(this.moduleId, id);
      this._registerGlobal(dt);
    }
    return dt;
  }
  loadFunc(id: ir.GlobalID): ir.FuncObjDt {
    let obj = this.loadGlobal(id);
    if (obj.typeid !== "Func") {
      throw new Error(`Global ${id} is not a function`);
    }
    return obj as ir.FuncObjDt;
  }
  private _loadLocal<I extends ir.PoolStrID, T>(id: I, map: Map<I, T>, name: string): T {
    let dt = map.get(id);
    if (!dt) {
      this._registerGlobal(ir.irLoadFuncOfScope(this.moduleId, id)!);
      dt = map.get(id);
      if (!dt)
        throw new Error(`${name} ${id} not found after loading its function`, { cause: { id, name } });
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
    if (!func.blocks)
      return;
    for (let bb of func.blocks) {
      this._registerBlock(bb);
    }
  }
  private _registerBlock(bb: ir.BlockDt) {
    this.blocks.set(bb.id, bb);
    for (let inst of bb.insts)
      this._registerInst(inst);
  }
  private _registerInst(inst: ir.InstDt) {
    this.insts.set(inst.id, inst);
    for (let use of inst.operands)
      this.uses.set(use.id, use);
    if (inst.typeid === "Terminator") {
      for (let jt of inst.succs)
        this.jts.set(jt.id, jt);
    }
  }

  applySourceUpdates(updates: ir.SourceUpdates, maybeFunc: ir.GlobalID | null = null): void {
    if (updates.scope === "Func" && maybeFunc === null) {
      throw new Error("Func scope updates must provide the func id");
    }
    switch (updates.scope) {
      case "Module":
        this.brief.overview_src = updates.source;
        break;
      case "Func": {
        let func = this.loadGlobal(maybeFunc!) as ir.FuncObjDt;
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
      if ("Global" in removed) {
        this.globals.delete(removed.Global);
      } else if ("Block" in removed) {
        const bb = this.blocks.get(removed.Block);
        if (bb) {
          for (const inst of bb.insts) {
            for (const u of inst.operands) this.uses.delete(u.id);
            if (inst.typeid === "Terminator") {
              for (const jt of inst.succs) this.jts.delete(jt.id);
            }
            this.insts.delete(inst.id);
          }
        }
        this.blocks.delete(removed.Block);
      } else if ("Inst" in removed) {
        const inst = this.insts.get(removed.Inst);
        if (inst) {
          for (const u of inst.operands) this.uses.delete(u.id);
          if (inst.typeid === "Terminator") {
            for (const jt of inst.succs) this.jts.delete(jt.id);
          }
        }
        this.insts.delete(removed.Inst);
      } else if ("Use" in removed) {
        this.uses.delete(removed.Use);
      } else if ("JumpTarget" in removed) {
        this.jts.delete(removed.JumpTarget);
      } else if ("FuncArg" in removed) {
        const [fid, idx] = removed.FuncArg;
        const f = this.globals.get(fid) as ir.FuncObjDt | undefined;
        if (f && f.typeid === "Func" && f.args && f.args[idx]) {
          f.args[idx].source_loc = defaultRange;
        }
      }
      // Expr and other untracked kinds are ignored
    }
    // Then apply location updates to existing items
    for (const r of updates.ranges) {
      const id = r.id;
      const new_loc = r.new_loc;
      if ("Global" in id) {
        const g = this.globals.get(id.Global);
        if (g) g.overview_loc = new_loc;
      } else if ("FuncArg" in id) {
        const [fid, idx] = id.FuncArg;
        const f = this.globals.get(fid) as ir.FuncObjDt | undefined;
        if (f && f.typeid === "Func" && f.args && f.args[idx]) {
          f.args[idx].source_loc = new_loc;
        }
      } else if ("Block" in id) {
        const bb = this.blocks.get(id.Block);
        if (bb) bb.source_loc = new_loc;
      } else if ("Inst" in id) {
        const inst = this.insts.get(id.Inst);
        if (inst) inst.source_loc = new_loc;
      } else if ("Use" in id) {
        const u = this.uses.get(id.Use);
        if (u) u.source_loc = new_loc;
      } else if ("JumpTarget" in id) {
        const jt = this.jts.get(id.JumpTarget);
        if (jt) jt.source_loc = new_loc;
      }
    }
  }

  hasId(id: ir.GlobalID | ir.BlockID | ir.InstID | ir.JumpTargetID | ir.UseID): boolean {
    let startWith = id[0];
    switch (startWith) {
      case "g": return this.globals.has(id as ir.GlobalID);
      case "b": return this.blocks.has(id as ir.BlockID);
      case "i": return this.insts.has(id as ir.InstID);
      case "j": return this.jts.has(id as ir.JumpTargetID);
      case "u": return this.uses.has(id as ir.UseID);
      default: return false;
    }
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
};

export type IRStoreActions = {
  compileModule: (kind: ir.SourceTy, source: string) => ModuleID | null;
  attachModule: (module: ModuleCache, kind: ir.SourceTy, source: string) => void;
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
  renameSymbol: (id: ir.SourceTrackable, newName: string) => ir.SourceUpdates | null;
  getActiveModuleId: () => ModuleID | null;
};

export type IRStore = IRStoreState & IRStoreActions;

function normalizeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
// operate on draft, not IRState
function idleState(state: any) {
  state.module = null;
  state.sourceKind = null;
  state.sourceText = "";
  state.status = "idle";
  state.error = null;
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
              state.revision += 1;
            },
            false,
            "ir/compile-module",
          );
          return module.moduleId;
        } catch (error) {
          set(
            (state) => {
              state.module = null;
              state.sourceKind = kind;
              state.sourceText = source;
              state.status = "error";
              state.error = normalizeError(error);
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

      renameSymbol(id, newName) {
        throw new Error(`Renaming(${id} to ${newName}) not supported yet: waiting for WASM`)
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
