# remusys-wasm 设计文档

> **版本**: 0.2.0  
> **角色**: Visual Remusys 的 Rust/WASM 计算核心  
> **目标读者**: 参与 Visual Remusys 开发/维护的其他 Agent 与人类开发者  
> **文档定位**: 固化已调研的设计全景，标记待深入分析的详细设计点。

---

## 1. 模块概述

`remusys-wasm` 是一个基于 `wasm-bindgen` 的 Rust crate，核心职责是将 **Remusys-IR** 的内存结构封装为前端（`remusys-lens`）可消费的 WASM API。它的工作横跨三个层面：

1. **编译接入层**：把前端传入的 IR 文本或 SysY 源代码翻译为 `remusys-ir` 的 `Module`。
2. **IR ↔ 源码双向映射层**：维护一棵**源码关联树（IR Tree）**，在 IR 对象与格式化源码文本之间建立精确的位置映射。
3. **图数据提取层**：从 `Module` 中提取 CFG、DomTree、BlockDFG、Def-Use Graph、Call Graph 等，序列化为前端可直接渲染的 DTO。

技术栈：Rust 2024 + `wasm-bindgen` + `wasm-pack`（target: bundler）。

---

## 2. 作为 Visual Remusys 一部分的整体设计

### 2.1 系统上下文

```
                          浏览器 (User)
   +---------------------------------------------------------+
   |                   remusys-lens (React/TS)                |
   |   - Monaco Editor 源码编辑                               |
   |   - React Flow / DAG 图可视化                            |
   |   - Zustand 状态管理 (IRStorage)                         |
   +-------------------------+--------------------------------+
                             | WASM JS API (wasm-bindgen)
   +-------------------------v--------------------------------+
   |                   remusys-wasm (WASM)                    |
   |   - ModuleInfo (核心 WASM 暴露对象)                       |
   |   - IRTree / SourceBuf / DTO 序列化                      |
   +-------------------------+--------------------------------+
                             | Rust API
   +------------+------------+------------+-------------------+
   | remusys-ir | remusys-lang | remusys-ir- | mtb-entity-slab |
   | (IR 核心)  | (SysY 前端)  | parser      | 等工具库        |
   +------------+------------+------------+-------------------+
```

### 2.2 与前端交互契约

remusys-wasm 与前端是**命令式调用**关系（JS 主动调用 Rust 方法），而非事件驱动或消息总线。

#### 数据交换格式
- 所有 DTO 通过 **`serde-wasm-bindgen`** 直接序列化为 **JS Object**（`serialize_maps_as_objects(true)`），而非 JSON 字符串。
- TypeScript 类型定义源文件位于 `remusys-wasm/api/types.ts`，通过 `#[wasm_bindgen(typescript_custom_section)]` 内嵌到生成的 `.d.ts` 中。

#### 核心交互模式

| 模式 | 入口方法 | 说明 |
|------|---------|------|
| **编译** | `ModuleInfo.compile_from(ty, source, filename)` | `ty` 为 `"ir"` 或 `"sysy"`。返回 `ModuleInfo` 实例，前端缓存于 Zustand Store。 |
| **位置查询** | `path_of_srcpos(pos)` | Monaco 1-based UTF-16 坐标 -> `IRObjPath`。 |
| **树导航** | `path_get_node(path)`, `ir_tree_get_children(path)` | 路径 -> 结点详情 / 子结点列表。 |
| **图生成** | `get_func_cfg`, `get_block_dfg`, `get_def_use_graph`, `get_call_graph`, `get_func_dom_tree` | 输入为字符串化的 ID，输出为序列化 DTO。 |
| **变更** | `rename(path, new_name)` | 全量更新策略，成功后前端需废弃缓存、重新建树。返回 `RenameRes`。 |

### 2.3 与下游 crate 的依赖关系

| 依赖 | 用途 | remusys-wasm 中的消费点 |
|------|------|------------------------|
| `remusys-ir` | IR 内存结构、优化算法、Def-Use 链、CFG | `Module` 定义、`InstID`/`BlockID`/`UseID` 等 ID 体系、`DominatorTree`、`CfgDfsSeq` |
| `remusys-lang` | SysY 语言编译前端 | `compile_from_sysy` -> `translate_sysy_text_into_full_ir` |
| `remusys-ir-parser` | IR 文本解析器 | `compile_from_ir` -> `source_to_full_ir` |
| `mtb-entity-slab` | Slab 分配器 + 强类型分代 ID | `IRTreeNodeID` 的定义与内存管理 |
| `serde` + `serde-wasm-bindgen` | Rust <-> JS 序列化 | 所有 `#[derive(Serialize)]` 的 DTO 类型 |
| `smol_str` | 小字符串优化 | 名称、标签、类型名等高频短字符串 |
| `smallvec` | 栈上小数组 | `IRTreeChildren`、`IRObjPathBuf`、`IRTreeNodePathBuf` |

