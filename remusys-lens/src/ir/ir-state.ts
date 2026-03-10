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
      let brief = this.brief.globals.find(g => g.id === id);
      if (!brief) {
        throw new Error(`Global with ID ${id} not found in module brief`);
      }
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

  getBlockSuccessors(block: ir.BlockDt): ir.JumpTargetDt[] {
    const insts = block.insts;
    const last = insts[insts.length - 1];
    if (last.typeid !== "Terminator")
      return [];
    return last.succs;
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
  focusedId: ir.SourceTrackable | { Module: true } | null;
  focusInfo: FocusSourceInfo | null;
  focusSince: number | null;
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
  focusOn: (id: ir.SourceTrackable | { Module: true }) => void;
  clearFocus: () => void;
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
      focusedId: null as ir.SourceTrackable | { Module: true } | null,
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
          console.error("Module compilation error:", (error instanceof Error ? error.stack : error));
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

      focusOn(id: ir.SourceTrackable | { Module: true }) {
        try {
          console.debug('ir-state: focusOn called with', id);
          const info = focusSource(get() as IRStore, id);
          console.debug('ir-state: focusSource returned', info);
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
          console.debug('ir-state: focus state set, new focusedId=', get().focusedId);
        } catch (error) {
          console.warn('ir-state: focusOn error', error);
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

export type FocusSourceInfo = {
  id: ir.SourceTrackable | { Module: true };
  scopeId: ir.GlobalID | null;
  sourceText: string;
  highlightLoc: ir.SourceLoc;
};
export function focusSource(state: IRStore, id: ir.SourceTrackable | { Module: true }): FocusSourceInfo | null {
  try {
    console.debug('ir-state.focusSource: called with', id);
    const module = state.module;
    if (!module) {
      console.debug('ir-state.focusSource: no module');
      return null;
    }
    console.debug('ir-state.focusSource: moduleId=', module.moduleId);

    // Determine scope: if the id belongs to a function scope, use that function's GlobalID;
    // otherwise scopeId is null (module scope).
    let scopeId: ir.GlobalID | null = null;
    // Module-level focus sentinel
    if ("Module" in id) {
      scopeId = null;
      console.debug('ir-state.focusSource: module-level focus');
    } else {
      // If id itself is a Global and it's a loaded function object, treat it as function scope.
      if ("Global" in id) {
        const gid = id.Global;
        const g = module.globals.get(gid);
        console.debug('ir-state.focusSource: Global id=', gid, 'loaded=', !!g);
        if (g && (g as any).typeid === "Func") {
          scopeId = gid;
          console.debug('ir-state.focusSource: scopeId set to global func', scopeId);
        }
      }

      // If we still don't have a scope, ask the module for the owning function (for blocks/insts/uses)
      if (scopeId === null) {
        const owning = module.getOwningFunc(id as ir.SourceTrackable);
        console.debug('ir-state.focusSource: owning func=', owning);
        if (owning) scopeId = owning;
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
    console.debug('ir-state.focusSource: chosen sourceText length=', sourceText?.length ?? 0, 'scopeId=', scopeId);

    // Determine highlight location. Prefer precise item source_loc; for focusing a function
    // itself, aggregate its blocks' ranges if possible.
    let highlightLoc: ir.SourceLoc | null = null;

    // If focusing a Global function and we have loaded func blocks, compute bounding range
    if (("Global" in id) && scopeId !== null && (id.Global === scopeId)) {
      const f = module.globals.get(scopeId) as ir.FuncObjDt | undefined;
      if (f && f.typeid === "Func" && f.blocks && f.blocks.length > 0) {
        let begin = { line: Number.MAX_SAFE_INTEGER, column: Number.MAX_SAFE_INTEGER };
        let end = { line: 0, column: 0 };
        for (const bb of f.blocks) {
          const loc = bb.source_loc;
          if (!loc) continue;
          if (loc.begin.line < begin.line || (loc.begin.line === begin.line && loc.begin.column < begin.column))
            begin = { ...loc.begin };
          if (loc.end.line > end.line || (loc.end.line === end.line && loc.end.column > end.column))
            end = { ...loc.end };
        }
        if (begin.line !== Number.MAX_SAFE_INTEGER) {
          highlightLoc = { begin, end };
          console.debug('ir-state.focusSource: function bounding highlightLoc=', highlightLoc);
        }
      }
    }

    // Fallback to item's recorded source location
    if (!highlightLoc) {
      const loc = module.findSourceLoc(id as ir.SourceTrackable);
      highlightLoc = loc ?? DEFAULT_RANGE;
      console.debug('ir-state.focusSource: fallback highlightLoc=', highlightLoc);
    }

    const res: FocusSourceInfo = {
      id,
      scopeId,
      sourceText,
      highlightLoc,
    };
    console.debug('ir-state.focusSource: returning', res);
    return res;
  } catch (e) {
    console.warn('ir-state.focusSource: error', e);
    return null;
  }
}