# remusys-wasm (b1) 设计分析

> 分析范围：`remusys-wasm/` 目录下全部 Rust 源代码（含 `src/` 及其子目录）。
> 分析日期：2026-04-21
> 版本依据：Git branch `feature/rename`，commit `1f3da1d`

---

## 总体设计

### 项目定位

`remusys-wasm` 是 **Visual Remusys** 的 **WASM 后端桥梁层（b1）**。它将底层 IR 核心库（`remusys-ir`、`remusys-lang`、`remusys-ir-parser`）的能力封装为 **WASM-bindgen 暴露的 JS API**，供前端（`remusys-lens`/`remusys-lens-b2`）调用。其核心职责可概括为：

1. **编译与加载**：接收 SysY 源码或 Remusys-IR 文本，编译成内存中的 `Module`，并分配唯一 ID 进行生命周期管理。
2. **序列化与 Source Mapping**：将 IR 对象（全局对象、函数、基本块、指令等）序列化为可展示的文本，同时生成精确的源码位置映射（SourceLoc），供前端实现代码高亮、跳转、选中。
3. **交互式重命名**：支持对全局对象、基本块、指令、函数参数、Use 等进行重命名，并返回失效范围（Delta）供前端刷新缓存。
4. **图分析与可视化**：生成控制流图（CFG）、支配树（Dominator Tree）、基本块级数据流图（DFG）、模块级调用图（Call Graph）。
5. **IR 变换**：提供函数克隆（Clone）等基础变换能力。

### 项目架构与模块划分

```
remusys-wasm
├── Cargo.toml              # WASM 编译配置 (cdylib + rlib), 依赖 remusys-ir/lang/parser
├── ir-internal.md          # 简要的 JS-Rust 数据结构对接文档
└── src
    ├── lib.rs              # WASM 入口: Api 结构体 + #[wasm_bindgen] 暴露的全部 JS API
    ├── dto.rs              # Data Transfer Objects: Rust → JS 的序列化数据结构
    ├── mapping.rs          # 源码位置映射工具: StrLines (IRSourceRange → SourceLoc)
    ├── module.rs           # 核心模块管理: ModuleInfo (编译、缓存、序列化、查询)
    ├── module/
    │   ├── source_buf.rs   # IR 源码文本缓冲区 (行式管理、增量更新)
    │   ├── source_tree.rs  # IR 源码树 (IRTree): 结点池 + 父子关系 + 源码跨度
    │   └── source_tree_builder.rs  # IR 源码树构建器: 独立的 IR 序列化实现
    ├── rename.rs           # 重命名引擎: 名称查重、作用域失效、RenameDelta
    └── graphs/
        ├── call_graph.rs   # 调用图构建 (直接调用, Root/Live/Unreachable 分类)
        ├── cfg.rs          # 函数级 CFG + 支配树 (边分类: Tree/Back/Forward/Cross)
        └── dfg.rs          # 基本块级 DFG (按 Pure/Effect 分 Section)
```

### 核心职责与数据流

#### 1. 模块生命周期

```
JS 前端
   │ compile_module(source_ty, source)
   ▼
[Api::compile_module]
   │ 分支:
   ├─ "sysy" → remusys_lang::translate_sysy_text_into_full_ir(source)
   └─ "ir"  → remusys_ir_parser::source_to_full_ir(source)
   ▼
ModuleInfo { module: Box<Module>, names: IRNameMap, overview: RefCell<None> }
   │ 存入 thread_local MODULES HashMap
   ▼
返回 ModuleBrief { id: "module_N" }
```

- **全局模块缓存**：使用 `thread_local! { HashMap<SmolStr, ModuleInfo> }` 实现单线程 WASM 环境下的模块会话管理。`ModuleInfo` 持有 `Box<Module>` 和 `IRNameMap`。
- **延迟初始化**：`OverviewInfo`（模块概览文本及其 source map）在首次请求时通过 `IRSerializer` 生成并缓存，重命名等修改操作会将其失效（`invalidate_overview`）。

#### 2. 对象加载与 Source Map 数据流