### 2.4 数据生命周期

以下是一个典型用户操作的完整数据流：

```
1. 用户在前端编辑器输入 SysY 代码
        |
        v
2. 前端调用 ModuleInfo.compile_from("sysy", source, "main.sy")
        |
        v
3. WASM 内部链路:
   remusys-lang::translate_sysy_text_into_full_ir()
        -> 生成 Module + IRNameMap
        -> ModuleInfo::from_module()
        -> IRTreeBuilder::build(IRTreeObjID::Module)
        -> 生成格式化源码文本 + IRTree
        -> 返回 ModuleInfo 实例
        |
        v
4. 前端缓存 ModuleInfo 到 Zustand (IRState.module)
        |
        v
5. 用户点击源码中某位置
   -> path_of_srcpos(MonacoSrcPos) -> IRObjPath
   -> path_get_node(path) -> IRTreeNodeDt (含 src_range, kind, label)
        |
        v
6. 用户请求查看某个函数的 CFG
   -> get_func_cfg("g:...") -> FuncCfgDt -> 前端用 React Flow 渲染
        |
        v
7. 用户重命名一个基本块
   -> rename(path, "new_bb") -> RenameRes::Renamed
   -> 前端收到 Renamed，废弃 ModuleInfo 缓存，重新调用 compile_from
   （注意：当前 rename 内部已重建 SourceBuf + IRTree，但前端 ID 缓存仍需要刷新）
```

---

## 3. 模块内部详细设计

### 3.1 目录与模块组织

```
remusys-wasm/
  Cargo.toml
  api/
    types.ts              # TypeScript 类型定义源文件（被 types.rs 内嵌）
  cases/
    main.sy               # 示例 SysY 源码（编译测试用）
  pkg/                    # wasm-pack 输出目录
  src/
    lib.rs                # 根模块、公共导出、fmt_jserr!/js_todo!/js_assert! 宏
    types.rs              # TypeScript 类型桥接（wasm_bindgen extern types）
    dto.rs                # DTO 基础：ValueDt, StrI64, IRTreeNodeClass, IRTreeNodeDt
    dto/
      cfg.rs              # FuncCfgDt / CfgNodeDt / CfgEdgeDt / EdgeRoleJudge
      dom.rs              # DomTreeDt / DomTreeNodeDt / DomTreeEdgeDt
      dfg.rs              # BlockDfg / DfgNode / DfgSection / DfgEdge / BlockDfgBuilder
      defuse_graph.rs     # DefUseGraphDt
      call_graph.rs       # CallGraphDt / CallGraphNodeDt / CallGraphEdgeDt
      testing.rs          # DTO 测试辅助（目前为空或预留）
    module.rs             # ModuleInfo 核心类型与 WASM API 实现
    module/
      rename.rs           # IRRename / RenameRes，全量更新策略
      source_buf.rs       # SourceBuf / SourceLine，坐标转换与文本替换
      secondary.rs        # （源码中存在，内容待确认）
    tree.rs               # IRTree / IRTreeNode / IRTreeNodeID / IRTreeObjID / IRTreeCursor
    tree/
      builder.rs          # IRTreeBuilder：IR 格式化打印 + IR Tree 构建（~1130 行）
      expand.rs           # 树展开/拷贝逻辑（待确认细节）
      testing.rs          # 树测试辅助
```

### 3.2 核心类型体系

类型之间的关系可分为四个层次：

