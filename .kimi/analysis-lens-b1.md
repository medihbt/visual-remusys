# remusys-lens (b1) 设计分析

> 分析范围：`remusys-lens/src/` 下全部 `.ts` / `.tsx` 源码  
> 分析日期：2026-04-21  
> 分支：`feature/rename`（b1 为原始版本，与当前 dirty 的 b2 区分）

---

## 总体设计

### 1. 项目定位
`remusys-lens` 是 **Visual Remusys** 的前端可视化层（第一代，代号 b1），基于：
- **React 19 + TypeScript + Vite**（SWC 插件）
- **React Flow (xyflow)** 用于图可视化
- **Zustand + Immer** 做全局状态管理
- **Monaco Editor (@monaco-editor/react)** 做源码只读展示
- **Graphviz (viz-js)** 做图布局引擎
- **dagre** 做 Guide View 树布局
- **remusys-wasm**（Rust/WASM 包）做 IR 解析与查询后端

### 2. 目录结构与模块划分

```
src/
├── main.tsx                 # 入口，StrictMode 挂载 App
├── App.tsx                  # 顶层布局：左右分栏 (react-reflex)
├── TopMenu.tsx              # 顶部菜单栏（文件/帮助）
├── FileLoader.tsx           # 首屏文件加载（拖拽+点击）
├── file-load.tsx            # 文件读取与类型识别工具
├── index.css / App.css      # 全局样式
│
├── ir/                      # IR 数据层 + 全局状态
│   ├── ir.ts                # IR 类型定义 + WASM API 封装
│   └── ir-state.ts          # Zustand Store (IRStore) + ModuleCache
│
├── editor/                  # 源码编辑器视图
│   ├── LensViewer.tsx       # Monaco Editor 封装，focus 高亮
│   └── llvmMonarch.ts       # LLVM IR Monarch 语法定义
│
├── guide-view/              # 左侧下方导航树（Module → Func → Block → Inst）
│   ├── GuideView.tsx        # 树形视图主组件（React Flow + dagre）
│   ├── guide-view-tree.ts   # 树节点模型、TreeNodeStorage（展开/折叠逻辑）
│   ├── types.ts             # GuideView 专用类型（NavEvent、GuideRFNode 等）
│   ├── GuideContext.tsx     # React Context（未实际使用）
│   └── components/
│       ├── GuideNodeComp.tsx   # 单个树节点 UI
│       ├── ChildRow.tsx        # 子节点行
│       ├── SimpleMenu.tsx      # 右键/菜单弹出层
│       └── TypeIcon.tsx        # 节点类型图标
│
└── flow/                    # 右侧可视化图（CFG/DFG/CallGraph/支配树）
    ├── FlowViewer.tsx       # 图视图主组件（React Flow + Graphviz 布局）
    ├── flow-stat.ts         # Flow 图类型 Zustand Store
    ├── graphviz-object.ts   # Graphviz JSON 输出类型定义
    └── components/
    │   ├── Node.tsx         # React Flow 节点渲染（elemNode / groupNode）
    │   ├── Edge.tsx         # React Flow 边渲染（支持 SVG path、虚线、箭头）
    │   └── Toast.tsx        # 当前图类型提示浮层
    └── graphs/
        ├── layout.ts        # 图布局引擎：Graphviz 调用 + 坐标解析
        ├── cfg.ts           # CFG 图构建与渲染
        ├── dominance.ts     # 支配树构建与渲染
        ├── dfg.ts           # DFG / Def-Use 图构建与渲染
        └── callgraph.ts     # 调用图构建与渲染
```

### 3. 核心职责

| 模块 | 职责 |
|------|------|
| `ir` | 定义完整的 IR 类型系统（ID、Value、Inst、Block、Global、Type 等）；封装 `remusys-wasm` 的所有 API；提供 `ModuleCache` 做按需加载与本地缓存；提供 `useIRStore` 管理全局 IR 状态（编译、focus、源码映射刷新）。 |
| `editor` | 只读 Monaco 编辑器，根据 `focusInfo` 动态切换展示文本（模块概览 or 函数源码），并通过 Monaco `deltaDecorations` 实现高亮定位。 |
| `guide-view` | 以树形结构导航整个模块的层次：Module → Globals → Blocks → Insts。支持展开/折叠/聚焦/右键菜单。菜单可触发 CFG、DFG、Def-Use 等图切换。 |
| `flow` | 根据 `FlowGraphType` 渲染不同的图：CallGraph、FuncCfg、FuncDom、BlockDfg、DefUse。所有图节点坐标由 Graphviz `dot` 引擎计算，再转换为 React Flow 的 nodes/edges。 |
| `App` | 组合以上模块，使用 `react-reflex` 实现可拖拽的三栏布局：左上是编辑器、左下是导航树、右侧是图视图。 |

