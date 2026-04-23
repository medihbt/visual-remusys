# b1 与 b2 总体设计对比分析

> 基于 `remusys-wasm` / `remusys-lens` (b1) 与 `remusys-wasm-b2` / `remusys-lens-b2` (b2) 的源码分析。

---

## 一、总体架构对比

### 1.1 项目定位

| 维度 | b1 | b2 |
|------|-----|-----|
| **WASM 后端** | 静态 API 桥接层：`Api` 结构体暴露无状态函数，模块缓存由 `thread_local HashMap` 管理 | 有状态对象层：`ModuleInfo` 作为 `#[wasm_bindgen]` 对象直接暴露给 JS，前端持有 WASM 对象句柄 |
| **前端** | 功能完整的可视化 IDE：编辑器 + 导航树 + 图视图均已可用 | 前端壳子：核心 UI（编辑器、图视图）为占位，GuideView 未接入 |
| **数据哲学** | 前端缓存 IR 实体（`ModuleCache` 管理 5 个 Map），惰性加载 | **WASM-first**：前端不缓存 IR 实体，所有树数据从 WASM 按需拉取，前端仅保留最小 UI 状态 |

### 1.2 技术栈变化

| 技术 | b1 | b2 | 说明 |
|------|-----|-----|------|
| React 构建插件 | `@vitejs/plugin-react-swc` | `@vitejs/plugin-react` | 从 SWC 换回 Babel |
| TypeScript | ~5.9 | ~6.0 | 升级大版本 |
| Vite | ~7.3 | ~8.0 | 升级大版本 |
| 其他核心依赖 | 基本一致（React 19, Zustand, Immer, xyflow, dagre, Monaco, headlessui, viz-js） | — | 无本质变化 |

### 1.3 模块结构对比

**WASM 后端 (Rust)**

```
b1 (remusys-wasm)                  b2 (remusys-wasm-b2)
├── lib.rs        (Api 静态方法)    ├── lib.rs        (宏工具)
├── dto.rs        (统一 DTO)        ├── types.rs      (TS 类型桥接)
├── mapping.rs    (SourceLoc 转换)   ├── dto.rs + dto/ (按图拆分 DTO)
├── module.rs     (ModuleInfo)      ├── tree.rs + tree/ (IRTree + Builder + Expand)
├── module/                              ├── module.rs + module/ (核心 API)
│   ├── source_buf.rs                    │   ├── rename.rs
│   ├── source_tree.rs  (实验性)         │   ├── source_buf.rs
│   └── source_tree_builder.rs (未接入)  │   ├── name_revmap.rs  (空)
├── rename.rs     (重命名引擎)           │   └── secondary.rs    (空)
└── graphs/       (图分析 DTO)      └── (graphs 并入 dto/)
    ├── call_graph.rs
    ├── cfg.rs
    └── dfg.rs
```

**前端 (TypeScript)**

```
b1 (remusys-lens)                  b2 (remusys-lens-b2)
├── main.tsx                        ├── main.tsx
├── App.tsx                         ├── App.tsx
├── TopMenu.tsx                     ├── AppMenu.tsx
├── FileLoader.tsx                  ├── file-load.tsx
├── file-load.tsx                   ├── ir/
├── ir/                                 └── state.ts
│   ├── ir.ts                       ├── guide_view/
│   └── ir-state.ts                     ├── GuideView.tsx   (空壳占位)
├── editor/                             ├── Node.tsx        (完整)
│   ├── LensViewer.tsx                  ├── ChildRow.tsx
│   └── llvmMonarch.ts                  ├── NodeMenu.tsx    (未接入)
├── guide-view/                         ├── TypeIcon.tsx
│   ├── GuideView.tsx   (完整)          └── guide-view-tree.ts (完整)
│   ├── guide-view-tree.ts
│   ├── types.ts
│   ├── GuideContext.tsx (未使用)
│   └── components/
│       ├── GuideNodeComp.tsx
│       ├── ChildRow.tsx
│       ├── SimpleMenu.tsx
│       └── TypeIcon.tsx
└── flow/                           └── (flow/ 完全缺失)
    ├── FlowViewer.tsx
    ├── flow-stat.ts
    ├── graphviz-object.ts
    ├── components/
    │   ├── Node.tsx
    │   ├── Edge.tsx
    │   └── Toast.tsx
    └── graphs/
        ├── layout.ts
        ├── cfg.ts
        ├── dominance.ts
        ├── dfg.ts
        └── callgraph.ts
```