```
+-------------------------------------------------------------+
|  Layer 4: WASM 暴露层                                        |
|  ModuleInfo (wasm_bindgen struct)                           |
|  持有：source, ir_tree, module, names, rev_local_names      |
+-------------------------------------------------------------+
                              | 通过引用访问
+-------------------------------------------------------------+
|  Layer 3: DTO 序列化层                                       |
|  FuncCfgDt, DomTreeDt, BlockDfg, DefUseGraphDt, CallGraphDt |
|  IRTreeNodeDt, IRTreeObjID, IRTreeObjName, RenameRes        |
|  特征：#[derive(Serialize)]，通过 serde-wasm-bindgen -> JS  |
+-------------------------------------------------------------+
                              | 包装/转换
+-------------------------------------------------------------+
|  Layer 2: IR 抽象层（remusys-wasm 特有）                      |
|  IRTree, IRTreeNode, IRTreeNodeID, IRTreeObjID              |
|  SourceBuf, SourceLine, SourcePosIndex, SourceRangeIndex    |
|  RevLocalNameMap                                            |
|  特征：将 remusys-ir 的内存结构映射为源码关联树              |
+-------------------------------------------------------------+
                              | 底层引用
+-------------------------------------------------------------+
|  Layer 1: remusys-ir 原生类型                                |
|  Module, InstID, BlockID, UseID, ExprID, GlobalID           |
|  FuncID, JumpTargetID, ValueSSA, InstObj, IRNameMap         |
|  DominatorTree, CfgDfsSeq                                   |
+-------------------------------------------------------------+
```

#### 3.2.1 IRTreeObjID：IR 对象在树中的标识

`IRTreeObjID` 是一个枚举，把 remusys-ir 中多种 ID 类型统一为树结点的对象标识：

```rust
pub enum IRTreeObjID {
    Module,                         // 模块根，无 ID
    Global(GlobalID),               // 全局对象（函数/全局变量）
    FuncArg(GlobalID, u32),         // 函数参数 (function_global_id, arg_index)
    Block(BlockID),                 // 基本块
    Inst(InstID),                   // 指令
    Use(UseID),                     // 操作数使用边（DAG 中的边）
    JumpTarget(JumpTargetID),       // 控制流跳转目标
    FuncHeader(GlobalID),           // 函数头声明行（独立结点，方便单独更新）
    BlockIdent(BlockID),            // 基本块标签行（独立结点，方便单独更新）
}
```

**设计意图**：
- `FuncHeader` 和 `BlockIdent` 被单独拎出来，是因为它们通常是 UI 中需要**单独高亮/编辑**的单元（如函数签名、基本块名称）。如果它们被包含在父结点中，任何小的变更都需要更新整个父结点的源码范围。
- `Use` 和 `JumpTarget` 代表主干树之外的图结构边。同一个 `UseID` 在 IR 中可能被多条指令共享（DAG），但在 IR Tree 中每次出现都会创建独立的树结点。

#### 3.2.2 IRTreeNode / IRTreeNodeID：树结点与内存管理

- `IRTreeNodeID` 由 `mtb-entity-slab` 的 `#[entity_id(IRTreeNodeID, backend = index)]` 宏生成，为 **48-bit 分代索引**（`GenIndex`），序列化为 `u64`。
- 每个结点包含：
  - `obj: IRTreeObjID` —— 该结点代表的 IR 对象
  - `children: IRTreeChildren` —— 子结点 ID 列表（`SmallVec<[IRTreeNodeID; 4]>`）
  - `children_map: BrownMap<IRTreeObjID, usize>` —— 当子结点多于 8 个时的快速查找表
  - `pos_delta: SourceRangeIndex` —— **相对于父结点**的源码范围（相对坐标系统）
  - `parent: Cell<Option<IRTreeNodeID>>` —— 父结点指针
  - `disposed: Cell<bool>` —— 标记是否已释放

**相对坐标系统的设计**：
- `SourcePosIndex { line: u32, col_byte: u32 }` 使用 0-based 行号和字节列号。
- `pos_delta` 采用**相对坐标**而非绝对坐标，目的是减少结点更新时的连锁反应。当父结点位置变化时，只要更新父结点的 `pos_delta`，子结点的相对坐标无需改变。
- 绝对坐标通过从根结点沿路径累加 `pos_delta` 得到（`IRTree::get_path_source_range`）。

#### 3.2.3 ValueDt：IR 值的序列化表示

`dto.rs` 中的 `ValueDt` 是 `ValueSSA` 和 `ConstData` 的序列化友好版本：

```rust
pub enum ValueDt {
    None,
    Undef(ValTypeID),
    PtrNull,
    I1(bool), I8(i8), I16(i16), I32(i32), I64(StrI64),  // StrI64: i64 序列化为字符串
    APInt(APInt),
    F32(f32), F64(f64),
    ZeroInit(AggrType),
    FuncArg(GlobalID, u32),
    Global(GlobalID), Block(BlockID), Inst(InstID), Expr(ExprID),
}
```

**设计意图**：
- `StrI64` 解决 JavaScript 无法安全表示 `i64` 的问题（JSON/JS 的 number 只有 53-bit 精度）。
- `FuncArg` 使用 `GlobalID`（函数的全局 ID）+ `u32`（参数索引）来表示，而不是 `FuncArgID`，因为 `FuncArgID` 不是 wasm-bindgen 友好的类型。