### 4. 数据流

```
用户上传文件
    │
    ▼
FileLoader / TopMenu ──text──▶ App.tsx ──▶ useIRStore.compileModule(ty, text)
    │                              │
    ▼                              ▼
ir-state.ts                 WASM Api (remusys-wasm)
    │                              │
    ▼                              ▼
ModuleCache (按需加载) ◀── JSON 数据
    │
    ├───▶ LensViewer ( Monaco 展示 sourceText / focusInfo )
    │
    ├───▶ GuideView ( TreeNodeStorage + dagre 布局 )
    │         │
    │         └── NavEvent ──▶ App.handleNavEvent ──┬──▶ useIRStore.focusOn(id)
    │                                               │        │
    │                                               │        └──▶ LensViewer 高亮
    │                                               │
    │                                               └──▶ useFlowStore.setGraphType(...)
    │                                                        │
    ▼                                                        ▼
FlowViewer ◀────────────────────────────────────────── 异步 renderGraph()
    │
    └── Graphviz(dot) 布局 → ReactFlow 渲染
```

### 5. 状态管理

- **`useIRStore`**（Zustand + Immer + Redux DevTools）
  - `module: ModuleCache | null`
  - `sourceKind / sourceText`
  - `status: idle | ready | error`
  - `focusedId / focusInfo / focusSince`
  - `revision`（手动递增，用于触发依赖组件重渲染）
  - Actions: `compileModule`, `attachModule`, `focusOn`, `clearFocus`, `loadGlobal`, `refreshModuleSourceMappings`, `refreshFuncSourceMappings`, `renameSymbol`（未实现）

- **`useFlowStore`**（Zustand + Immer，极轻量）
  - 仅保存 `graphType: FlowGraphType`
  - Actions: `setGraphType`, `restoreGraphType`

### 6. UI 组件层次

```
App (div.app-root)
├── TopMenu
├── ReflexContainer (vertical)
│   ├── ReflexElement (left panel, flex=40)
│   │   └── ReflexContainer (horizontal)
│   │       ├── ReflexElement (editor, flex=70)
│   │       │   └── LensViewer (Monaco)
│   │       ├── ReflexSplitter
│   │       └── ReflexElement (guide view, flex=30)
│   │           └── GuideView (ReactFlow)
│   ├── ReflexSplitter
│   └── ReflexElement (flow view, flex=60)
│       └── FlowViewer
│           ├── FlowGraph (ReactFlow)
│           └── FlowToast
└── (首屏无 module 时) FileLoader
```

### 7. 与 WASM 后端的交互方式

- 通过 `remusys-wasm` 包的 `Api` 对象直接调用 Rust 暴露的函数。
- 所有 IR 数据通过 **JSON 序列化** 传递（因此 `bigint` 被替换为 `string`，如 `I64`、`APInt`）。
- 前端采用 **惰性加载（Lazy Loading）** 策略：
  - 编译模块后仅获取 `ModuleGlobalsDt`（概览）。
  - 当用户在 GuideView 展开函数、基本块、指令时，`ModuleCache` 按需调用 `Api.load_global_obj`、`Api.load_func_of_scope` 等接口加载详细数据，并缓存到内部 `Map`。
  - 图的构建（CFG、DFG、CallGraph、支配树）也通过 WASM API 完成，前端只负责渲染。

---

## 详细设计（按模块分小节）

### 2.1 IR 类型系统 (`ir/ir.ts`)