---

## 二、详细设计差异

### 2.1 WASM 后端核心设计差异

#### A. API 暴露模式（最本质差异）

| 特性 | b1 | b2 |
|------|-----|-----|
| **暴露方式** | `pub struct Api;` + 静态方法 `#[wasm_bindgen]` | `#[wasm_bindgen] pub struct ModuleInfo` 实例方法 |
| **模块标识** | 字符串 ID `"module_N"`，通过 `thread_local MODULES: HashMap` 查找 | `ModuleInfo` 自身就是句柄，含原子递增的 `id: usize` |
| **状态持有** | `thread_local! { HashMap<SmolStr, ModuleInfo> }` | `ModuleInfo` 自身持有 `module`, `ir_tree`, `source`, `names` |
| **序列化** | `serialize_to_js<T>()` 全局辅助函数 | `ModuleInfo::serialize<T>()` + `deserialize<T>()` 实例方法 |
| **TS 类型契约** | 无显式契约，靠 serde 隐式生成 | `api/types.ts` + `types.rs` 显式注入 30+ 个 TypeScript 类型 |

#### B. IR ↔ 源码映射（b2 的核心重构）

| 特性 | b1 | b2 |
|------|-----|-----|
| **序列化器** | 复用 `remusys_ir::ir::IRSerializer` / `FuncSerializer`（旧路径） | **独立重写** `IRDagBuilder`（`tree/builder.rs`），同步构建 `IRTree` 和 `SourceBuf` |
| **源码缓冲区** | `IRSourceBuf`（基于 `SmallVec<[u8; 16]>` 的行缓冲，实验性） | `SourceBuf`（基于 `SmallVec<[u8; 32]>` 的行缓冲，**主路径**） |
| **树结构** | `IRTree` / `IRTreeNode`（实验性，**未接入主 API**） | `IRTree` / `IRTreeNode`（**核心主路径**），使用相对位置 `pos_delta` + `mtb-entity-slab` 管理 ID |
| **DAG 复用** | 无 | `tree_map: HashMap<UseID, IRTreeNodeID>` 复用共享表达式结点 |
| **双向查询** | 单向：IR 对象 → SourceLoc（通过 `SourceRangeMap` + `StrLines`） | **双向**：源码位置 → `IRObjPath`（`path_of_srcpos`）；`IRTreeObjID` → 路径（`path_of_tree_object`） |
| **光标导航** | 无 | `IRTreeCursor`：带路径栈的游标，支持 `goto_parent/child`，所有权校验 |

#### C. 可展开引导树（b2 新增核心概念）

b1 中没有 `IRExpandTree` 概念。GuideView 的展开状态完全由前端 `TreeNodeStorage` 管理。

b2 在 WASM 层新增了 `IRExpandTree`（`tree/expand.rs`）：
- 维护 `expand_children: HashMap<IRTreeObjID, Node>` 记录展开状态。
- `load_tree(module, focus_path)` 与真实 `IRTree` 取**交集**：若旧展开对象在新 IR 中已不存在，则丢弃该分支。
- 支持焦点路径标记（`FocusNode` / `FocusScope` / `FocusParent`）。

#### D. 重命名与更新策略

