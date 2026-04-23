# remusys-wasm-b2 设计分析

> 分析日期：2026-04-21  
> 版本：0.2.0  
> 分支：feature/rename  
> 源码路径：`remusys-wasm-b2/src/`

---

## 总体设计

### 项目定位

`remusys-wasm-b2` 是 **Remusys 可视化系统的 WASM 后端（第二代）**，负责将 `remusys-ir` 的内存表示编译、序列化，并通过 `wasm-bindgen` 向 TypeScript/前端暴露一组高层 API。其核心职责包括：

1. **源码编译与加载**：支持从 SysY 源码或 Remusys-IR 文本编译为内部 `Module`。
2. **IR ↔ 源码双向映射**：构建一棵与源码文本位置绑定的树（`IRTree`），使得前端可以通过源码位置定位 IR 对象，也可以通过 IR 对象反查源码范围。
3. **可视化数据生成**：按需生成调用图（Call Graph）、控制流图（CFG）、支配树（Dom Tree）、数据流图（DFG）等 DTO，供前端渲染。
4. **交互式编辑支持**：支持重命名（Rename）IR 对象，并通过增量/全量更新维持 IR 树与源码文本的一致性。
5. **可展开的引导树（Guide Tree）**：为前端提供按需展开/收起的树形导航数据结构（`IRExpandTree`）。

### 模块划分

| 模块 | 文件 | 职责 |
|------|------|------|
| **基础设施** | `lib.rs` | crate 入口，声明子模块，提供 `fmt_jserr!`、`js_todo!`、`js_assert!` 三个跨平台宏，用于在 WASM 环境下生成 `JsError`，在非 WASM 环境下 panic。 |
| **WASM 类型绑定** | `types.rs` | 通过 `wasm_bindgen(typescript_custom_section)` 引入 `api/types.ts`，并声明大量 `#[wasm_bindgen(typescript_type = ...)]` 外部类型，用于桥接 Rust serde 类型与前端 TypeScript 类型。 |
| **DTO** | `dto.rs` + `dto/{call_graph,cfg,dfg,dom,testing}.rs` | 定义所有向前端传输的数据结构，如 `ValueDt`、`IRTreeNodeDt`、`CallGraphDt`、`FuncCfgDt`、`BlockDfg`、`DomTreeDt` 等。 |
| **IR 树** | `tree.rs` + `tree/{builder,expand,testing}.rs` | 定义 `IRTree`、`IRTreeNode`、`IRTreeObjID`、`IRTreeNodeID`、`IRTreeCursor`、`IRExpandTree` 等核心结构，实现 IR 对象与源码位置的双向映射。 |
| **核心 WASM API** | `module.rs` + `module/{rename,source_buf}.rs` | 定义 `ModuleInfo`，这是 WASM 暴露给 JS 的唯一主要状态对象，封装了编译、查询、重命名等全部业务逻辑。 |
| **（空文件占位）** | `module/{name_revmap,secondary}.rs` | 目前为空，仅有模块声明。 |

### 数据流概览

```
SysY 源码 / IR 文本
        │
        ▼
┌─────────────────────┐     ┌─────────────────┐
│ remusys-lang /      │────▶│   Module (IR)   │
│ remusys-ir-parser   │     │  + IRNameMap    │
└─────────────────────┘     └────────┬────────┘
                                     │
                                     ▼
                           ┌───────────────────┐
                           │   IRDagBuilder    │───▶ SourceBuf (行缓冲)
                           │  (序列化 + 建树)   │───▶ IRTree (结点位置树)
                           └───────────────────┘
                                     │
                                     ▼
                           ┌───────────────────┐
                           │    ModuleInfo     │◀──── JS 侧主入口
                           │  (WASM 暴露对象)   │
                           └───────────────────┘
                                     │
           ┌─────────────────────────┼─────────────────────────┐
           ▼                         ▼                         ▼
    ┌─────────────┐          ┌─────────────┐          ┌─────────────┐
    │   IRTree    │          │   DTOs      │          │ IRExpandTree│
    │(双向映射查询) │          │(CFG/DFG/...)│          │(展开/收起)  │
    └─────────────┘          └─────────────┘          └─────────────┘
```