#### 关键类型/接口
- **池分配 ID（PoolStrID）**：格式为 `{prefix}:{slot-index}:{generation}`，前缀区分全局对象 `g`、基本块 `b`、指令 `i`、表达式 `e`、Use `u`、JumpTarget `j`。使用 TypeScript Template Literal Types 强约束。
- **`SourceTrackable`**：可被追踪的 IR 实体，用于 focus、source loc 更新。包括 `Global`、`Block`、`Inst`、`Expr`、`Use`、`JumpTarget`、`FuncArg`、`Module`。
- **Value 系统**：`ValueDt` 为 discriminated union，涵盖立即数（I1/I8/I16/I32/I64/APInt/F32/F64）、引用值（Global/Inst/Expr/Block/FuncArg）、Undef、ZeroInit。
- **指令类型**：`NormalInstDt`、`TerminatorDt`（含 `succs: JumpTargetDt[]`）、`PhiInstDt`（含 `incomings`）。
- **图数据类型**：`FuncCfgDt`（`CfgNode` + `CfgEdge`）、`DomTreeDt`、`BlockDfgDt`（按 `BlockDfgSectionDt` 分节）、`CallGraphDt`。

#### IDCast 工具类
提供一系列静态方法将字符串断言为特定 ID 类型，如 `asGlobal`、`asBlock`、`asSourceTrackable` 等。所有方法均基于正则校验，运行时安全。

#### WASM API 封装
所有函数都是 `Api.xxx` 的薄封装：
- `irCompileModule` → `Api.compile_module`
- `irGetModuleGlobalsBrief` → `Api.get_globals_brief`
- `irLoadGlobalObj` → `Api.load_global_obj`
- `irMakeCfg` → `Api.make_func_cfg`
- `makeDominatorTree` → `Api.make_dominator_tree`
- `irMakeBlockDfg` → `Api.make_block_dfg`
- `irMakeCallGraph` → `Api.make_call_graph`
- `irUpdateFuncSource` / `irUpdateModuleOverviewSource` → 源码映射刷新

### 2.2 IR 状态管理 (`ir/ir-state.ts`)

#### ModuleCache
- **职责**：对单一模块的内存缓存，管理 `globals`、`blocks`、`insts`、`uses`、`jts` 五个 Map。
- **按需加载策略**：
  - `loadGlobal(id)`：先查缓存，缺失则调用 `irLoadGlobalObj`。
  - `loadBlock / loadInst / loadUse / loadJumpTarget`：通过 `_loadLocal` 统一实现，若缺失则调用 `irLoadFuncOfScope` 加载整个所属函数的所有局部数据（因为 WASM 侧目前以函数为单位返回完整局部信息）。
- **SourceLoc 查询**：`findSourceLoc(id)` 根据 ID 类型从对应缓存取 `source_loc`。
- **源码更新应用**：`applySourceUpdates(updates, maybeFunc)` 在重命名或源码映射刷新后，更新缓存中的 source text 和各对象的 `source_loc`，并清理被删除（eliminated）的对象。
- **辅助方法**：`getValueOperands`、`getValueUsers`、`valueGetName`、`typeGetName`、`makeBlockDfg`、`makeCallGraph`。

#### IRStore（Zustand）
- **`compileModule`**：调用 `ModuleCache.compileFrom`，成功则写入 store 并递增 `revision`。
- **`focusOn(id)`**：
  1. 确定 `scopeId`（所属函数 GlobalID 或 null）。
  2. 确定 `sourceText`（模块概览 or 函数源码）。
  3. 确定 `highlightLoc`（优先取对象自身 `source_loc`；若聚焦整个函数，则计算其所有 Block 的包围盒范围）。
  4. 写入 `focusInfo`。
- **`refreshModuleSourceMappings / refreshFuncSourceMappings`**：调用 WASM API 获取最新源码映射和 eliminated 列表，通过 `applySourceUpdates` 同步到缓存，并更新 Monaco 显示的 `sourceText`。
- **`renameSymbol`**：【**缺失/未完成**】直接抛出错误，等待 WASM 后端支持。

### 2.3 编辑器 (`editor/LensViewer.tsx`)

- **Monaco 配置**：
  - `readOnly: true`
  - 语言根据 `srcType` 动态切换：`"ir"` → `"llvm"`（自定义 Monarch），`"sysy"` → `"c"`（Monaco 内置）。
  - 首次挂载时注册 `llvm` Monarch tokenizer。