### 3.3 数据流与控制流

#### 3.3.1 编译与初始化流程

```rust
// module.rs
ModuleInfo::compile_from(ty, source, filename)
  |-- "ir"  -> compile_from_ir(source)
  |            \-- remusys_ir_parser::source_to_full_ir(source)
  |-- "sysy" -> compile_from_sysy(source)
  |            \-- remusys_lang::translate_sysy_text_into_full_ir(source)
  \-- from_module(module, names)
       |-- IRTree::new()                         // 创建空树
       |-- IRTreeBuilder::new(module, names, &ir_tree)
       |   \-- build(IRTreeObjID::Module)        // 递归格式化 IR 并建树
       |       |-- fmt_module() -> fmt_global()   // 遍历全局对象
       |       |-- fmt_func() -> fmt_func_header() + fmt_func_body()
       |       |-- fmt_block() -> do_fmt_block()
       |       |-- fmt_inst() -> do_fmt_inst()    // 匹配所有指令类型
       |       \-- fmt_use() -> fmt_expr() / fmt_const_data() // 递归展开操作数
       |-- SourceBuf::from(builder.source_buf)    // 收集格式化文本
       \-- ir_tree.root = root                    // 设置树根
```

**关键实现**：`IRTreeBuilder` 实现 `std::fmt::Write`，在 `write_str` 时同步更新当前光标位置 `curr_pos`。每打印一个 IR 对象，就记录其相对父结点的 `pos_delta`，并分配 `IRTreeNodeID`。

#### 3.3.2 位置查询流程（源码位置 -> IR 对象）

```rust
// module.rs
ModuleInfo::path_of_srcpos(pos: JsMonacoSrcPos)
  |-- deserialize(pos) -> MonacoSrcPos
  |-- source.monaco_pos_to_byte(monaco_pos) -> SourcePosIndex
  |   |-- 行号转换：monaco.line - 1
  |   \-- 列号转换：SourceLine::utf16_col_to_byte(column - 1)
  |       \-- ASCII 快速路径 / UTF-8 逐字符遍历
  \-- ir_tree.locate_obj_path(byte_pos) -> IRObjPathBuf
      \-- locate_node_path(byte_pos) -> IRTreeNodePathBuf
          \-- 从 root 开始，递归 find_child_by_offset(pos)
              |-- 子结点数 < 8：线性扫描
              \-- 子结点数 >= 8：二分查找（按 pos_delta.start 排序）
```

#### 3.3.3 图生成流程（以 CFG 为例）

```rust
// module.rs
ModuleInfo::get_func_cfg(func_id: &str)
  |-- global_strid_as_func(func_id) -> FuncID
  \-- FuncCfgDt::new(&self.module, &self.names, func_id)
      |-- EdgeRoleJudge::new(module, func)     // 构建前序/后序 DFS 序列
      |   |-- CfgDfsSeq::new_pre(module, func)
      |   \-- CfgDfsSeq::new_post(module, func)
      |-- FuncNumberMap::new(...)              // 为匿名块分配编号
      |-- 遍历函数所有基本块
      |   |-- 创建 CfgNodeDt（role: Entry/Branch/Exit）
      |   \-- 遍历基本块后继，创建 CfgEdgeDt
      |       \-- edge_role.role(from, to)     // Tree/Back/Forward/Cross/SelfRing
      \-- serialize(&cfg_dt) -> JsValue
```

#### 3.3.4 重命名流程

```rust
// module.rs -> rename.rs
ModuleInfo::rename(path, new_name)
  |-- deserialize(path) -> IRObjPathBuf
  |-- IRRename::new(self, last_obj).rename(new_name)
  |   |-- 根据对象类型分发：
  |   |   |-- Module -> 修改 module.name
  |   |   |-- Global/FuncHeader -> rename_global()
  |   |   |-- FuncArg -> rename_arg()
  |   |   |-- Block/BlockIdent/JumpTarget -> rename_block()
  |   |   |-- Inst -> rename_inst()
  |   |   \-- Use -> rename_use() -> 透传到 operand 对应类型
  |   |-- 检查名称冲突（GlobalNameConflict / LocalNameConflict）
  |   \-- full_update()                        // 全量重建
  |       |-- IRTree::new()
  |       |-- IRTreeBuilder::build(Module)
  |       \-- SourceBuf::from(builder.source_buf)
  \-- serialize(&RenameRes) -> JsRenameRes
```