### 与外部 crate 的交互

- **`remusys-ir`**（核心依赖）：使用其 `ir::*` 类型（`Module`、`FuncID`、`BlockID`、`InstID`、`UseID`、`ValueSSA` 等）、`opt::CfgDfsSeq` / `DominatorTree`、`typing::*`。启用 `"serde"` 和 `"random-generation"` feature。
- **`remusys-lang`**：用于 `translate_sysy_text_into_full_ir`，将 SysY 源码翻译为 IR。启用 `"remusys-ir-integration"` feature。
- **`remusys-ir-parser`**：用于 `source_to_full_ir`，直接解析 IR 文本。启用 `"serde"` feature。
- **`wasm-bindgen` + `serde-wasm-bindgen`**：所有 DTO 通过 `serde` 序列化后，由 `serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true)` 转为 JS 对象/Map。WASM 侧方法统一返回 `Result<T, JsError>`。
- **`mtb-entity-slab`**：为 `IRTreeNodeID` 提供带代际检查的 slab 分配器，通过 `#[entity_id]` 宏生成强类型 ID。

---

## 详细设计（按模块分小节）

### 1. `lib.rs` — 基础设施

#### 核心宏

- **`fmt_jserr!`**：双模式宏。`fmt_jserr!(Err ...)` 在 `test` 或非 `wasm32` 目标下 `panic!`，在 WASM 下返回 `Err(JsError::new(...))`；无 `Err` 前缀时直接构造 `JsError`。
- **`js_todo!`**：快捷生成 TODO 错误。
- **`js_assert!`**：条件断言宏，失败时返回 `Err(JsError)`。

这些宏统一了开发期（panic）与运行期（JS 异常）的错误处理体验。

### 2. `types.rs` — WASM/TS 类型桥接

- 通过 `include_str!("../api/types.ts")` 将 TypeScript 类型定义注入 WASM 绑定产物，确保前端类型与 Rust serde 形状对齐。
- 声明了约 30 个外部类型，如 `JsTreeObjID`、`JsIRTreeNodeDt`、`JsFuncCfgDt`、`JsBlockDfg`、`JsRenameRes` 等。Rust 侧的 WASM 方法返回这些类型的别名，实际值由 `serde_wasm_bindgen` 转换而来。

### 3. `dto.rs` 及子模块 — 数据传输对象

#### 3.1 `dto.rs` — 公共 DTO 与值表示

- **`StrI64`**：自定义 serde 包装，将 `i64` 序列化为字符串（前端 JS number 精度不足），反序列化时从 `SmolStr` 解析。
- **`ValueDt`**：前端友好的值表示，枚举覆盖 `None`、`Undef`、`PtrNull`、`I1`/`I8`/`I16`/`I32`/`I64`/`APInt`、`F32`/`F64`、`ZeroInit`、`FuncArg`、`Global`、`Block`、`Inst`、`Expr`。
  - `From<ConstData>` / `From<ValueSSA>`：将 IR 内部值转为 DTO。
  - `into_value(&self, module)`：DTO 转回 IR `ValueSSA`，带存活检查（alive check）。
  - `get_name(&self, module, names)`：生成人类可读的名称字符串，处理 `@global`、`%local`、`%auto_numbered` 等各种形式。
- **`IRTreeNodeClass`**：枚举，标记结点类型（`Module`、`GlobalVar`、`ExternFunc`、`Func`、`FuncArg`、`Block`、`PhiInst`、`NormalInst`、`TerminatorInst`、`Use`、`JumpTarget`）。
- **`IRTreeNodeDt`**：前端渲染单个树结点所需的数据，包含 `obj`、`kind`、`label`、`src_range`。

#### 3.2 `dto/call_graph.rs` — 调用图