```
JS 请求 load_global_obj(global_id)
   ▼
ModuleInfo::make_global_obj
   ├─ 对 Func: 调用 FuncSerializer 生成函数体文本 + SourceRangeMap
   ├─ 对 GlobalVar: 直接提取 init value
   └─ 统一通过 StrLines 将 IRSourceRange (byte/char-based) 映射为 SourceLoc (UTF-16 column)
   ▼
返回序列化后的 IRPoolObjDt (FuncObjDt / GlobalVarObjDt / BlockDt / ...)
```

- **两种序列化器并存**：
  - **旧路径**（当前主路径）：`module.rs` 使用 `remusys_ir::ir::{IRSerializer, FuncSerializer}` 生成文本和 `SourceRangeMap`，再由 `StrLines` 转换坐标系。这是当前 `update_func_src`、`make_global_obj` 等功能的实际实现。
  - **新路径**（实验性）：`source_tree_builder.rs` 中实现了完整的 `IRTreeBuilder`，重新手写了一套 IR 序列化逻辑，输出 `IRTreeNode` 树。但该路径**未被主 API 调用**。

#### 3. 重命名数据流

```
JS 请求 rename(module_id, pool_id, new_name)
   ▼
ModuleInfo::rename(pool_id, new_name)
   ├─ gc_names() 清理已失效 ID 的名称映射
   ├─ 根据 SourceTrackable 类型分发到 rename_global/block/inst/func_arg/use
   ├─ 查重 (模块级或函数级)
   ├─ 修改 names 映射或 Module.symtab / GlobalObj.common.name
   └─ invalidate_user_scopes(value) 收集所有引用该值的函数/全局对象
   ▼
返回 RenameDelta { invalidate_overview: bool, invalidated: Vec<GlobalID> }
```

- **名称分层管理**：
  - **全局对象名**：直接存储在 `Module.symtab` / `GlobalObj.common.name` 中。
  - **函数局部名**（参数、基本块、指令）：存储在 `ModuleInfo.names`（`IRNameMap`）中，`names` 是一个与 `Module` 分离的映射层，前端可自由重命名而不影响 IR 语义。
- **失效传播**：重命名后，所有引用该值的函数（通过 `def-use` 链遍历）会被标记为 `invalidated`，前端需要重新请求这些函数的源码更新。

#### 4. 图分析数据流

```
JS 请求 make_func_cfg / make_dominator_tree / make_block_dfg / make_call_graph
   ▼
ModuleInfo 委托给 graphs/ 下各模块
   ├─ cfg.rs: 利用 remusys_ir::opt::CfgDfsSeq (前序+后序DFS) 计算边分类和关键边标记
   ├─ cfg.rs: 利用 remusys_ir::opt::DominatorTree 构建支配树
   ├─ dfg.rs: 遍历基本块内指令，按 Pure/Effect 分 Section，收集 Use 边
   └─ call_graph.rs: 从根函数 (main / dso_local) 开始 DFS，遍历 Call 指令构建调用图
   ▼
返回序列化后的图 DTO
```

### 与外部库的交互方式

| 外部库 | 交互方式 | 说明 |
|--------|---------|------|
| `wasm-bindgen` | `#[wasm_bindgen]` 宏 + `JsValue`/`JsError` | 定义 `Api` 结构体，所有公共方法直接暴露给 JS。使用 `serde-wasm-bindgen` 将 Rust 结构体序列化为 JS Plain Object（`serialize_maps_as_objects(true)`）。 |
| `remusys-ir` | 直接依赖其核心数据结构 + 算法 | 使用 `Module`、`IRAllocs`、`FuncID`/`BlockID`/`InstID` 等 ID 体系；使用 `IRSerializer`、`FuncSerializer`、`FuncClone`、`DominatorTree`、`CfgDfsSeq` 等内置算法。 |
| `remusys-lang` | `translate_sysy_text_into_full_ir(source)` | SysY 前端编译入口，返回 `ModuleInfo { module, names }`。 |
| `remusys-ir-parser` | `source_to_full_ir(source)` | IR 文本解析入口，返回 `ModuleWithInfo { module, namemap }`。 |
| `mtb-entity-slab` | `#[entity_id(...)]` 宏 | 在 `source_tree.rs` 中用于定义 `IRTreeNodeID`，提供带分代的强类型 ID 和 slab 分配器。 |

---

## 详细设计（按模块分小节）

### 1. `lib.rs` — WASM API 入口

#### 关键结构