- **Focus 高亮逻辑**：
  - 监听 `focusInfo`、`revision`、`moduleOverview`。
  - 当 `focusInfo` 存在时，将编辑器内容设为 `focusInfo.sourceText`，并用 `deltaDecorations` 在 `highlightLoc` 范围添加 CSS class `ir-focus-decoration`。
  - 当 `focusInfo` 为 null 时，恢复显示 `moduleOverview`。
- **缺失**：当 `srcType === "sysy"` 时，语言设为 `"c"` 但没有做 C 语言的 Monarch 定制；如果 Monaco 没有内置 C 语言支持（某些精简版本），高亮会回退到 plain text。

### 2.4 GuideView 导航树 (`guide-view/`)

#### 树形结构模型 (`guide-view-tree.ts`)
- **`TreeNodeKind`**：Module、GlobalVar、ExternGlobalVar、Func、ExternFunc、Block、Inst、Phi、Terminator。
- **`GuideTreeNode`**：不可变树节点，含 `moduleId`、`selfId`（`SourceTrackable`）、`kind`、`parentId`、`childIds`、`label`、`sourceLoc`。
- **`TreeNodeStorage`**：
  - 内部用 `Map<PoolStrID, GuideTreeNode>` 存储非 Module 节点；Module 节点单独存放。
  - `expand(id, module)`：按需从 `ModuleCache` 加载数据并构造树节点。
    - Module → globals 列表
    - Global(Func) → blocks 列表
    - Global(GlobalVar) → 空
    - Block → insts 列表
    - Inst → 空（叶子）
  - `expandChildren` / `dfsExpand` / `collapse` / `collapseChildren`：控制展开深度。
  - `export(module)`：DFS 导出为 `Exported.NodesAndEdges`，只导出已展开节点，未展开节点以 `CollapsedNode` 形式出现在父节点的 `children` 数组中。导出过程中会清理 `nodesById`，只保留已展开节点。

#### 布局算法 (`GuideView.tsx` 中的 `renderTree`)
- 使用 **dagre**（`graphlib.Graph`）做 LR（从左到右）层次布局。
- 节点尺寸：宽度固定 `220px`，高度根据子节点数估算 `52 + children.length * 41`，上限 `300px`。
- 边为普通默认边，带 `arrowclosed` 箭头。
- 布局后坐标通过 `x - width/2, y - height/2` 转为 React Flow 的 top-left 坐标。

#### 节点组件 (`GuideNodeComp.tsx`)
- 每个节点是一个自定义 React Flow 节点，内部用 HTML/CSS 模拟一个卡片：
  - **顶栏**：图标（`TypeIcon`）+ 标签 + `⋯` 菜单按钮。双击顶栏触发 `onFocus`；右键或点击 `⋯` 触发 `onRequestMenu`。
  - **子节点列表**：展示 `data.children`（`ChildRow`），点击可展开/折叠。
  - 当节点与当前 `irStore.focusedId` / `focusInfo.scopeId` 匹配时，顶栏背景变为淡蓝色（`#eef2ff`），图标外围加蓝圈。

#### 导航事件 (`types.ts`)
- `NavEvent` 是 GuideView 与外部（App / FlowViewer）通信的通用协议：
  - `Focus`：聚焦到某 IR 实体，更新编辑器高亮。
  - `ExpandOne / ExpandAll / Collapse`：展开/折叠树节点（由 GuideView 自身处理，也回传 App 做日志或额外处理）。
  - `ShowCfg` / `ShowDominance` / `ShowDfg` / `ShowValueDefUse`：切换右侧 FlowViewer 的图类型。

#### 菜单 (`SimpleMenu.tsx`)
- 固定定位的弹出层，根据节点 `kind` 动态生成菜单项。
- 支持屏幕边缘防溢出（限制 left/top）。

### 2.5 Flow 可视化图 (`flow/`)

#### Flow 状态 (`flow-stat.ts`)
- 极简 Zustand Store，仅保存 `graphType: FlowGraphType`。
- `FlowGraphType` 为 discriminated union：
  - `Empty / Focus / CallGraph / ItemReference / FuncCfg / FuncDom / BlockDfg / DefUse`