### 3.4 关键机制

#### 3.4.1 Use 边的 DAG -> Tree 展开（拷贝展开）

IR 中的 `UseID` 可能在多条指令中共享（例如同一个常量表达式被多处引用），形成 DAG。IR Tree 要求每个结点有唯一的父结点（树结构），因此需要**展开**。

展开策略（位于 `tree/builder.rs` 的 `fmt_use` 方法）：

```
第一次遇到 UseID U：
  -> 创建新的 IRTreeNode N1，记录源码片段 [begin_byte..end_byte]
  -> 在 tree_map 中缓存：U -> { id: N1, src: [begin..end] }

后续再次遇到同一个 UseID U：
  -> 从 tree_map 中取出 N1 和源码片段
  -> 调用 N1.insert_pos_delta(tree, new_delta)
    -> 拷贝 N1 的整个子树（clone_subtree）
    -> 分配新的结点 ID N2
    -> 设置 N2 的 pos_delta 为新的相对位置
  -> 返回 N2
```

**设计权衡**：
- 优点：前端可以精确点击任意一个操作数位置，高亮只影响该位置。
- 代价：内存中树的规模可能远大于 IR 本身的 DAG 规模。`clone_subtree` 会递归拷贝所有子结点。

#### 3.4.2 WASM 错误处理的跨平台策略

`lib.rs` 中定义了三个宏，实现**测试/WASM 双模式**：

```rust
#[macro_export]
macro_rules! fmt_jserr {
    (Err $($arg:tt)*) => {
        if cfg!(test) || cfg!(not(target_arch = "wasm32")) {
            panic!($($arg)*);          // 非 WASM：直接 panic（方便调试）
        } else {
            Result::Err(JsError::new(...))  // WASM：返回 JsError
        }
    };
    ($($arg:tt)*) => {
        JsError::new(&format!($($arg)*))
    };
}
```

- `js_todo!()` —— 未实现功能占位。
- `js_assert!()` —— 断言失败时 WASM 返回 `Err(JsError)`，测试时 panic。

**为什么必须分两支**：`wasm_bindgen::JsError` 只在 `target_arch = "wasm32"` 下可用，非 WASM 的测试/原生环境根本没有这个类型。如果不把非 WASM 分支降级为 `panic!`，测试连编译都过不了；即便能编译，一遇到报错就会先被 "不支持 JsError" 的运行期 panic 拦截，真正的错误信息和堆栈根本看不到。因此 `panic!` 不是随意的权宜之计，而是**环境限制下的被迫选择**。

**权衡与风险**：这种双分支策略保证了测试可调试性，但也意味着测试路径和 WASM 路径的错误处理行为本质不同——测试 panic 会中断执行，而 WASM 返回 `Result::Err` 可以被前端捕获。边界情况（如某个分支返回了 `Ok` 但另一个分支 panic）可能导致测试通过而 WASM 行为异常，需保持警惕。

#### 3.4.3 BlockDFG 的分节机制

`dto/dfg.rs` 中 `BlockDfgBuilder` 将基本块内的指令按**副作用语义**切分为若干 `DfgSection`：

```
遍历基本块内所有指令：
  |-- GuideNode / PhiInstEnd -> 跳过
  |-- Phi -> DfgNodeRole::Phi
  |-- Unreachable / Ret / Jump / Br / Switch -> Terminator
  |-- Store / AmoRmw -> Effect
  |-- Call -> 检查 callee.attrs().is_func_pure() -> Pure 或 Effect
  \-- 其他 -> Pure

相同角色的连续指令合并到同一个 Section
角色变化时开启新 Section
```

**b2 规则**（跨块结点处理）：
- 操作数为 `Inst` 或 `FuncArg`（来自其他基本块）：放入独立的 `Income` 节，**全局去重**。
- 其他操作数（常量、表达式、全局变量）：以 `Use` 形式放在 user 所在的 Section，**不去重**。

#### 3.4.4 源码缓冲区的坐标转换

`SourceLine` 同时维护 `buffer: SmallVec<[u8; 32]>`（UTF-8 字节）和 `is_ascii: bool`（快速路径标记）。

转换逻辑：
- **ASCII 行**：`byte_col == utf16_col`，直接返回。
- **非 ASCII 行**：逐字符遍历，累加 `ch.len_utf8()` 和 `ch.len_utf16()`，找到对应位置。
- **边界检查**：`utf16_col_to_byte` 拒绝落在字符中间的列号。

---

## 4. 值得深入分析的设计点（待详细设计）