- `pub struct Api;`：空结构体，仅作为 `#[wasm_bindgen]` 的载体。
- `serialize_to_js<T: Serialize>`：统一序列化辅助函数，配置 `serialize_maps_as_objects(true)`，确保 Rust 的 `HashMap`/`BTreeMap` 在 JS 侧表现为 Plain Object 而非 Map。

#### WASM 暴露的 API 清单

| 方法签名 | 功能 | 依赖的内部模块 |
|---------|------|-------------|
| `compile_module(source_ty, source)` | 编译 SysY 或 IR 文本，缓存并返回模块 ID | `module::ModuleInfo::compile_from_*` |
| `type_get_name(id, tyid)` | 将 `ValTypeID` 格式化为可读类型字符串 | `module::ModuleInfo::with_module` + `TypeFormatter` |
| `get_globals_brief(id)` | 获取模块概览文本及所有全局对象摘要 | `module::ModuleInfo::get_globals` |
| `load_global_obj(id, global_id)` | 加载指定全局对象的完整 DTO | `module::ModuleInfo::make_global_obj` |
| `load_func_of_scope(id, value_id)` | 根据任意 IR 对象 ID 找到其所属函数并返回函数 DTO | `module::ModuleInfo::try_get_func_scope` |
| `func_scope_of_id(id, value_id)` | 同上，但只返回 `Option<GlobalID>` | `module::ModuleInfo::try_get_func_scope` |
| `rename(id, poolid, new_name)` | 重命名指定对象 | `module::ModuleInfo::rename` |
| `update_func_src(id, func_id)` | 重新生成函数文本及所有内部对象的 SourceLoc | `module::ModuleInfo::update_func_src` |
| `update_overview_src(id)` | 重新生成模块概览文本及全局对象的 SourceLoc | `module::ModuleInfo::overview_or_make` |
| `get_value_used_by(id, val)` | 查询某个 IR Value 被哪些 Use 引用 | `dto::ValueDt::into_value` + `IInst::iter_users` |
| `clone_function(id, func_id)` | 克隆函数，返回新旧 ID 映射 | `remusys_ir::ir::FuncClone` |
| `make_func_cfg(id, func_id)` | 生成函数 CFG | `graphs::cfg::FuncCfgDt::new` |
| `make_dominator_tree(id, func_id)` | 生成支配树 | `graphs::cfg::DomTreeDt::new` |
| `make_block_dfg(id, block_id)` | 生成基本块 DFG | `graphs::dfg::BlockDfgDt::new` |
| `make_call_graph(id)` | 生成模块级调用图 | `graphs::call_graph::CallGraphDt::new` |

#### 宏工具

- `fmt_jserr!`：快速构造 `JsError`。
- `console_log!`：调试日志，输出到浏览器控制台。

---

### 2. `dto.rs` — 数据传输对象

#### 设计目标

将 `remusys-ir` 中不适合直接序列化的内部类型（如 `ValueSSA`、`ConstData`、`UserID`）转换为**扁平、带标签的枚举 DTO**，使 JS 前端无需理解 Rust 内部类型布局即可消费数据。

#### 关键数据结构

- **`ValueDt`**（扁平枚举）：
  ```rust
  pub enum ValueDt {
      None, Undef(ValTypeID), PtrNull,
      I1(bool), I8(i8), I16(i16), I32(i32), I64(StrI64),
      APInt(APInt), F32(f32), F64(f64), ZeroInit(AggrType),
      FuncArg(GlobalID, u32),
      Global(GlobalID), Block(BlockID), Inst(InstID), Expr(ExprID),
  }
  ```
  - `StrI64`：将 `i64` 序列化为字符串，避免 JS `number` 精度丢失。
  - 双向转换：`From<ValueSSA>` 用于 Rust → JS；`into_value(&Module)` 用于 JS → Rust（如 `get_value_used_by` 中反序列化后转回 `ValueSSA`）。

- **`SourceTrackable`**（可追踪源码位置的 IR 对象）：
  ```rust
  pub enum SourceTrackable {
      Global(GlobalID), Block(BlockID), Inst(InstID), Expr(ExprID),
      Use(UseID), JumpTarget(JumpTargetID), FuncArg(GlobalID, u32),
  }
  ```
  这是前端与后端交互的**通用 ID 类型**，覆盖了主干树和边对象。