#### FlowViewer 主组件 (`FlowViewer.tsx`)
- `FlowGraph` 内部维护 `nodes`、`edges` 两个 React state。
- 通过 `useEffect` 监听 `irStore`（module + focusInfo）和 `graphType` 变化，调用 `renderGraph()` 异步生成新图。
- **双击交互**：双击节点或边时，若其 `data.irObjID` 存在，则调用 `irStore.focusOn(...)`，实现“图 → 编辑器”的反向定位。
- `Focus` 模式的行为：
  - 若当前 focus 无函数 scope → 展示 CallGraph。
  - 若有函数 scope → 展示该函数的 CFG，并自动高亮当前 focus 的 Block / Edge。

#### 图布局引擎 (`graphs/layout.ts`)
- **核心依赖**：`@viz-js/viz`（WebAssembly 版 Graphviz）。
- **`layoutSimpleFlow`**：
  1. 将 `FlowElemNode[]` + `FlowEdge[]` 构造为 `Viz.Graph`（DOT 的 JS 对象表示）。
  2. 节点尺寸按像素转英寸（96 DPI），并乘 `1.25` 作为 padding。
  3. 调用 `viz.renderJSON(dot, { engine: "dot", format: "json0" })`。
  4. `decodeSimpleLayout` 解析 JSON：
     - 坐标系翻转：Graphviz 原点是左下角，转为左上角（`y = bb.yMax - rawY`）。
     - 节点中心坐标转 top-left（减去 half width/height）。
     - 边解析 `_draw_` / `_hdraw_` / `_tdraw_` / `pos` 等 draw ops，转换为 SVG path string（支持 B-spline → Bézier 平滑）。
     - 边标签位置从 `_ldraw_` 的 Text op 提取。
- **`layoutSectionFlow`**（用于 Block DFG）：
  1. 输入 `SectionFlowGraph`，每个 section 转为 Graphviz 的 `subgraph`（`cluster_` 前缀）。
  2. 根据 section kind 设置 `rank`：Income → `source`（顶部），Outcome → `sink`（底部），Pure/Effect → 中间递增值。
  3. Effect section 内部添加隐形边（`style: "invis"`）强制垂直顺序。
  4. 调用 Graphviz 布局后，通过 `decodeSimpleLayout` 得到平面坐标。
  5. `createGroupedLayout`：
     - 为每个 section 计算包围盒，创建 `FlowGroupNode`（React Flow 分组节点）。
     - 子节点坐标转为相对于 group 的坐标，设置 `parentId` 和 `extent: "parent"`。
     - Pure / Effect 分组内的节点可拖拽（`draggable: true`），Income / Outcome 不可拖拽。

#### 图类型实现

##### CFG (`graphs/cfg.ts`)
- `makeCfg`：调用 WASM `irMakeCfg`。
- `renderCfgToFlow`：
  - 节点：Entry（绿底 `#d1fae5`）、Exit（红底 `#fee2e2`）、普通（白底）。
  - 边：按 `edge_class` 设置虚线样式：
    - `Unreachable` → `2 2` 虚线，灰色
    - `Cross` → `4 4` 虚线
    - `Back` → `6 3` 虚线 + 实线叠加（`dashAndLine`）
    - `Forward` → `4 2` 虚线 + 实线叠加
  - 边颜色按 `JTKind` 区分：`Jump` 黑、`BrThen` 绿、`BrElse` 红、`SwitchDefault` 蓝、`SwitchCase` 橙。

##### 支配树 (`graphs/dominance.ts`)
- 调用 WASM `makeDominatorTree`。
- 简单映射为 nodes/edges，白色背景，无特殊样式。

##### DFG / Def-Use (`graphs/dfg.ts`)
- **`DfgBuilder`**：
  - `buildFromCentered(centerValue, module)`：构建以某个 Value 为中心的局部 def-use 子图。
  - 中心节点标记为 `Focused`，其 operand users 标记为 `Outcome`，其定义来源标记为 `Income`。
  - 节点 ID 复用 IR 的池分配 ID（`Inst` / `Global` / `Block` / `Expr`）或生成 `FuncArg(...)` 字符串。
- **`renderDfgInsideBlock(blockID, module)`**：
  - 调用 WASM `makeBlockDfg`，获取按 section（Income / Pure / Effect / Outcome）分组的节点和边。
  - 将每个 section 转为 `FlowSection`，边按 `section_id` 分为内部边或跨节边。
  - 调用 `layoutSectionFlow` 进行分组布局。
