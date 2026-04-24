# Visual Remusys 项目架构分析报告

## 一、项目概述

**Visual Remusys** 是一个基于 Web 的 Remusys-IR（一种类 LLVM 的中间表示）可视化工具，作为毕业设计项目，用于展示 IR 内存结构、优化器可视化和变换规则可视化。

### 技术栈
- **前端**: React + TypeScript + Vite
- **后端/核心**: Rust + WebAssembly (wasm-bindgen)
- **可视化**: React Flow (xyflow) + Dagre 布局 + Monaco Editor
- **状态管理**: Zustand + Immer

---

## 二、模块组织结构

```
visual-remusys/
├── Cargo.toml              # Rust Workspace 定义
├── remusys-ir/             # 【核心】IR 基础库
├── remusys-ir-parser/      # IR 文本解析器
├── remusys-lang/           # SysY 语言前端
├── remusys-wasm/           # 【重点】WASM 绑定层
├── remusys-lens/           # 【重点】前端可视化应用
└── src/                    # (几乎为空，仅 main.rs)
```

### 依赖关系图

```
remusys-lens (前端应用)
    │
    ├── remusys-wasm (WASM 包)
    │       │
    │       ├── remusys-ir-parser ──→ remusys-ir
    │       ├── remusys-lang ───────→ remusys-ir
    │       └── remusys-ir (核心 IR 库)
    │
    └── 外部依赖: React, React Flow, Monaco, Zustand...
```

---

## 三、各模块详细设计

### 1. remusys-ir（核心 IR 库）

**定位**: 提供 LLVM-like 的中间表示数据结构和管理机制

**核心模块**:

| 子模块 | 职责 |
|--------|------|
| `ir::module` | Module 结构体，管理全局符号池、内存分配器、类型上下文 |
| `ir::global` | 全局对象（函数、全局变量）定义 |
| `ir::block` | 基本块（BasicBlock）管理 |
| `ir::inst` | 指令系统（各类指令的定义和属性） |
| `ir::usedef` | Use-Def 链管理（SSA 形式的核心） |
| `ir::constant` | 常量表达式系统 |
| `ir::jumping` | 跳转目标（JumpTarget）和终止指令 |
| `ir::utils` | 序列化、克隆、构建器等工具 |
| `opt` | 优化分析（CFG、支配树、DFG、活跃区间） |
| `typing` | 类型系统 |
| `base` | 基础数据结构（APInt、SlabID、混合引用等） |

**关键设计**:

```rust
// Module: 核心容器
pub struct Module {
    pub allocs: IRAllocs,      // 内存分配器池
    pub tctx: TypeContext,     // 类型上下文
    pub symbols: RefCell<SymbolPool>, // 符号池
    pub name: String,
}

// ValueSSA: SSA 值类型枚举
pub enum ValueSSA {
    None,
    ConstData(ConstData),
    ConstExpr(ExprID),
    FuncArg(FuncID, u32),
    Block(BlockID),
    Inst(InstID),
    Global(GlobalID),
}

// IRAllocs: 统一的内存池分配器
pub struct IRAllocs {
    pub globals: Slab<GlobalObj>,
    pub blocks: Slab<BlockObj>,
    pub insts: Slab<InstObj>,
    pub uses: Slab<Use>,
    pub exprs: Slab<ExprObj>,
    pub jts: Slab<JumpTarget>,
}
```

---

### 2. remusys-ir-parser（IR 解析器）

**定位**: 将文本形式的 IR（类似 LLVM IR）解析为内存结构

**模块结构**:
- `parser`: 基于 Logos 的词法分析 + 手写递归下降语法分析
- `ast`: 抽象语法树定义
- `irgen`: AST 到 IR 的转换
- `sema`: 语义分析
- `mapping`: 源映射管理

**入口函数**:
```rust
pub fn source_to_full_ir(source: &str) -> Result<ModuleWithInfo, CompileErr>
```

---

### 3. remusys-lang（SysY 语言前端）

**定位**: 将 SysY（C 语言子集）源代码编译为 Remusys-IR

**模块结构**:
- `grammar`: LALRPOP 生成的语法分析器
- `ast`: SysY AST 定义
- `sema`: 语义分析
- `irgen`: (条件编译) IR 生成（需启用 `remusys-ir-integration` 特性）

**关键入口**:
```rust
pub fn translate_sysy_text_into_full_ir(
    source: &str
) -> Result<ModuleInfo, Box<dyn Error>>
```