- **`IRPoolObjDt`**（统一 IR 对象 DTO）：
  ```rust
  pub enum IRPoolObjDt {
      Func(FuncObjDt), GlobalVar(GlobalVarObjDt), Block(BlockDt),
      Terminator(TerminatorDt), Inst(NormalInstDt), Phi(PhiInstDt),
  }
  ```
  采用 `#[serde(tag = "typeid")]` 实现类型标签，前端根据 `typeid` 字段区分对象类型。

- **`InstDt`**（指令 DTO，三种变体）：
  - `Normal(NormalInstDt)`：普通指令
  - `Terminator(TerminatorDt)`：终结指令（带 `succs` 后继跳转目标）
  - `Phi(PhiInstDt)`：Phi 指令（带 `incomings` 前驱块与值列表）

- **`SourceUpdates`**：源码更新包，包含 `scope`（Module/Func）、`source`（完整文本）、`ranges`（所有对象的新 SourceLoc）、`elliminated`（已删除对象列表）。

---

### 3. `module.rs` — 模块管理与核心服务

#### 核心结构：`ModuleInfo`

```rust
pub struct ModuleInfo {
    pub module: Box<Module>,      // IR 核心模块实例
    pub names: IRNameMap,          // 函数局部名称映射（参数、基本块、指令）
    pub overview: RefCell<Option<Rc<OverviewInfo>>>, // 模块概览缓存
}
```

- **`ModuleInfo` 是全局状态管理器**：通过 `thread_local` 的 `MODULES: HashMap<SmolStr, ModuleInfo>` 保存所有已加载模块。
- **访问模式**：提供 `with_module`（只读借用）和 `with_module_mut`（可变借用）两种静态方法，所有 API 都通过这两个入口访问模块状态。

#### 编译与缓存

- `compile_from_sysy(source)` → 调用 `remusys_lang::translate_sysy_text_into_full_ir`
- `compile_from_ir(source)` → 调用 `remusys_ir_parser::source_to_full_ir`
- `insert_module(info)` → 生成 `module_N` 格式 ID，存入全局 HashMap。

#### 概览（Overview）生成

`make_overview()` 使用 `IRSerializer::new_buffered(module, names)`：
1. 遍历 `symtab.exported()` 中的所有全局对象。
2. 对全局变量：调用 `ser.fmt_global(id)`。
3. 对函数：调用 `ser.fmt_func_header(FuncID)`，记录 header 的 source range。
4. 提取序列化后的字符串，构建 `OverviewInfo`（含行首字节偏移数组 `lines`，用于 UTF-16 列号转换）。

#### 函数源码更新

`update_func_src(func_id)` 使用 `FuncSerializer::new_buffered(module, func_id, names)`：
1. 启用 `enable_srcmap()`，完整序列化函数体。
2. 提取 `SourceRangeMap`，其中包含 `funcargs`、`blocks`、`insts`、`uses`、`jts` 的源码范围。
3. 通过 `StrLines::map_range` 将 `IRSourceRange`（基于字符偏移）转换为 `SourceLoc`（基于 UTF-16 码元，行号从 1 开始）。

#### 对象 DTO 构建

- `make_global_obj(id)`：根据 `GlobalObj::Func` / `GlobalObj::Var` 分发到 `make_func_obj` / `make_var_obj`。
- `make_func_obj(func, base)`：
  - 外部函数：返回空 `args` 和 `blocks: None`。
  - 内部函数：使用 `FuncSerializer` 生成文本，遍历 `func.args` 构建 `FuncArgDt`（含 source_loc），遍历 `body.blocks` 构建 `BlockDt` 数组。
- `make_block_obj` / `make_inst_obj` / `make_use_dt` / `make_jt_dt`：逐级构建，source_loc 均来自 `SourceRangeMap`。

#### 作用域查询

`try_get_func_scope(id: SourceTrackable) -> Option<GlobalID>`：
- 对 `Global`：检查是否为函数。
- 对 `Block`：返回 `block.get_parent_func()`。
- 对 `Inst`/`Use`/`JumpTarget`：沿父链追溯到函数。
- 对 `Expr`：返回 `None`（Expr 没有 `get_parent` 方法）。

#### 图分析委托

- `make_func_cfg` → `FuncCfgDt::new`
- `make_dominator_tree` → `DomTreeDt::new`
- `make_block_dfg` → `BlockDfgDt::new`
- `make_call_graph` → `CallGraphDt::new`