- **`renderDfgFromCentered(centerValue, module)`**：
  - 使用 `DfgBuilder.buildFromCentered`，然后走 `layoutSimpleFlow`（无分组）。

##### CallGraph (`graphs/callgraph.ts`)
- 调用 WASM `makeCallGraph`。
- 节点按 `CallNodeRole` 着色：Root（浅粉）、Live（浅绿）、Indirect（浅青）、Unreachable（浅灰）。
- 节点标签使用 React element 包装，方便设置文字颜色。

#### 节点与边组件 (`components/Node.tsx` / `Edge.tsx`)
- **Node**：
  - `elemNode`：普通矩形节点，带上下 Handle，背景色由 `data.bgColor` 决定，聚焦时边框加深。
  - `groupNode`：分组容器，仅展示标签，无边框 Handle。
- **Edge**：
  - 完全自定义 SVG 渲染（`FlowEdgeComp`）。
  - 支持 `mainPaths`（主线条，可虚线）、`arrowPaths`（箭头填充）、`dashAndLine`（虚线+细实线叠加，用于 Back/Forward 边）、标签文字定位。
  - 聚焦时线宽加粗到 `1.5`。

### 2.6 文件加载 (`FileLoader.tsx` / `file-load.tsx`)
- 支持 **拖拽上传** 和 **点击选择文件**。
- 文件类型识别：
  - `.ll` / `.ir` / `.remusys-ir` → `SourceTy = "ir"`
  - `.sy` / `.sysy` → `SourceTy = "sysy"`
- 通过 `FileReader.readAsText` 读取后，回调给 `onLoad(mode, text)`。

---

## 缺失/未完成部分

1. **保存功能**  
   `TopMenu.tsx` 的 `save()` 函数仅有 `alert("等待实现: 保存当前存档到浏览器...")`，没有任何序列化或下载逻辑。

2. **重命名（Rename）**  
   `ir-state.ts` 中的 `renameSymbol(id, newName)` 直接抛出 `Error("Renaming(...) not supported yet: waiting for WASM")`，WASM 后端尚未暴露重命名 API。

3. **ItemReference 图**  
   `FlowViewer.tsx` 的 `FlowGraphType` 包含 `{ type: "ItemReference"; item: GlobalID }`，但 `renderGraph` 中直接返回 `todoNodes("ItemReference")`，未实现。

4. **GuideContext 未使用**  
   `guide-view/GuideContext.tsx` 定义了 `GuideContext`，但项目中没有任何组件 `useContext(GuideContext)`，属于遗留代码。

5. **`utils` 目录为空**  
   `src/utils/` 存在但没有任何文件，说明通用工具函数目前散落在各模块中（如 `sourceTrackableToString` 在 `ir.ts`，`normalizeError` 在 `ir-state.ts`）。

6. **SysY 源码高亮**  
   `LensViewer.tsx` 在 `srcType === "sysy"` 时将语言设为 `"c"`，但没有注册 C 语言的 Monarch grammar。如果 Monaco 的运行时环境缺少 C 语言支持，则 SysY 文件将无高亮。

7. **NavEvent 的 Expand/Collapse 在 App 层仅为日志**  
   `App.tsx` 的 `handleNavEvent` 对 `ExpandOne` / `ExpandAll` / `Collapse` 仅做 `console.debug`，未执行实质性操作（实际由 `GuideView` 内部通过 `incomingNavEvent` 处理）。虽然功能未缺失，但事件分发逻辑存在冗余。

8. **DefUse 图（以 Value 为中心）缺少分组**  
   `renderDfgFromCentered` 使用 `layoutSimpleFlow`，节点平铺，未像 `BlockDfg` 那样按 Income/Outcome 分组。

9. **FlowToast 关闭按钮样式**  
   `FlowToast` 的关闭按钮使用内联 `onMouseEnter/Leave` 修改 style，未使用 CSS class，属于实现细节层面的粗糙。

10. **缺少测试文件**  
    整个 `remusys-lens` 项目中没有任何 `.test.ts`、`.spec.ts` 或测试配置，全部逻辑依赖手动测试。