---

### 4. remusys-wasm（WASM 绑定层）⭐重点

**定位**: 将 Rust IR 库暴露给 JavaScript，提供 WASM API

**模块结构**:

| 文件 | 职责 |
|------|------|
| `lib.rs` | WASM API 入口，暴露 `Api` 结构体 |
| `dto.rs` | 数据传输对象（DTO）定义，用于序列化 |
| `module.rs` | `ModuleInfo` 管理，模块生命周期 |
| `mapping.rs` | IR 到 DTO 的映射 |
| `rename.rs` | 重命名功能 |
| `graphs/cfg.rs` | 控制流图（CFG）和支配树生成 |
| `graphs/call_graph.rs` | 调用图生成 |
| `graphs/dfg.rs` | 数据流图（DFG）生成 |
| `module/source_tree.rs` | 源树结构 |
| `module/source_tree_builder.rs` | 源树构建 |

**核心 API 设计**:

```rust
#[wasm_bindgen]
impl Api {
    // 编译模块
    pub fn compile_module(source_ty: &str, source: &str) -> Result<JsValue, JsError>
    
    // 获取全局对象摘要
    pub fn get_globals_brief(id: &str) -> Result<JsValue, JsError>
    
    // 加载特定全局对象
    pub fn load_global_obj(id: &str, global_id: &str) -> Result<JsValue, JsError>
    
    // 图生成
    pub fn make_func_cfg(module_id: &str, func_id: &str) -> Result<JsValue, JsError>
    pub fn make_dominator_tree(module_id: &str, func_id: &str) -> Result<JsValue, JsError>
    pub fn make_block_dfg(module_id: &str, block_id: &str) -> Result<JsValue, JsError>
    pub fn make_call_graph(module_id: &str) -> Result<JsValue, JsError>
    
    // 源码映射更新
    pub fn update_func_src(id: &str, func_id: &str) -> Result<JsValue, JsError>
}
```

**模块管理机制**:
```rust
thread_local! {
    static MODULES: RefCell<HashMap<SmolStr, ModuleInfo>> = RefCell::new(HashMap::new());
}

pub struct ModuleInfo {
    pub module: Box<Module>,
    pub names: IRNameMap,
    pub overview: RefCell<Option<Rc<OverviewInfo>>>,
}
```

**支持的图类型**:

1. **CFG (Control Flow Graph)**: 函数的控制流图，含边分类（Tree/Back/Forward/Cross）
2. **Dominator Tree**: 支配树，用于分析控制依赖
3. **DFG (Data Flow Graph)**: 基本块内的数据流，按 Pure/Effect/Income/Outcome 分区
4. **Call Graph**: 模块级函数调用关系图

---

### 5. remusys-lens（前端可视化应用）⭐重点

**定位**: 基于 React 的交互式 IR 可视化界面

**目录结构**:

```
src/
├── App.tsx                 # 主布局（Reflex 分栏）
├── main.tsx                # 应用入口
├── FileLoader.tsx          # 文件上传组件
├── TopMenu.tsx             # 顶部菜单
├── ir/
│   ├── ir.ts              # IR 类型定义 + WASM API 封装
│   └── ir-state.ts        # Zustand 状态管理 + ModuleCache
├── editor/
│   ├── LensViewer.tsx     # Monaco Editor IR 代码展示
│   └── llvmMonarch.ts     # IR 语法高亮定义
├── guide-view/            # 【左侧下方面板】导航树
│   ├── GuideView.tsx      # React Flow 树形导航
│   ├── GuideContext.tsx   # 导航上下文
│   ├── guide-view-tree.ts # 树节点数据结构和操作
│   └── components/        # 节点组件、菜单
├── flow/                  # 【右侧面板】图形可视化
│   ├── FlowViewer.tsx     # 主流程图容器
│   ├── flow-stat.ts       # Flow 状态管理
│   ├── components/        # 节点/边/Toast 组件
│   └── graphs/            # 各类图的渲染逻辑
│       ├── cfg.ts         # CFG 渲染
│       ├── dominance.ts   # 支配树渲染
│       ├── dfg.ts         # DFG 渲染
│       └── callgraph.ts   # 调用图渲染
└── utils/                 # 工具函数
```

**状态管理架构**:

```
┌─────────────────────────────────────────────────────────────┐
│                     Zustand Store                           │
├─────────────────────────────────────────────────────────────┤
│  IR Store (ir-state.ts)        │  Flow Store (flow-stat.ts) │
│  ─────────────────────         │  ─────────────────────     │
│  - module: ModuleCache         │  - graphType: FlowGraphType│
│  - sourceText: string          │  - layoutConfig            │
│  - focusedId: SourceTrackable  │                            │
│  - focusInfo: FocusSourceInfo  │                            │
│  - status/error/revision       │                            │
└─────────────────────────────────────────────────────────────┘
```

**ModuleCache 设计**:
```typescript
class ModuleCache {
  readonly moduleId: ModuleID;
  globals: Map<GlobalID, GlobalObjDt>;  // 全局对象缓存
  blocks: Map<BlockID, BlockDt>;        // 基本块缓存
  insts: Map<InstID, InstDt>;           // 指令缓存
  uses: Map<UseID, UseDt>;              // Use 缓存
  jts: Map<JumpTargetID, JumpTargetDt>; // 跳转目标缓存
  
  // 按需加载
  loadGlobal(id: GlobalID): GlobalObjDt
  loadBlock(id: BlockID): BlockDt
  loadInst(id: InstID): InstDt
  // ...
}
```

**UI 布局**:

```
┌─────────────────────────────────────────────────────────────┐
│                      TopMenu                                │
├──────────────────────────┬──────────────────────────────────┤
│    LensViewer (Monaco)   │                                  │
│    [IR 代码只读展示]      │      FlowViewer (React Flow)     │
│                          │      [CFG/DFG/CallGraph]         │
├──────────────────────────┤                                  │
│    GuideView (React Flow)│                                  │
│    [树形导航]             │                                  │
│    - Module              │                                  │
│      - GlobalVar         │                                  │
│      - Func              │                                  │
│        - Block           │                                  │
│          - Inst          │                                  │
└──────────────────────────┴──────────────────────────────────┘
       左侧面板 (40%)              右侧面板 (60%)
```

**交互设计**:

1. **GuideView（导航树）**:
   - 可展开/折叠的树形结构
   - 右击菜单：展开一层、展开全部、折叠、聚焦、显示 CFG/DFG
   - 点击聚焦：在 LensViewer 中高亮对应源码位置
   - 双击：FlowViewer 显示对应图形

2. **FlowViewer（流程图）**:
   - 支持多种图类型：CallGraph、CFG、Dominance Tree、Block DFG
   - 双击节点/边：触发聚焦到对应 IR 实体
   - 使用 Dagre 自动布局

3. **LensViewer（代码编辑器）**:
   - Monaco Editor 只读模式
   - 支持 IR 语法高亮
   - 根据 Focus 状态高亮对应代码区间

**ID 系统设计**:

前端使用字符串格式的池分配 ID：
```typescript
type GlobalID = `g:${string}:${string}`;      // g:<index>:<generation>
type BlockID = `b:${string}:${string}`;
type InstID = `i:${string}:${string}`;
type UseID = `u:${string}:${string}`;
// ...
```

---

## 四、数据流架构

```
1. 用户上传/输入源码
        ↓
2. remusys-wasm::Api::compile_module()
   - SysY → remusys-lang → IR
   - IR 文本 → remusys-ir-parser → IR
        ↓
3. ModuleCache 存储在 Zustand Store
        ↓
4. GuideView 加载 Module 摘要，按需展开树节点
        ↓
5. 用户交互触发
   - 聚焦 → LensViewer 高亮源码
   - 显示图 → FlowViewer 渲染 CFG/DFG/...
        ↓
6. 图数据通过 WASM API 实时计算
   - make_func_cfg()
   - make_dominator_tree()
   - make_block_dfg()
   - make_call_graph()
```

---

## 五、关键技术亮点

1. **池分配 ID 系统**: 使用代际计数器（generation）实现安全的内存管理，支持垃圾回收
2. **Source Mapping**: IR 实体与源码位置的精确映射，支持双向导航
3. **按需加载**: ModuleCache 实现惰性加载策略，优化大模块性能
4. **分区 DFG**: 数据流图按 Pure/Effect/Income/Outcome 分区，清晰展示数据依赖和控制依赖
5. **边分类算法**: CFG 边自动分类为 Tree/Back/Forward/Cross，帮助识别循环结构
6. **Use-Def 链**: 完整的 SSA Use-Def 链支持，用于数据流分析