---

### 4. `rename.rs` — 重命名引擎

#### 错误模型

```rust
pub enum RenameErr {
    InvalidID(SourceTrackable),
    InvalidName(SmolStr),
    NameRepeated { name: SmolStr, existed: SourceTrackable },
}
```

#### 核心数据结构

- **`RenameDelta`**：描述重命名的副作用。
  - `invalidate_overview: bool`：是否需刷新模块概览（仅全局对象重命名时触发）。
  - `invalidated: Vec<GlobalID>`：所有需要重新序列化的函数/全局对象 ID 列表。
- **`FunctionNames`**：函数作用域内的名称快照，供前端管理作用域和查重（但**未被当前 API 暴露**）。

#### 重命名分发逻辑

`rename(id, new_name)` 的执行流程：
1. `gc_names()`：清理 `names` 映射中已失效（ID 被删除）的条目。
2. 合法性检查：`id.is_alive()` + `utils::name_is_ir_ident(new_name)`。
3. 按类型分发：
   - `Global` → `rename_global`：修改 `GlobalObj.common.name`，若已导出则重新导出（unexport + rename_and_export）。遍历所有 `def-use` 用户，将引用该全局对象的函数加入 `invalidated`。
   - `Block` → `rename_block`：写入 `names.blocks`，检查父函数内是否重名。
   - `Inst` → `rename_inst`：写入 `names.insts`，检查父函数内是否重名。
   - `FuncArg` → `rename_func_arg`：写入 `names.funcs[func_id][index]`，检查函数内是否重名。
   - `Use` → `rename_use`：代理到其操作数对应的实际对象。
   - `JumpTarget` → 代理到其目标 `Block`。
   - `Expr` → 直接返回 `no_need_to_rename()`（表达式无持久名称）。

#### 名称查重算法

`is_name_used_in_function(func, name, exclude)`：
- 扫描 `names.funcs[func]`（参数名）
- 扫描 `names.insts`（过滤属于该函数的指令）
- 扫描 `names.blocks`（过滤属于该函数的基本块）
- 时间复杂度：O(该模块所有命名指令/基本块总数)，未使用倒排索引。

#### 作用域失效算法

`invalidate_user_scopes(value: ValueSSA)`：
1. 获取 `value.as_dyn_traceable(allocs)`，遍历其所有 Use。
2. 对每个 Use，找到 `UserID`，如果是 `Inst` 则追溯到其所在函数；如果是 `Global` 则直接加入失效列表。
3. 特殊处理：如果重命名的是函数本身，额外遍历该函数 `FuncObj` 的 `users()`（即所有调用点），将调用者函数加入失效列表。
4. 排序去重后返回。

#### 工具函数

- `utils::name_is_ir_ident(name)`：手写状态机实现 IR 标识符校验，允许 `[0-9A-Za-z_]+(\.[0-9A-Za-z_]+)*` 模式，含完整单元测试。

---

### 5. `graphs/call_graph.rs` — 调用图

#### 算法设计

采用**基于 DFS 的直接调用图构建器** `CallGraphDirectBuilder`：

1. **根函数识别**：遍历 `symtab.func_pool()`，将 `main` 或 `Linkage::DSOLocal` 的函数标记为 `Root`，其余为 `Other`。
2. **DFS 遍历**：从所有 Root 开始压栈，逐函数提取 `Call` 指令，记录边 `(caller, callee, use_id)`。
3. **角色传播**：
   - Root 调用的函数 → `Live`
   - Live 调用的函数 → `Live`
   - 未被 Root 可达的函数 → `Unreachable`
4. **间接调用未处理**：`dump_direct_calls` 中明确过滤掉非 `ValueSSA::Global` 的 callee，因此**函数指针调用被忽略**。

#### 输出 DTO

- `CallGraphDt { nodes: Vec<CallGraphNode>, edges: Vec<CallGraphEdge> }`
- `CallGraphNode { id: GlobalID, role: CallNodeRole }`
- `CallGraphEdge { id: UseID, caller: GlobalID, callee: GlobalID }`

---

### 6. `graphs/cfg.rs` — 控制流图与支配树

#### 支配树 (`DomTreeDt`)