| 特性 | b1 | b2 |
|------|-----|-----|
| **重命名 API** | `Api::rename(module_id, poolid, new_name) -> RenameDelta` | `ModuleInfo::rename(path, new_name) -> RenameRes` |
| **失效传播** | 返回 `RenameDelta { invalidated: Vec<GlobalID> }`，前端需重新请求这些函数的源码 | **全量重建** `IRTree` + `SourceBuf`（`full_update`），注释明确为权宜之计 |
| **名称查重** | `is_name_used_in_function` 线性扫描 | `rev_local_names: HashMap<FuncID, RevLocalNameMap>` 反向表（`name_revmap.rs` 为空，实际定义在 `module.rs`） |
| **增量更新** | 概念上支持（`SourceUpdates` + `apply_line_update`），但实际未用于编辑 | **缺失**：明确标注为后续优化目标 |

#### E. 图分析 DTO

| 图类型 | b1 | b2 |
|--------|-----|-----|
| CFG | `FuncCfgDt`（`Entry/Control/Exit/Unreachable` 节点角色） | `FuncCfgDt`（`Entry/Branch/Exit` 角色，简化） |
| 支配树 | `DomTreeDt`（仅前支配，后支配拒绝） | `DomTreeDt`（相同） |
| DFG | `BlockDfgDt`（`Income/Pure/Effect/Outcome` Section，跨块输入去重） | `BlockDfg`（`Income/Outgo/Phi/Pure/Effect/Terminator` 角色，**b2 规则**：仅 `Inst` 和 `FuncArg` 放入 Income） |
| 调用图 | `CallGraphDt`（`Root/Live/Unreachable/Indirect` 角色） | `CallGraphDt`（`Public/Private/Extern` 角色，按 Linkage 分类） |
| DOT 导出 | 无 | `BlockDfg::to_dot_text()` 直接生成 Graphviz DOT |

### 2.2 前端核心设计差异

#### A. 状态管理

**b1（前端缓存实体）**
```typescript
// ir-state.ts
class ModuleCache {
  globals: Map<GlobalID, GlobalObjDt>
  blocks:  Map<BlockID, BlockDt>
  insts:   Map<InstID, InstDt>
  uses:    Map<UseID, UseDt>
  jts:     Map<JumpTargetID, JumpTargetDt>
  // 按需加载 + 本地缓存
}

IRStore {
  module: ModuleCache | null
  sourceKind, sourceText
  focusedId, focusInfo
  revision  // 手动递增触发重渲染
}

FlowStore {
  graphType: FlowGraphType
}
```

**b2（WASM-first，前端零缓存）**
```typescript
// ir/state.ts
IRStore {
  module?: ModuleInfo   // WASM 句柄
  source: string        // 模块源码文本
  focus: IRObjPath      // 焦点路径（全局唯一真相源）
}

GuideViewTreeStore {
  moduleId?: number
  expandTree?: IRExpandTree  // WASM 句柄
  treeEpoch: number
  root?: GuideNodeExpand     // 可见树根
}
```

#### B. GuideView 导航树

| 特性 | b1 | b2 |
|------|-----|-----|
| **展开状态** | 前端 `TreeNodeStorage.expand(id, module)` 按需从 `ModuleCache` 取数据 | WASM `IRExpandTree` 管理展开状态，`load_tree` 返回可见切片 |
| **树构建** | 前端 `GuideTreeNode` + `export(module)` DFS 导出为 React Flow 节点 | 前端 `connectGuideTree` 补全 `parent` 指针，`collectGuideTree` 递归展开 |
| **焦点处理** | `IRStore.focusOn(id)` 计算 `scopeId` + `sourceText` + `highlightLoc` | `reconcileFocusPath` 用 `IRTreeCursor` 验证路径有效性，失效则回退到最近祖先 |
| **布局引擎** | dagre `graphlib.Graph` LR 布局 | dagre `graphlib.Graph` LR 布局（相同） |
| **菜单** | `SimpleMenu.tsx` 已接入 `GuideNodeComp` | `NodeMenu.tsx` 完整实现但**未被集成**到 `Node.tsx` |