以下设计点已经过初步调研，但其内部机制、边界条件、性能特征或演化方向值得后续 Agent 做**专题级别的深入分析**。每个条目包含：**当前状态简述**、**复杂性来源**、**建议深挖方向**。

### 4.1 IRTree 的内存管理与生命周期（高优先级）

- **锚点文件**：`src/tree.rs`（~1259 行）
- **当前状态**：`IRTree` 使用 `EntityAlloc<IRTreeNode>` 分配结点，内部有 `IRTreeInner { unmap, free_queue }`。结点支持 `dispose()`（标记释放）和 `tree_dispose()`（递归释放子树）。`ManagedTreeNodeID` 提供 RAII 管理。`gc()` 方法可 DFS 遍历回收不可达结点。
- **复杂性来源**：
  - `IRTreeNodeID` 有 48-bit 分代，需要理解 `mtb-entity-slab` 的 `GenIndex`、`IndexedID`、`IEntityAllocID` 等 trait。
  - `unmap: BrownMap<IRTreeObjID, SmallVec<[IRTreeNodeID; 2]>>` 维护"一个 IR 对象 -> 多个树结点"的映射（因为 DAG 展开）。这个映射的维护与释放逻辑容易出错。
  - `disposed` 标记 + `free_queue` 的延迟释放模式。
- **建议深挖方向**：
  1. `gc()` 和 `free_disposed()` 在当前主流程中是否被调用？如果没有，是否存在内存泄漏风险？
  2. `ManagedTreeNodeID` 的 `Drop` 实现是否在所有路径上都被正确调用？
  3. `unregister_unmap` 在结点释放时的逻辑是否完整？
  4. 大树（上千结点）的内存占用和 GC 开销如何？

### 4.2 IRTreeBuilder 的格式化-建树耦合（高优先级）

- **锚点文件**：`src/tree/builder.rs`（~1130 行）
- **当前状态**：`IRTreeBuilder` 同时承担**IR 文本打印机**和**IR Tree 构建器**两个职责。它实现 `std::fmt::Write`，在 `write_str` 时更新 `curr_pos`，并在打印每个 IR 对象前后记录 `pos_delta`。
- **复杂性来源**：
  - 代码量巨大，涵盖所有指令类型、表达式类型、属性、全局变量、函数头的格式化逻辑。
  - `fmt_use` 中的 DAG 展开逻辑（`tree_map` 缓存 + `insert_pos_delta` 拷贝）与打印逻辑交织在一起。
  - `expr_as_string` 中的字符串常量折叠（将 `i8` 数组表达式折叠为 `c"..."` 字面量）。
- **建议深挖方向**：
  1. 能否将 `IRPrinter` 与 `IRTreeBuilder` 分离？分离后增量更新是否更容易实现？
  2. `tree_map` 的缓存命中率和拷贝开销的量化分析。
  3. `fmt_use` 中的 `begin_byte..end_byte` 源码片段复用机制：如果格式化文本中包含非打印字符或换行，片段复用是否安全？
  4. 所有指令类型的格式化输出与 remusys-ir 的 `Display` 实现是否一致？

### 4.3 IRRename 的全量更新 vs 增量更新（高优先级）

- **锚点文件**：`src/module/rename.rs`（~213 行）
- **当前状态**：重命名成功后调用 `full_update()`，重建整个 `IRTree` 和 `SourceBuf`。前端也需要废弃所有缓存。
- **复杂性来源**：
  - 名称变更会影响源码文本长度，从而改变所有后续结点的 `pos_delta`。
  - 如果新名称更长/更短，同一行内所有后续结点的相对位置都需要更新。
  - 全局对象重命名还可能影响导出符号表和跨函数引用。
- **建议深挖方向**：
  1. `full_update` 的完整调用链和性能瓶颈分析（对大模块的耗时）。
  2. 增量更新的可行性：如果只改局部名称，能否只更新该函数对应的子树？
  3. 如果引入增量更新，`SourceBuf::replace` 的文本替换机制是否可以被利用？
  4. 重命名冲突检测（`GlobalNameConflict` / `LocalNameConflict`）的边界条件。

### 4.4 SourceBuf 的坐标转换与文本替换（中优先级）

- **锚点文件**：`src/module/source_buf.rs`（~408 行）
- **当前状态**：支持 `monaco_pos_to_byte` / `byte_pos_to_monaco` 双向转换，以及 `replace(range, text)` 跨行文本替换。
- **复杂性来源**：
  - UTF-8 字节偏移 <-> UTF-16 code unit 的转换涉及多字节字符和代理对。
  - `replace` 操作需要处理单行替换、跨行替换、追加到末尾等多种情况。