- 封装 `remusys_ir::opt::DominatorTree`。
- `DominatorTree::builder(allocs, func)?.build()` 构建后，遍历 `dt.nodes`，提取 `(idom_block, block)` 边。
- **限制**：当前 `TryFrom` 实现中若遇到 `CfgBlockStat::PostDom`（后支配）会报错 `"post-dominance not supported"`，即**仅支持前支配树**。

#### CFG (`FuncCfgDt`)

- 输入：`ModuleInfo` + `FuncID`。
- 核心步骤：
  1. 获取函数 `body` 和 `entry` 块。
  2. 计算前序 DFS (`CfgDfsSeq::new_pre`) 和后序 DFS (`CfgDfsSeq::new_post`)。
  3. 遍历所有基本块，分类节点类型：
     - `Entry`：等于 `entry`
     - `Unreachable`：DFS 不可达
     - `Exit`：无后继
     - `Control`：其他
  4. 遍历所有后继边（`JumpTarget`），计算：
     - `is_critical`：当 `from_succs.len() > 1 && to_preds.is_multiple()` 时为关键边。
     - `edge_class`：基于前序/后序 DFS 序判断：
       - `SelfRing`：`to == from`
       - `Tree`：`to` 的 DFS 父节点是 `from`
       - `Back`：`to` 是 `from` 的祖先（前序更小、后序更大）
       - `Forward`：`to` 是 `from` 的后代（前序更大、后序更小）
       - `Cross`：其他
       - `Unreachable`：`from` 不可达

---

### 7. `graphs/dfg.rs` — 基本块级数据流图

#### 核心设计：`BlockDfgBuilder`

为单个基本块构建 DFG，节点为 `DfgNodeID`，边为 `UseID` 的 def-use 关系。

#### Section 划分

DFG 将节点组织为多个 `DfgSection`，以区分副作用边界：

- **Section 0 (Income)**：所有来自块外的操作数（常量、全局变量、函数参数、其他块的值）。
- **Section 1 (Outcome)**：所有被块外使用的值（指令的结果被其他块/全局对象使用）。
- **内部 Sections**：按指令顺序排列，每个 Section 要么是 `Pure`（纯计算，无副作用），要么是 `Effect`（有副作用，如 Store、Call、Terminator）。相邻同类型指令合并到同一 Section。

#### 边的构建

对块内每条指令：
1. 遍历其 `operands`（UseID）：将 `(user, operand)` 加入边集，`operand` 放入 `Income` Section（如果是外部值）。
2. 遍历其 `users`（反向 def-use）：将 `(user, operand)` 加入边集，`user` 放入 `Outcome` Section。

#### 特殊处理

- `GuideNode` 和 `PhiInstEnd` 被跳过。
- `Call` 指令：若被调函数标记为 `pure`（`attrs().is_func_pure()`），则视为 `Pure`，否则为 `Effect`。
- 边的 `section_id`：若 `user` 和 `operand` 在同一 Section，则记录该 Section ID，否则为 `None`。

---

### 8. `module/source_buf.rs`、`source_tree.rs`、`source_tree_builder.rs` — 源码树（实验性模块）

#### `IRSourceBuf`

- 基于 `SmallVec<[u8; 16]>` 的行式文本缓冲区，实现 `std::io::Write`。
- 支持行级增量更新 `apply_line_update(range, new_lines)`，用于后续源码编辑的局部替换。

#### `IRTree` 与 `IRTreeNode`

- **目标**：建立 IR 对象与源码文本位置之间的**双向树映射**，解决 IR 中操作数 DAG 无法直接挂到主干树的问题。
- **结构**：
  ```rust
  pub struct IRTree {
      pub alloc: IRTreeAlloc,                    // slab 分配器
      pub overview_id: IRTreeNodeID,             // 模块概览视图根节点
      pub local_views: HashMap<FuncID, IRTreeNodeID>, // 每函数局部视图（未使用）
  }
  ```
- `IRTreeNode` 包含 `parent`、`children`、`ir_obj: IRTreeObjID`、`src_span: Range<IRSrcTreePos>`、`depth`。
- `IRTreeNodeID`：使用 `mtb-entity-slab` 的 `#[entity_id(...)]` 宏生成，序列化为 `u64`（仅低 48 位有效，适配 JS 安全整数范围）。

#### `IRTreeBuilder`

