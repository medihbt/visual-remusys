import { IRTreeCursor, ModuleInfo } from "remusys-wasm";
import type {
  IRTreeNodeDt,
  IRObjPath,
  MonacoSrcRange,
  SourceTy,
  RenameRes,
} from "remusys-wasm";

import { create } from "zustand";
import { devtools } from "zustand/middleware";

export interface IRStorage {
  module?: ModuleInfo;
  source: string;
  /**
   * 焦点是系统中最核心的概念，代表用户当前关注的程序元素。它由一系列 IR 对象引用组成，
   * 类似于文件系统中的路径——从模块到函数到基本块到具体指令，每一层都定位一个更小的范围。
   *
   * 当用户在任何视图中通过双击、菜单操作等聚焦一个元素时，系统计算出该元素对应的焦点路径，
   * 更新全局焦点状态。三个视图各自监测到焦点变化后，用自己擅长的方式做出响应：
   * 源码视图滚动到对应行并高亮相关代码，树视图展开焦点所在的路径，图视图自动切换到与焦点匹配的图类型。
   *
   * 在重命名等操作后，某些中间表示对象可能发生变化，旧的焦点路径可能不再完整有效。此时系统
   * 会沿路径逐层检查，保留仍然有效的部分，将焦点回退到最近的可用范围，避免发生错误。
   * 
   * 默认的根焦点是 `[{ type: "Module" }]`，表示整个模块。当用户没有明确聚焦任何元素时，
   * 系统保持在这个默认状态。
   */
  focus: IRObjPath;
}

export interface IRActions {
  compile: (src_kind: SourceTy, src: string, filename?: string) => ModuleInfo;
  getFocusSrcRange: () => MonacoSrcRange;
  getModule: () => ModuleInfo;
  setFocus: (path: IRObjPath) => void;
  clearFocus: () => void;

  /**
   * 重命名一个 IR 对象（函数、基本块、指令等）. JS 侧需要废弃所有缓存, 重新构建 IRDag 和相关数据结构.
   */
  rename: (object_id: IRObjPath, new_name: string) => RenameRes;
}

export type IRState = IRStorage & IRActions;

export const useIRStore = create<IRState>()(
  devtools((set, get) => ({
    module: undefined,
    source: "",
    focus: [{ type: "Module" }],
    compile(src_kind, src, filename) {
      const module_name = filename ?? "input";
      const module = ModuleInfo.compile_from(src_kind, src, module_name);
      set({
        module,
        source: module.dump_source(),
        focus: [{ type: "Module" }],
      });
      return module;
    },
    getFocusSrcRange(): MonacoSrcRange {
      const { module, focus } = get();
      if (!module) {
        throw new Error("module not loaded");
      }
      const node: IRTreeNodeDt = module.path_get_node(focus);
      return node.src_range;
    },
    getModule(): ModuleInfo {
      const { module } = get();
      if (!module) {
        throw new Error("module not loaded");
      }
      return module;
    },
    setFocus(path) {
      const old = get().focus;
      if (isSamePath(old, path)) return;
      set({ focus: path });
    },
    clearFocus() {
      set({ focus: [{ type: "Module" }] });
    },

    rename(object_id, new_name) {
      const { module } = get();
      if (!module) {
        throw new Error("module not loaded");
      }
      const res = module.rename(object_id, new_name);
      if (res.type !== "Renamed") return res;

      const focus = get().focus;
      const newFocus = sliceValidObjPath(module, get().focus);
      const source = module.dump_source();
      if (isSamePath(newFocus, focus)) {
        set({ source });
      } else {
        set({ focus: newFocus, source });
      }
      return res;
    },
  })),
);

/**
 * 从一个 IR 对象路径中切出一个有效的路径. 例如, 如果当前 IR 中没有 `@main` 这个函数, 那么路径 `[@main, %bb1]` 就是无效的.
 *
 * @param module 当前的 IR 模块.
 * @param path 传入的 IR 对象路径.
 * @returns 如果 path 是有效的, 则返回原路径; 否则返回一个有效的子路径.
 */
export function sliceValidObjPath(
  module: ModuleInfo,
  path: IRObjPath,
): IRObjPath {
  const length = path.length;
  const cursor = new IRTreeCursor(module);
  let cnt = 1;
  try {
    while (cnt < length && cursor.has_child(module, path[cnt])) {
      cursor.goto_child(module, path[cnt]);
      cnt++;
    }
  } finally {
    cursor.free();
  }
  if (cnt === length) {
    return path;
  } else {
    return path.slice(0, cnt);
  }
}

export function useIRFocusSrcRange(): MonacoSrcRange {
  return useIRStore((s) => s.getFocusSrcRange());
}
export function isSamePath(a: IRObjPath, b: IRObjPath): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const ai = a[i],
      bi = b[i];
    if (ai.type !== bi.type) return false;
    if (ai.type === "Module" || bi.type === "Module") continue;
    if (ai.type === "FuncArg" && bi.type === "FuncArg") {
      const [afunc, aindex] = ai.value;
      const [bfunc, bindex] = bi.value;
      if (afunc !== bfunc || aindex !== bindex) return false;
      continue;
    }
    if (ai.value !== bi.value) return false;
  }
  return true;
}