- **建议深挖方向**：
  1. 边界测试用例设计：emoji（代理对）、中文（3-byte UTF-8）、混合文本的坐标转换。
  2. `replace` 后，`SourceLine` 的 `is_ascii` 标记是否正确更新？
  3. 坐标转换在频繁调用时的性能（Monaco 光标移动可能高频触发 `path_of_srcpos`）。
  4. `replace` 是否被当前主流程使用？如果没有，其测试覆盖是否足够？

### 4.5 BlockDFG 的分节算法与 b2 规则（中优先级）

- **锚点文件**：`src/dto/dfg.rs`（~498 行）
- **当前状态**：`BlockDfgBuilder` 按指令副作用角色分节，并通过 `b2 规则`处理跨块操作数。
- **复杂性来源**：
  - `calls_pure` 的判定：通过 callee 的 `attrs().is_func_pure()` 决定 `Call` 是 `Pure` 还是 `Effect`。如果 callee 是间接调用（非 `Global`），默认返回 `false`（视为 `Effect`）。
  - `b2 规则` 的语义：为什么只有 `Inst` 和 `FuncArg` 放入 `Income` 节？常量、表达式、全局变量为什么不放入 `Income`？
- **建议深挖方向**：
  1. 分节算法的正确性：是否存在某种指令序列导致分节结果不符合执行顺序？
  2. `calls_pure` 对间接调用的保守处理是否合理？是否有误判导致 `Pure` 被错误标记为 `Effect`？
  3. `Income` 节的全局去重是否会导致可视化中跨块数据流信息丢失？
  4. `BlockDfg::to_dot_text` 的 DOT 输出是否可用于自动化回归测试？

### 4.6 CfgEdgeDfsRole 的边分类算法（中优先级）

- **锚点文件**：`src/dto/cfg.rs`（~153 行）
- **当前状态**：`EdgeRoleJudge` 通过前序和后序 DFS 序列判断 CFG 边的角色（Tree/Back/Forward/Cross/SelfRing）。
- **复杂性来源**：
  - 标准 DFS 边分类通常基于单个 DFS 树。这里使用**前序+后序双重索引**。
  - `to_is_ancestor_of_from` 的判断逻辑：`to_pre < from_pre && to_post > from_post`。
- **建议深挖方向**：
  1. 该分类算法与标准图论教材（CLRS 等）中的定义是否等价？
  2. 对于不可规约流图（irreducible CFG），该分类是否仍然成立？
  3. `SelfRing` 的判断（`from == to`）是否足够？是否存在多基本块自环的边界情况？

### 4.7 serde-wasm-bindgen 的类型桥接与性能（中优先级）

- **锚点文件**：`src/types.rs`、`src/module.rs` 中的 `serialize`/`deserialize` 方法
- **当前状态**：所有 DTO 通过 `serde-wasm-bindgen` 直接转 JS Object，`serialize_maps_as_objects(true)`。
- **复杂性来源**：
  - `serde-wasm-bindgen` 的性能特征不如原生 JSON.parse（对于某些场景）。
  - DTO 中大量使用 `SmolStr`、枚举 tagged union、嵌套结构。
  - 大图的序列化可能产生大量的临时 JS Object，对 GC 造成压力。
- **建议深挖方向**：
  1. `CallGraphDt` / `FuncCfgDt` 等典型 DTO 的序列化耗时测量。
  2. 是否可以通过 `wasm-bindgen` 的 `#[wasm_bindgen]` 结构体（而非 serde）来减少转换开销？
  3. `serialize_maps_as_objects(true)` 对 hashmap 类结构的影响。

### 4.8 IRTreeCursor 的设计意图与使用模式（中优先级）

- **锚点文件**：`src/tree.rs`（第 922 行之后）
- **当前状态**：`IRTreeCursor` 是一个状态ful 的游标结构体，持有 `module_id`、`node_path` 和累加后的 `source_range`。它**被前端活跃使用**于树加载和结点有效性验证，灵活性比直接调用 `ModuleInfo` API 更强。
- **设计意图**：`IRTreeCursor` 是对 `IRTreeNodePathBuf` 的"有状态包装"，前端可以在不频繁与 WASM 往返通信的情况下维护树导航状态。`source_range` 字段内部完成了从根到当前结点的 `pos_delta` 累加，避免了前端重复计算。
- **建议深挖方向**：
  1. 前端具体的创建、移动和销毁模式是什么？与 Zustand Store 如何协作？
  2. 相比 `ModuleInfo::path_get_node`/`ir_tree_get_children`，Cursor 减少了哪些状态管理负担？
  3. 是否需要扩展 Cursor 的能力（如上移、兄弟导航）？