- **数据结构**：`CallGraphDt { nodes, edges }`，`CallGraphNodeRole` 分 `Public`/`Private`/`Extern`。
- **算法**：`CallGraphBuilder`
  1. `build_nodes`：遍历 `symbols.func_pool()`，按 `Linkage` 分类，按 `(role, label)` 排序。
  2. `build_edges`：遍历非外部函数的指令，识别 `InstObj::Call` 且 callee 为 `ValueSSA::Global` 的调用，去重后添加边。
  - 去重使用 `HashMap<EdgeKey, usize>`（`EdgeList`）。

#### 3.3 `dto/cfg.rs` — 控制流图

- **数据结构**：`FuncCfgDt { nodes, edges }`。
  - `CfgNodeRole`：`Entry` / `Branch` / `Exit`。
  - `CfgEdgeDfsRole`：`Tree` / `Back` / `SelfRing` / `Forward` / `Cross`。
- **算法**：`EdgeRoleJudge`
  - 基于 `CfgDfsSeq::new_pre` 和 `new_post` 获取 DFS 序。
  - 边的分类逻辑：
    - `from == to` → `SelfRing`
    - `pre_dfs.nodes[to].parent == from` → `Tree`
    - 祖先/后代关系通过 pre/post 序比较判定 → `Back` / `Forward`
    - 其余 → `Cross`

#### 3.4 `dto/dfg.rs` — 数据流图（基本块级）

- **数据结构**：
  - `DfgNodeID`：统一标识 `Inst`、`Expr`、`Block`、`Global`、`FuncArg`、`Use`。
  - `DfgNodeRole`：`Income`（跨块输入）、`Outgo`（跨块输出）、`Phi`、`Pure`、`Effect`、`Terminator`。
  - `BlockDfg { sections, edges }`：按 `DfgNodeRole` 分区的结点 + 边列表。
- **算法**：`BlockDfgBuilder`
  - 遍历块内指令，按 `inst_role` 分类（`GuideNode` 和 `PhiInstEnd` 被跳过）。
  - `calls_pure`：检查 callee 函数的 `attrs().is_func_pure()`，决定 `Call` 是 `Pure` 还是 `Effect`。
  - 边的构建：对每条指令的 operand 和 user 分别调用 `add_edge_with_nodes`。
    - **b2 规则**：只有 `Inst` 和 `FuncArg` 放入 `Income` section 并去重；其他操作数作为 `Use(edge)` 放入 user 所在 section 且不去重。
  - 提供 `to_dot_text()` 方法，可直接生成 Graphviz DOT 字符串（带颜色分区）。

#### 3.5 `dto/dom.rs` — 支配树

- **数据结构**：`DomTreeDt { nodes, edges }`，`edges` 为 `(idom, block)` 对。
- **算法**：调用 `DominatorTree::builder(allocs, func)?.build()`，然后按 pre-order 遍历 `DominatorTree.nodes`。
- **限制**：`CfgBlockStat::Block` 断言会拒绝 post-dominance（后支配树）。

### 4. `tree.rs` 及子模块 — IR 源码关联树

#### 4.1 设计动机

`tree.rs` 顶部注释清晰解释了为何不能直接用 IR AST 做源码映射：Remusys-IR 的主干树（Module → Global → Block → Inst）无法覆盖所有需要文本化表达的对象（如操作数、控制流边），而这些对象通过 def-use 构成 **DAG**（有向无环图），不是树。因此需要一套独立的树结构 `IRTree`，将 IR 对象的文本化表示（重新实现的序列化器）与源码位置绑定。

#### 4.2 核心数据结构

- **`IRTreeObjID`**：所有可能被树化的 IR 对象的标识。包括主干树对象（`Module`、`Global`、`FuncArg`、`Block`、`Inst`）和 DAG 对象（`Use`、`JumpTarget`），以及辅助布局对象（`FuncHeader`、`BlockIdent`）。
- **`IRTreeNodeID`**：使用 `mtb-entity-slab` 的 `#[entity_id(..., backend = index)]` 生成的强类型 slab ID，支持代际检查。
- **`IRTreeNode`**：
  - `parent: Cell<Option<IRTreeNodeID>>`：父子关系（树）。
  - `obj: IRTreeObjID`：关联的 IR 对象。
  - `children: IRTreeChildren`（`SmallVec<[IRTreeNodeID; 4]>`）：子结点列表。
  - `pos_delta: SourceRangeIndex`：相对于父结点的源码范围（**相对位置**，减少更新开销）。