#### C. 编辑器

| 特性 | b1 | b2 |
|------|-----|-----|
| **Monaco 接入** | ✅ `LensViewer.tsx` 完整实现 | ❌ **缺失**：`PanePlaceholder("源码编辑器")` |
| **语法高亮** | ✅ LLVM IR Monarch + C 语言回退 | ❌ **缺失** |
| **Focus 高亮** | ✅ `deltaDecorations` + `ir-focus-decoration` | ❌ **缺失** |
| **源码文本来源** | `focusInfo.sourceText` / `moduleOverview` | `IRStore.source` + `getFocusSrcRange()`（未消费） |

#### D. 图视图（Flow）

| 特性 | b1 | b2 |
|------|-----|-----|
| **FlowViewer** | ✅ 完整实现，支持 5 种图类型 | ❌ **缺失**：`PanePlaceholder("图视图")` |
| **布局引擎** | ✅ Graphviz `dot`（`@viz-js/viz`）+ 坐标系翻转 + B-spline → Bézier | ❌ **缺失**：依赖已声明但未使用 |
| **分组布局** | ✅ `layoutSectionFlow`（DFG 按 Section 分组） | ❌ **缺失** |
| **CFG 边样式** | ✅ 按 DFS 分类（Tree/Back/Forward/Cross）设置虚线 | ❌ **缺失** |
| **DefUse 图** | ✅ 以 Value 为中心的局部子图 | ❌ **缺失** |

---

## 三、缺失/未完成部分汇总

### 3.1 b1 缺失（但 b2 有规划或已改进）

| 缺失项 | b1 状态 | b2 状态 |
|--------|---------|---------|
| IR ↔ 源码双向映射 | 仅单向；`source_tree` 实验性未接入 | ✅ `IRTree` + `IRTreeCursor` 完整实现 |
| 源码位置 → IR 对象查询 | ❌ 无 | ✅ `path_of_srcpos` |
| 可展开引导树状态管理 | 前端自行管理 | ✅ `IRExpandTree` 在 WASM 层管理 |
| 保存功能 | `alert("等待实现")` | `alert("等待实现")`（相同） |
| 重命名前端对接 | `renameSymbol` 抛 Error | `rename` 未在前端调用（但 WASM 已支持） |
| ItemReference 图 | `todoNodes("ItemReference")` | —（图视图整体缺失） |

### 3.2 b2 缺失（b1 已完成）

| 缺失项 | b1 状态 | b2 状态 |
|--------|---------|---------|
| **源码编辑器** | ✅ Monaco 完整可用 | ❌ **占位** |
| **图视图（Flow）** | ✅ CFG/DFG/CallGraph/支配树/DefUse 完整 | ❌ **占位** |
| **GuideView 接入** | ✅ 完整可用 | ❌ `GuideView.tsx` 是空壳，未接入 `Node.tsx` |
| LLVM IR Monarch 高亮 | ✅ 已注册 | ❌ 未接入 |
| 右键/浮动菜单 | ✅ `SimpleMenu` 已接入 | `NodeMenu` 实现完整但**未接线** |
| `FlowStore` / 图状态管理 | ✅ `useFlowStore` | ❌ 无 |
| `NavEvent` 协议 | ✅ 连接 GuideView ↔ FlowViewer | ❌ 无 |

### 3.3 两者共有的缺失

1. **增量源码更新**：重命名后 b1 返回失效列表，b2 全量重建，均未实现真正的增量更新。
2. **IR 编辑操作**：均无指令/基本块/全局变量的增删改 API。
3. **后支配树**：均不支持。
4. **跨基本块 DFG**：均只支持单块内 DFG。
5. **保存/导出**：前端均为 stub。
6. **Undo/Redo**：均无历史栈。
7. **模块释放**：b1 `MODULES` 只增不减；b2 旧 `ModuleInfo` 被覆盖时未显式 `free()`。