- **独立重新实现了完整的 IR 序列化逻辑**，与 `remusys_ir::ir::IRSerializer` 平行：
  - 支持 `build_overview()`：构建模块概览树（函数头 + 全局变量）。
  - 支持 `build_func(func)`、`update_block(block)`、`update_inst(inst)`：构建局部视图（**未接入主 API**）。
  - 对每种指令类型（`Alloca`、`GEP`、`Load`、`Store`、`Call`、`Phi`、`Switch`、`Br`、`Ret` 等）均有独立的 `fmt_*` 方法。
  - 对聚合表达式（`Array`、`Struct`、`FixVec`、`SplatArray`、`KVArray`、`DataArray`）有完整的格式化支持。
  - 特别处理了字符串常量字面量的检测（`expr_as_str`）：若 `i8` 数组内容可打印，则格式化为 `c"..."` 风格字符串字面量。

#### 当前状态

- `IRTree::with_overview(module, names)` 是唯一被调用的入口，但**在 `module.rs` 中没有任何地方调用它**。
- `local_views` 始终为空；`update_line_map`、`find_editable` 被标记为 `#[allow(unused)]`。
- 该模块处于**高度实验性状态**，作者注释明确表示 "这个我没想好要怎么做, 现在处在做做看的状态"。

---

## 缺失/未完成部分

1. **`IRTree` / `source_tree` 子模块未接入主 API**
   - `IRTreeBuilder` 虽然实现了完整的序列化，但 `Api` 中没有任何方法暴露 `IRTree` 的构建、查询或更新。
   - `ModuleInfo` 仍使用旧的 `IRSerializer`/`FuncSerializer` 生成 SourceUpdates。
   - `local_views` 字段未填充，局部视图源码树功能缺失。
   - `update_line_map`、`find_editable` 等源码编辑辅助方法未使用。

2. **重命名相关 API 暴露不全**
   - `rename.rs` 中实现了 `check_name_availability(id, new_name)` 和 `get_function_names(func)`，但**未在 `Api` 中暴露给 JS**，前端无法做预检或获取名称快照。

3. **模块生命周期管理缺失**
   - `MODULES` 只增不减，没有 `remove_module` 或 `drop_module` API，存在内存泄漏风险。
   - `ModuleCounter` 使用简单自增，未回收已删除模块的序号。

4. **IR 编辑操作缺失（编辑器核心功能）**
   - 当前仅有 `rename` 和 `clone_function`，缺少：
     - 指令的增删改（创建新指令、删除指令、修改操作数）。
     - 基本块的增删改。
     - 函数的增删改（除克隆外）。
     - 全局变量的增删改。
   - 没有 **Apply Source Edits** 的反向解析能力（即前端修改文本后传回后端解析并更新 IR）。

5. **图分析功能局限**
   - **调用图**：仅处理直接调用（`Call` 指令且 callee 为 `Global`），**函数指针/间接调用被忽略**。
   - **支配树**：仅支持前支配树（`DominatorTree`），**后支配树（Post-Dominator）被显式拒绝**。
   - **DFG**：仅支持**基本块级别**，没有函数级或模块级 DFG。

6. **`FuncInfo` 结构体未使用**
   - `module.rs` 中定义了 `pub struct FuncInfo { pub id: FuncID }`，但整个项目中无任何引用。

7. **`source_tree_builder.rs` 中部分方法被标记为 dead_code**
   - `fmt_use`、`fmt_label`、`of_global`、`fmt_global_var` 等虽已实现，但因上层未调用而被编译器标记为 `#[allow(dead_code)]`。

8. **SourceTree 的 GC 未触发**
   - `IRTree::gc_mark_sweep()` 实现了基于标记-清除的结点池回收，但没有任何调用点。

9. **缺少 Undo/Redo 支持**
   - 重命名和克隆操作直接修改 `ModuleInfo`，没有历史栈或快照机制，前端无法实现撤销。

10. **错误处理与边界情况**
    - `rename_use` 中对 `JumpTarget` 的代理处理与 `rename` 主函数中的处理逻辑存在重复，且 `rename` 主函数对 `JumpTarget` 的处理是直接调用 `rename_block`，而 `rename_use` 也做了同样的代理，逻辑可简化。
    - `ExprID` 在 `try_get_func_scope` 中直接返回 `None`，意味着常量表达式在前端无法定位到所属函数，这可能影响某些交互场景。