- **`IRTree`**：
  - `alloc: EntityAlloc<IRTreeNode>`：结点分配器。
  - `root: IRTreeNodeID`：根结点（对应 `Module`）。
  - `funcs: HashMap<FuncID, IRTreeNodeID>`：函数结点到树结点的快速映射。
- **`SourcePosIndex { line, col_byte }`**：0-based 字节级位置；`advance` 和 `delta_to` 用于相对位置计算。
- **`IRTreeCursor`**（`#[wasm_bindgen]`）：
  - 封装了从根到当前结点的 `node_path` 和每层的绝对 `source_range`。
  - 支持 `new_root`、`from_path`、`goto_parent`、`goto_child`、`get_node`、`get_children`、`emit_path`。
  - 所有权检查：`assert_inside_module` 通过 `module_id` 确保 cursor 不会被跨模块误用。

#### 4.3 关键算法

- **`locate_node_path` / `locate_obj_path`**：根据绝对源码位置，自根向下逐层用 `find_child_by_offset` 定位子结点，将绝对位置减去父位置得到相对位置继续下探。时间复杂度 O(深度 × 平均子结点数)。
- **`get_path_source_range`**：将路径上所有 `pos_delta` 累加，得到该结点对应的绝对源码范围。
- **`resolve_path`**：根据 `IRObjPath`（`IRTreeObjID` 序列）在树中查找对应的 `IRTreeNodePath`。
- **`gc`**：从 `root` 和 `funcs` 值出发 DFS，释放未被引用的 slab 条目。
- **`check_children_invariant`**：检查子结点按源码范围有序且不重叠。

#### 4.4 `tree/builder.rs` — `IRDagBuilder`

这是整个 crate 中最复杂的模块，负责**重新实现 IR 的文本序列化**并同步构建 `IRTree`。

- **状态**：持有 `module`、`names`、`tree`，维护 `source_buf: String`（正在构建的源码文本）、`curr_pos: SourcePosIndex`（当前写入位置）、`indent`（缩进层级）、`scopes`（每个函数的 `FuncNumberMap` 缓存）、`tree_map`（UseID → 已构建树结点的映射，用于 DAG 结点的复用）、`expr_str`（常量字符串表达式缓存）。
- **`fmt::Write` 实现**：所有写入操作同步更新 `source_buf` 和 `curr_pos`（按字符统计换行和字节列）。
- **相对位置机制**：`begin_pos()` 将当前位置压栈，`end_pos()` 弹栈，`relative_pos()` 计算自栈顶以来的 delta。每个结点记录的是这个 delta 范围。
- **`fmt_use`（核心）**：
  - 如果 `use_id` 已在 `tree_map` 中，**复用**已有树结点：克隆子树并插入新的相对位置（解决 DAG 共享问题）。
  - 否则根据 `ValueSSA` 的类型分别处理：`None`、`AggrZero`、`ConstData`、`ConstExpr`（递归）、`FuncArg`、`Block`、`Inst`、`Global`。
  - 新创建的 `Use` 结点会被记入 `tree_map`，以便后续复用。
- **`expr_as_string`**：将 `i8` 数组表达式检测为字符串常量，生成 `c"..."` 格式。
- **指令序列化**：覆盖 `Unreachable`、`Ret`、`Jump`、`Br`、`Switch`、`Alloca`、`GEP`、`Load`、`Store`、`AmoRmw`、`BinOP`、`Call`、`Cast`、`Cmp`、`IndexExtract`/`Insert`、`FieldExtract`/`Insert`、`Phi`、`Select` 等全部指令类型。每个指令将其 `Use` 操作数格式化为子结点。
- **`fmt_func_header` / `fmt_func`**：函数头结点（`FuncHeader`）与函数体结点（`Global`）分离，使得函数头可以独立更新而无需重建整个函数体。