---

## 四、b1 与 b2 的差异大不大？

**结论：差异非常大，b2 不是 b1 的渐进式改进，而是一次架构层面的重写。**

### 4.1 差异巨大的核心证据

1. **WASM API 范式完全不同**
   - b1 是**无状态静态函数**风格（类似 C 语言 FFI）：`Api.compile_module(...) -> ModuleBrief`，后续所有操作传模块 ID 字符串。
   - b2 是**面向对象句柄**风格（类似 JS 类绑定）：`ModuleInfo.compile_from(...) -> ModuleInfo`，前端直接持有 WASM 对象，调用实例方法。
   - 这导致前后端的交互契约发生了根本性变化。

2. **数据所有权彻底翻转**
   - b1 前端是**有状态缓存层**：`ModuleCache` 持有完整 IR 实体副本，WASM 侧仅提供一次性序列化数据。
   - b2 前端是**无状态代理层**：`IRStore` 仅保留 `focus` 路径和 `source` 文本，所有 IR 树遍历、展开状态、对象查询都委托给 WASM。
   - 这是从 "Fat Client + Thin WASM" 到 **"Thin Client + Fat WASM"** 的范式迁移。

3. **源码映射体系完全重构**
   - b1 基于 `IRSerializer` 的 `SourceRangeMap` + `StrLines` 坐标转换，是**附加式**映射。
   - b2 基于 `IRDagBuilder` 手写序列化 + `IRTree` 相对位置树，是**内生式**映射，且支持双向查询（源码位置 ↔ IR 路径）。
   - b1 的 `source_tree_builder.rs` 虽然写了一套类似逻辑，但**完全没有接入**；b2 的 `tree/` 是主路径。

4. **前端完成度倒挂**
   - b1 前端是**功能完整的 IDE**：编辑器 + 导航树 + 图视图均可正常工作。
   - b2 前端是**高度残缺的壳子**：除了 `guide-view-tree.ts` 的状态逻辑和 `Node.tsx` 的渲染逻辑较完整外，编辑器、图视图、GuideView 主组件均为占位或未接入。
   - 换言之，b2 的 WASM 后端设计更先进，但前端完成度远低于 b1。

### 4.2 差异较小的部分

- **编译管道**：两者都走 `remusys-lang`（SysY）和 `remusys-ir-parser`（IR 文本）。
- **图分析算法**：CFG 边分类、支配树、DFG Section 划分、调用图 DFS 等核心逻辑基本复用 `remusys-ir` 的能力，DTO 形状相似。
- **UI 技术栈**：React 19、Vite、Zustand、Immer、xyflow、dagre、Monaco、headlessui 均未更换。
- **布局理念**：三栏分栏（`react-reflex`）、GuideView 的 dagre LR 布局、节点卡片式设计保持一致。

### 4.3 总结评价

b2 代表了作者对架构的**深刻反思和重新设计**：
- **解决了 b1 的核心痛点**：前端缓存与 WASM 状态不同步、IR 对象缺乏双向定位能力、SourceMap 依赖旧序列化器。
- **引入了新的复杂度**：`IRTree` 的相对位置维护、`IRExpandTree` 的交集语义、`IRTreeCursor` 的所有权校验、WASM 对象的显式内存管理。
- **代价是前端大量功能回退**：b2 目前无法作为可用产品运行，其前端相当于只实现了 b1 中 `guide-view-tree.ts` 的状态机逻辑，但完全没有接入 UI。

如果 b1 的完成度是 **70%**（可用但粗糙），b2 的完成度大约是 **35%**（WASM 后端设计完成约 80%，前端完成约 10%）。b2 的 WASM 后端设计明显更干净、更面向未来，但要达到 b1 的可用水平，还需要大量的前端回填工作。