### 4.9 fmt_jserr! 宏的跨平台一致性与错误处理策略（低优先级）

- **锚点文件**：`src/lib.rs`、`src/module/rename.rs`
- **当前状态**：`fmt_jserr!` 的 panic 分支是**刻意设计**。`wasm_bindgen::JsError` 只在 `wasm32-unknown-unknown + JS` 环境下可用，非 WASM target 根本没有这个类型，不用 `panic!` 替代的话测试连编译都过不了。
- **核心语义**：`JsError` 在设计上等价于 **throw**，仅用于**不可恢复错误**（如内部状态不一致、ID 解析失败）。可恢复错误（如重命名冲突）应走 `Ok` 分支返回结构化枚举（如 `RenameRes`）。这是因为 JS 的类型系统太动态，`try-catch` 里不能按类型 catch，可恢复错误走 `JsError` 会在前端失控。
- **建议深挖方向**：
  1. 是否存在可恢复错误被误用为 `fmt_jserr!`（不可恢复）的场景？
  2. `JsError` 的 `message` 格式是否对前端错误处理友好？
  3. 前端如何区分 "程序错误（需修复）" 和 "用户错误（需提示）"？

### 4.10 测试覆盖与自动化（低优先级）

- **锚点文件**：`src/module/source_buf.rs`（底部有单元测试）、`src/tree/testing.rs`、`src/dto/testing.rs`
- **当前状态**：`source_buf.rs` 有 3 个单元测试。`tree/testing.rs` 和 `dto/testing.rs` 似乎为预留文件。
- **建议深挖方向**：
  1. `tree/builder.rs` 的格式化输出如何测试？是否需要 snapshot testing（如 `insta` crate）？
  2. WASM 绑定的测试策略：是否使用 `wasm-bindgen-test`？
  3. 图算法（CFG、DomTree）的测试用例设计。

---

## 5. 设计约束与假设

| 约束/假设 | 说明 | 影响 |
|-----------|------|------|
| **浏览器单线程 WASM** | 无并发，无需考虑 `Send/Sync` 之外的并发问题。 | `RefCell` 在 `IRTreeInner` 中的使用是安全的。 |
| **Monaco Editor 坐标系** | 1-based 行号，UTF-16 code unit 列号。 | Rust 侧需全程维护字节坐标，仅在 API 边界转换。 |
| **前端状态缓存** | 假设前端（Zustand）会缓存 ModuleInfo 和树路径。 | `rename` 全量重建后，前端必须主动废弃缓存。 |
| **IR Tree 的规模** | 假设编译单元不会特别大（教学/毕设场景）。 | 全量重建和 DAG 展开在规模可控时可接受。 |
| **WASM 内存不共享** | Rust 和 JS 之间通过拷贝传递数据。 | 大图 DTO 序列化是性能敏感点。 |
| **remusys-ir 的稳定性** | 假设下游 crate 的 IR 结构稳定。 | IRTreeBuilder 与 IR 指令集强耦合，IR 变更需同步修改。 |

---

## 6. 术语表

| 术语 | 解释 |
|------|------|
| **IR Tree** | remusys-wasm 内部维护的源码关联树，将 IR 对象映射到格式化源码中的位置。 |
| **DAG 展开** | 将 IR 中共享的操作数（UseID）在 IR Tree 中复制为独立树结点的过程。 |
| **pos_delta** | 树结点相对于父结点的源码范围偏移，采用相对坐标以减少更新连锁。 |
| **DTO** | Data Transfer Object，用于 WASM <-> JS 序列化的数据结构（如 `FuncCfgDt`）。 |
| **b2 规则** | BlockDFG 中处理跨块操作数的规则：`Inst`/`FuncArg` 放入 `Income` 节并去重，其他操作数放在 user 所在节。 |
| **全量更新** | 重命名后重建整个 `IRTree` + `SourceBuf`，而非局部修改。 |
| **GenIndex** | `mtb-entity-slab` 的分代索引，高 16-bit 为 generation，低 48-bit 为 index。 |
| **MonacoSrcPos** | Monaco Editor 的坐标，{ line: 1-based, column: 1-based UTF-16 }。 |
| **SourcePosIndex** | Rust 内部坐标，{ line: 0-based, col_byte: 0-based 字节偏移 }。 |