#### 4.5 `tree/expand.rs` — `IRExpandTree`

为前端 Guide View 设计的**状态镜像树**。

- **`Node { ir_object, expand_children }`**：每个结点记录其对应的 IR 对象和已展开的子结点映射（`hashbrown::HashMap`）。`expand_children` 为空表示该结点未展开。
- **`IRExpandTree`**：绑定到某个 `module_id`，根结点默认展开第一层（Module 下的全局对象）。
- **操作**：
  - `expand_one`：展开指定路径结点的直接子结点。
  - `expand_all`：DFS 展开指定路径下的所有后代。
  - `collapse`：清空指定路径结点的 `expand_children`。
  - `path_expanded`：查询展开状态。
  - `load_tree`：根据当前展开状态，与真实 `IRTree` 取交集，生成前端可直接渲染的 `IRGuideNodeDt` 树。
    - **交集语义**：如果旧展开树中的某个子对象在新的 IR 中已不存在（例如重命名后重建导致对象替换），则该分支被丢弃。
    - **焦点标记**：根据传入的 `focus_path`，在结果树上标记 `FocusNode` / `FocusScope` / `FocusParent`。

### 5. `module.rs` 及子模块 — 核心 WASM API

#### 5.1 `ModuleInfo`

`#[wasm_bindgen]` 标记的主状态对象，是 JS 与 Rust 交互的核心句柄。

- **字段**：
  - `source: SourceBuf`：行缓冲的源码文本，支持 UTF-8 ↔ UTF-16（Monaco）列号转换。
  - `ir_tree: IRTree`：IR 与源码的双向映射树。
  - `module: Box<Module>`：IR 内存表示。
  - `names: IRNameMap`：局部名称映射。
  - `rev_local_names: HashMap<FuncID, RevLocalNameMap>`：每个函数内的“名称 → IRTreeObjID”反向表，用于重命名冲突检测。
  - `id: usize`：模块唯一 ID（原子递增），用于 `IRTreeCursor` 所有权校验。

- **构造**：
  - `compile_from("ir" | "sysy", source)`：分别走 `remusys-ir-parser` 或 `remusys-lang` 编译管道。
  - `from_module(module, names)`：统一的后处理，调用 `IRDagBuilder` 构建 `IRTree` 和 `SourceBuf`。

- **序列化辅助**：
  - `serialize<T: Serialize>`：使用 `serde_wasm_bindgen` 并开启 `serialize_maps_as_objects(true)`。
  - `deserialize<T: DeserializeOwned>`：反序列化 JS 值。

#### 5.2 源码位置与路径查询 API

| WASM 方法 | 功能 |
|-----------|------|
| `dump_source()` | 返回当前 Monaco 兼容源码文本。 |
| `path_of_srcpos(pos)` | Monaco 位置 → `IRObjPath`。 |
| `path_get_node(path)` | `IRObjPath` → 结点详情（含源码范围）。 |
| `ir_tree_get_children(path)` | 获取某路径结点的子结点列表。 |
| `path_of_tree_object(object_id)` | 根据 `IRTreeObjID` 反查其在树中的路径。 |
| `get_object_scope(object_id)` | 获取对象所属的函数（Global）scope。 |

- **`path_vec_of_tree_object`**：内部实现，对每种 `IRTreeObjID` 变体手写父链回溯逻辑。例如 `Inst` 需回溯到 `Block` → `Func` → `Module`。

#### 5.3 重命名 API

- **`rename(path, new_name) -> RenameRes`**：
  - 支持重命名的对象：`Module`、`Global`/`FuncHeader`、`FuncArg`、`Block`/`BlockIdent`、`JumpTarget`、`Inst`、`Use`。
  - 对 `Use` 的重命名会透射到其操作数的实际对象（`FuncArg` / `Block` / `Inst` / `Global`）。
  - 冲突检测：
    - 全局名冲突 → `GlobalNameConflict`
    - 局部名冲突（同函数内）→ `LocalNameConflict`
    - 名称无变化 → `NoChange`
    - 无命名对象（如 `ValueSSA::None`）→ `UnnamedObject`
  - **更新策略**：目前采用 **`full_update`**（全量重建）。修改名称后，重新执行 `IRDagBuilder` 构建整棵 `IRTree` 并替换 `SourceBuf`。注释明确说明这是“权宜之计”，后续应优化为增量更新。

#### 5.4 图数据 API

| WASM 方法 | 功能 |
|-----------|------|
| `get_func_cfg(func_id_str)` | 获取函数 CFG。 |
| `get_func_dom_tree(func_id_str)` | 获取函数支配树。 |
| `get_block_dfg(block_id_str)` | 获取基本块 DFG。 |
| `get_call_graph()` | 获取模块级调用图。 |

所有 ID 参数都以字符串形式传入（`to_strid()` 的序列化结果），在 Rust 侧解析为强类型 ID。

#### 5.5 `module/source_buf.rs` — 源码缓冲

- **`SourceLine`**：基于 `SmallVec<[u8; 32]>` 的行缓冲，缓存 `is_ascii` 标志以加速 Monaco（UTF-16）列号转换。
  - `byte_col_to_utf16` / `utf16_col_to_byte`：逐字符转换，对 ASCII 行走快速路径。
- **`SourceBuf`**：行向量，实现 `Display`、`fmt::Write`。
  - `replace(range, new_text)`：支持单行内替换和跨多行替换，内部使用 `SourceBufUpdateBuilder` 处理行合并/拆分逻辑。
  - 严格的范围校验：拒绝非字符边界的位置，拒绝越界。

### 6. `api/types.ts` — TypeScript 类型契约

- 定义了前端与 Rust 之间的全部类型契约，包括 ID 格式（`g:`/`b:`/`i:`/`e:`/`u:`/`j:` 前缀的模板字符串类型）。
- `GuideNodeData` 使用联合类型 `GuideNodeExpand | GuideNodeItem` 区分已展开结点和叶级菜单项。

---

## 缺失/未完成部分

1. **`module/name_revmap.rs` — 缺失**  
   文件为空，仅有模块声明。`RevLocalNameMap` 的定义实际放在 `module.rs` 中。

2. **`module/secondary.rs` — 缺失**  
   文件为空，用途不明，可能是为将来的次要模块信息预留。

3. **增量更新 — 未完成**  
   `IRRename::full_update` 明确是“权宜之计”：每次重命名后**全量重建** `IRTree` 和 `SourceBuf`。对于大模块这会造成性能瓶颈，作者计划在后续优化为增量更新。

4. **后支配树（Post-Dominator Tree）— 不支持**  
   `dto/dom.rs` 中 `DomTreeDt::try_from` 遇到 `CfgBlockStat` 非 `Block` 时会报错 `"post-dominance not supported"`，说明目前只支持前向支配树。

5. **跨基本块数据流 — 不支持**  
   `dto/dfg.rs` 中 `BlockDfg` 的注释和实现明确限定为“单个基本块内的局部数据流”，不包含跨块 `Phi` 参数传递等全局数据流。

6. **`IRTreeCursor` 的 JS 使用面较窄**  
   `IRTreeCursor` 虽然标记为 `#[wasm_bindgen]`，但 `ModuleInfo` 的 WASM API 中并没有返回 `IRTreeCursor` 的方法；前端若要使用 cursor 模式，需要自行在 JS 侧 `new IRTreeCursor(ir)` 或 `IRTreeCursor.from_path(ir, path)`。目前看起来 cursor 更多是为内部/未来扩展设计。

7. **全局变量编辑 — 缺失**  
   目前 `IRDagBuilder` 和重命名逻辑主要围绕函数、基本块、指令展开；对全局变量（`GlobalVar`）的初始化值编辑、属性修改等没有暴露 WASM API。

8. **指令级别的增删改 — 缺失**  
   当前只有重命名（rename）API，没有新增/删除指令、修改指令操作数等编辑能力。

9. **SourceBuf 的持久化/undo — 缺失**  
   `SourceBuf` 支持文本替换，但没有内置 undo/redo 栈或变更历史记录，前端需要自行管理。
