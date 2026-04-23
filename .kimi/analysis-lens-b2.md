# remusys-lens-b2 设计分析

> 分析日期：2026-04-23
> 分析范围：`remusys-lens-b2/src/` 全部源码 + 配置 + WASM 类型接口

---

## 总体设计

### 项目定位
`remusys-lens-b2` 是 **Visual Remusys** 的第二代前端可视化工具，用于展示和交互式导航 Remusys IR（中间表示）。与第一代（b1）最大的架构区别在于：**b2 彻底放弃了前端 IR 实体缓存**，所有 IR 数据均通过 WASM 后端按需拉取，前端仅保留最小化的 UI 状态（如展开态、焦点路径）。

### 技术栈
- **框架**：React 19 + TypeScript 6
- **构建工具**：Vite 8 + `vite-plugin-wasm`（WASM 集成）
- **状态管理**：Zustand 5 + Immer（不可变更新）+ Redux DevTools
- **UI 组件**：
  - `@headlessui/react` — 无样式可访问组件（Menu 等）
  - `@monaco-editor/react` — Monaco 编辑器（当前为占位）
  - `react-reflex` — 可拖拽分割面板
  - `@xyflow/react` — React Flow 图引擎（用于 GuideView 树 + FlowViewer 图视图）
- **图布局**：
  - `dagre` — 单层图布局（GuideView 导航树、CFG/CallGraph/DomTree）
  - `elkjs` — 子图嵌套布局（BlockDfg，计划中）
- **WASM 后端**：`remusys-wasm-b2`（通过 `wasm-pack` 构建为 bundler target）

### 模块划分
```
src/
├── main.tsx              # React 根入口
├── index.css / App.css   # 全局 + 应用级样式
├── App.tsx               # 顶层布局：FileLoader → MainPage
├── AppMenu.tsx           # 顶部菜单栏（文件/帮助）
├── FileLoader.tsx        # 初始文件加载页面（拖拽/点击上传）
├── file-load.tsx         # 文件读取与类型推断逻辑
├── ir/
│   └── state.ts          # IR 全局状态（Zustand）：编译、焦点、WASM API 封装
├── guide_view/           # 左侧导航树（React Flow 实现）
│   ├── guide-view-tree.ts # 树控制器：展开/收起/焦点对齐逻辑
│   ├── GuideView.tsx      # 导航树主组件
│   ├── Node.tsx           # React Flow 自定义节点 + dagre 布局
│   ├── NodeMenu.tsx       # 右键上下文菜单
│   ├── TypeIcon.tsx       # 节点类型图标（SVG）
│   ├── ChildRow.tsx       # 节点内部子项行
│   └── ChildRow.css       # 子项行样式
└── flow/                 # 右侧图视图（新增模块，目前多为骨架）
    ├── state.ts           # 图类型状态（Zustand）
    ├── FlowViewer.tsx     # 图视图主组件（占位）
    ├── Node.tsx           # 图视图节点定义（elemNode/groupNode）
    ├── Edge.tsx           # 图视图边定义（BaseEdge 包装）
    ├── layout.ts          # 图布局（空文件）
    └── Toolbar.tsx        # 图工具栏（仅注释）
```

### 页面布局架构
应用采用**三栏拖拽布局**（`react-reflex`）：
1. **左侧栏（40%）**：上下分割
   - **上 60%**：源码编辑器（当前为 `PanePlaceholder`，显示"源码编辑器 施工中"）
   - **下 40%**：`GuideView`（IR 导航树）
2. **右侧栏（60%）**：`PanePlaceholder`，显示当前图类型字符串（来自 `useGraphState().graphType`）

初始无 Module 时，全屏显示 `FileLoader`（拖拽上传或点击选择文件）。加载成功后切换到 `MainPage`。

### 数据流
1. **文件加载**：`FileLoader` / `AppMenu` → `handleFileLoad` → `file-load.tsx`（推断 SourceTy） → `IRState.compile()` → WASM `ModuleInfo.compile_from()`
2. **IR 树导航**：
   - 用户点击/展开/收起 → `GuideView` 回调 → `guide-view-tree.ts` 控制器 → WASM `IRExpandTree` / `IRTreeCursor` → 返回新的 `GuideNodeData` 树 → React Flow 重新渲染
   - 焦点路径变更 → `IRState.setFocus()` → Zustand 触发重渲染 → `GuideView` 通过 `revision` reducer 重新构建树
3. **图视图切换**：`GuideView` 右键菜单 → `GraphStore.setGraphType()` → 右侧占位区域显示当前类型（实际渲染尚未接入）
4. **图数据获取**：计划由 `FlowViewer` 直接从 `IRState` 调用 WASM API（`getCallGraph`, `getFuncCfg`, `getFuncDomTree`, `getBlockDfg`）获取数据

### 与 WASM 后端的交互方式
- **依赖包**：`remusys-wasm-b2`（本地 tgz 包，通过 `wasm-pack build --target bundler` 生成）
- **核心类**：
  - `ModuleInfo`：编译入口、IR 树查询、图数据生成（CallGraph/CFG/DomTree/DFG）、重命名
  - `IRExpandTree`：Rust 侧维护的展开状态树，支持 `expand_one/two/all`, `collapse`, `load_tree`
  - `IRTreeCursor`：安全地在 IR 树中导航，验证路径有效性，用于焦点路径 reconcile
- **数据契约**：`remusys-wasm-b2/api/types.ts` 与 `pkg/remusys_wasm_b2.d.ts` 保持同步，定义了完整的 DTO 类型（IRTreeObjID、ValueDt、CFG/DFG/CallGraph/DomTree 结构等）
- **所有权管理**：WASM 对象（`ModuleInfo`, `IRExpandTree`, `IRTreeCursor`）均需手动调用 `.free()` 释放内存。`GuideTreeController` 在组件卸载时会 dispose。

---

## 详细设计（按模块分小节）

### 1. ir/state.ts — IR 全局状态管理（Zustand）

**类型设计**：
```ts
interface IRStorage {
  module?: ModuleInfo;   // 当前加载的 WASM Module
  source: string;        // 反编译/导出的源码文本
  focus: IRObjPath;      // 当前焦点路径（默认 [Module]）
}
interface IRActions {
  compile(src_kind, src, filename?): void;
  getFocusSrcRange(): MonacoSrcRange;
  getModule(): ModuleInfo;
  setFocus(path): void;
  clearFocus(): void;
  getTreeChildren(path): IRTreeNodeDt[];
  getCallGraph(): CallGraphDt;
  getFuncCfg(func): FuncCfgDt;
  getFuncDominance(func): DomTreeDt;
  getBlockDfg(block): BlockDfg;
}
```

**核心逻辑**：
- `compile()`：调用 `ModuleInfo.compile_from()`，成功后保存 module、source、重置焦点为根节点。
- `setFocus()`：通过 `isSamePath()` 进行深度相等性检查，避免无意义的重复更新。
- 所有 `get*()` 方法均为**受控访问器**：若 `module` 未加载则抛出 `"module not loaded"` 错误。
- 导出便捷 Hook：`useIRFocus()`, `useIRModule()`, `useIRFocusSrcRange()`。

**关键点**：
- Zustand store 使用 `immer` + `devtools` 中间件，支持不可变更新和 Redux DevTools 调试。
- `isSamePath()` 是路径比较的核心工具函数，处理 `Module` 类型无 `value` 字段的特殊情况。

---

### 2. guide_view/ — GuideView 导航树

#### 2.1 guide-view-tree.ts — 树控制器与状态机

**设计理念**：
- 前端不缓存 IR 实体，唯一可信数据源是 WASM 的 `IRExpandTree`。
- 区分两种生命周期事件：
  1. **同一 Module 内的树刷新**（展开/收起/重命名）：保留展开状态，通过 `IRTreeCursor` reconcile 焦点路径。
  2. **重新编译/加载 Module**：硬重置，废弃所有前端状态。

**核心类型**：
```ts
export type GuideTreeBuildResult = {
  root: GuideNodeExpand;        // React Flow 可消费的展开树根
  nextFocusPath: IRObjPath;     // reconcile 后的焦点路径
  resolvedPath?: IRObjPath;     // 操作目标路径
};
export type GuideTreeController = {
  moduleId?: number;
  expandTree?: IRExpandTree;
};
```

**关键函数**：
- `ensureExpandTree(irStore, controller)`：检查 `moduleId` 是否变化，若变化则释放旧 `IRExpandTree` 并新建。
- `reconcileFocusPath(irStore, focusPath)`：使用 `IRTreeCursor` 沿旧焦点路径逐层验证，若某层子对象不存在则截断，回退到最近可达祖先。
- `reloadTreeWithWasm()`：调用 `expandTree.load_tree(module, focusPath)` 获取新的可见树，补全父指针（`connectGuideTree`），同步更新全局焦点。
- 操作函数：`expandNode`, `expandChildrenNode`, `dfsExpandNode`, `collapseNode`, `collapseChildrenNode`, `requestFocusNode`, `requestFocusPath`。

**父指针机制**：WASM 返回的树节点无 `parent` 引用，`connectGuideTree()` 在 JS 侧递归补全，以支持 `pathOfNode()` 从任意节点回溯到完整路径。

#### 2.2 GuideView.tsx — 导航树主组件

**架构**：
- 使用 `ReactFlow` + `ReactFlowProvider` 渲染树。
- 通过 `useRef<GuideTreeController>` 持有可变控制器状态，避免触发 React 重渲染。
- 通过 `useReducer` 的 `revision` 计数器强制触发树的重新构建（因为树构建是同步的 memo 计算）。
- `pendingRootRef`：存储最近一次操作产生的新树根，在 `useMemo` 中消费并清空。

**事件处理**：
- 节点双击/点击 → `onFocus`（设置焦点）
- 节点展开/收起 → `onToggle`（根据当前 `children` 是否存在决定 collapse 或 expand）
- 右键菜单 → `onRowContextMenu` / `onNodeContextMenu` → `buildMenuItems()`

**菜单构建**（`buildMenuItems`）：
- 基础操作：聚焦、展开一层、展开全部、收起、收起全部子节点
- 根据 `kind` 动态追加图视图切换项：
  - `Module` → "显示函数调用图"
  - `Func`（实体为 `Global`） → "显示 CFG"、"显示支配树"
  - `Block`（实体为 `Block`） → "显示 DFG"
  - `NormalInst/TerminatorInst/PhiInst` → "显示 Def-Use 图"
- 特殊节点（`FuncHeader`, `FuncArg`, `BlockIdent`）不追加实体相关菜单项。

**容错**：若 `collectGuideTree` 返回空数组，渲染一个错误占位节点。

#### 2.3 Node.tsx — React Flow 自定义节点与布局

**节点类型**：`GuideNode`（单一自定义节点类型）

**数据结构转换**（`collectGuideTree`）：
- 递归 DFS 遍历 `GuideNodeExpand`，将展开节点转为 `GuideRFNode`，子节点间的连接转为 `XYFlow.Edge`。
- **非展开子节点不渲染为独立 Flow 节点**，而是作为父节点内部的 `ChildRow` 列表。
- 边样式根据子节点的 `focusClass` 动态变化：
  - 焦点路径上的边（`FocusNode/FocusParent/FocusScope`）：蓝色虚线 + 动画（`animated: true`）
  - 普通边：灰色实线

**布局算法**（`dagreLayoutGuideTree`）：
- 使用 `dagre`（`graphlib.Graph`），方向 `LR`（从左到右），`nodesep: 24`, `ranksep: 56`。
- 布局后根据节点宽高做中心对齐修正（`x - width/2, y - height/2`）。

**节点尺寸估算**（`guideNodeSize`）：
- 头部固定 52px，每行 40px，最大高度 300px，最小高度 92px。
- 宽度固定 240px。

**节点渲染**（`GuideViewNode`）：
- 左侧 `Handle(target)` + 右侧 `Handle(source)`，用于 React Flow 边连接。
- 顶部标题栏：显示 `TypeIcon` + 标签名（空则显示 `"(no name)"`）。
- 焦点节点标题栏背景为 `#eef2ff`（浅蓝），非焦点为 `#f9fafb`。
- 焦点节点的 `TypeIcon` 外圈加蓝色边框。
- 主体区域：可滚动子节点列表（`ChildRow`），空列表显示 "(无子节点)"。

#### 2.4 NodeMenu.tsx — 右键上下文菜单

- 固定定位（`position: fixed`），通过 `clampMenuPosition()` 限制在视口内。
- 每项高度约 40px，底部有 "取消" 按钮。
- hover 效果通过 `onMouseEnter/Leave` 动态修改背景色。
- 点击菜单项后调用 `onSelect(node)` 并关闭菜单。

#### 2.5 TypeIcon.tsx — 节点类型图标

- 纯 SVG 组件，固定 `viewBox="0 0 16 16"`。
- 每种 `IRTreeNodeClass` 映射到颜色 + 缩写文字：
  - `Module` → 红底白字 "M"
  - `GlobalVar` → 深蓝底白字 "Gv"
  - `Func` → 琥珀底黑字 "Fx"
  - `ExternFunc` → 灰底白字 "Fx"
  - `Block` → 橙底白字 "B"
  - `NormalInst` → 绿底黑字 "I"
  - `PhiInst` → 天蓝底黑字 "Φ"
  - `TerminatorInst` → 橙底白字 "Ti"
  - `Use` → 紫底白字 "U"
  - `JumpTarget` → 粉底白字 "Jt"
  - `FuncArg` → 琥珀底黑字 "Arg"
- `focused=true` 时绘制双层圆环（外圈黑边 + 内圈填充）。

#### 2.6 ChildRow.tsx / ChildRow.css — 节点内部子项行

- 每行高度 40px，flex 布局：图标（20px）+ 标签（ellipsis 截断）+ 展开指示器（圆形）。
- 展开状态的指示器变为蓝底白点，行背景变为 `#f3f4f6`。
- 点击行触发 `onToggle`；右键触发 `onContextMenu`。
- 在焦点路径上的行（`insideFocusPath=true`）会通过 `TypeIcon` 的 `focused` 属性显示聚焦圆环。

---

### 3. flow/ — 图视图（Flow Viewer）

#### 3.1 state.ts — 图类型状态

```ts
export type GraphType =
  | { type: "Empty" }
  | { type: "Error", message, backtrace? }
  | { type: "Focus" }              // 根据焦点自动选择图
  | { type: "CallGraph" }
  | { type: "FuncCfg", func: GlobalID }
  | { type: "FuncDom", func: GlobalID }
  | { type: "BlockDfg", block: BlockID }
  | { type: "DefUse", center: InstID };
```

- 默认状态：`{ type: "Focus" }`
- 仅存储当前图类型，**不缓存图数据**。

#### 3.2 FlowViewer.tsx — 图视图主组件

**状态：严重未完成。**
- 当前仅 `import { useGraphState } from "./state"` 并导出空函数组件。
- 注释中描述了完整设计蓝图：
  - 数据直接从 `IRStore` 获取，不维护前端缓存。
  - 单层图（CallGraph/CFG/DomTree/DefUse）使用 `dagre` 布局。
  - 双层子图（BlockDfg）计划使用 `elkjs` 布局。
  - 交互部分待实现（"目前暂时不做什么交互, 等后面再说. 写论文要紧."）。

#### 3.3 Node.tsx — 图视图节点

定义了两种节点类型：
- `elemNode`：普通元素节点，带上下 `Handle`（Top target + Bottom source）。
- `groupNode`：分组节点（用于 BlockDfg 的 Section 容器），无 Handle。

**数据类型**：
```ts
export type FlowNodeBase = {
  label: string | React.ReactNode;
  focused: boolean;
  irObjID: IRTreeObjID | null;
  bgColor: string;
};
```

**辅助函数**：`makeErrorGraph(error)` — 将 JS Error 转为可展示的 React Flow 节点（红底错误卡片 + 堆栈回溯）。

#### 3.4 Edge.tsx — 图视图边

- 自定义边类型 `FlowEdge`，包装 `@xyflow/react` 的 `BaseEdge`。
- 数据字段：`path`（SVG path 字符串）、`labelPosition`、`isFocused`、`irObjID`。
- `isFocused=true` 时边粗细为 1.5，否则为 1。
- 注释说明：b2 使用 dagre 替代 b1 的 GraphViz，因此边路由路径需适配 dagre 格式。

#### 3.5 layout.ts — 图布局

**状态：空文件。** 尚未实现任何布局逻辑。

#### 3.6 Toolbar.tsx — 流图工具栏

**状态：仅注释，无代码。** 设计意图是底部小白条，非 Focus 模式时显示当前图类型和关闭按钮。

---

### 4. App / AppMenu / FileLoader / file-load.tsx — 顶层布局与文件加载

#### 4.1 App.tsx

**条件渲染**：
- 无 `module` → `<FileLoader onLoad={compile} />`
- 有 `module` → `<MainPage />`

**MainPage 布局**（`react-reflex`）：
- 外层 `ReflexContainer`（vertical）：左侧 40% + 右侧 60%
- 左侧内层 `ReflexContainer`（horizontal）：上 60%（源码编辑器占位）+ 下 40%（GuideView）
- 右侧：图视图占位，显示 `JSON.stringify(useGraphState().graphType)`

**PanePlaceholder**：通用占位组件，标题 + 描述，背景为点阵图案（`radial-gradient`）。

#### 4.2 AppMenu.tsx

- 使用 `@headlessui/react` 的 `Menu/MenuButton/MenuItems/MenuItem` 实现下拉菜单。
- **文件菜单**：
  - "打开..."：创建隐藏 `<input type="file">`，支持 `.ll/.ir/.remusys-ir/.sy/.sysy`，调用 `handleFileLoad`。
  - "保存..."：`alert("等待实现: 保存当前存档到浏览器...")`
- **帮助菜单**："关于" → `alert(aboutText)`
- 右侧显示应用名称 "Visual Remusys"。
- 样式为内联 style 对象（复古工具栏风格，灰底黑字）。

#### 4.3 FileLoader.tsx

- 全屏拖拽上传区域（`onDragOver` + `onDrop`）。
- 中央大卡片（520px，圆角虚线边框），点击唤起文件选择器。
- 支持隐藏 `<input type="file">` 的 change 事件。
- 样式为内联 style 对象（浅色现代风格）。

#### 4.4 file-load.tsx

- 核心函数：`handleFileLoad(file, onLoad)`
- 文件扩展名推断 `SourceTy`：
  - `.ll`/`.ir`/`.remusys-ir` → `"ir"`
  - `.sy`/`.sysy` → `"sysy"`
  - 其他 → `alert` 错误
- 使用 `FileReader.readAsText()` 读取文件内容，成功后回调 `onLoad(mode, text, name)`。

---

### 5. 样式与入口文件

#### 5.1 main.tsx
- 标准 React 19 `StrictMode` + `createRoot` 入口。

#### 5.2 index.css
- 重置 `html/body/#root` 宽高为 100%，全局 `box-sizing: border-box`。
- 字体栈：Segoe UI / PingFang SC / Noto Sans CJK SC。

#### 5.3 App.css
- `.app-root`：全屏 flex 列布局，灰底。
- `.app-main`：flex 1 填充剩余空间。
- `.panel-left`：白底右边框。
- `.pane-placeholder`：居中的点阵背景占位。
- `.guide-view-host/.guide-view-shell`：GuideView 外壳样式（标题栏 + 主体）。
- 响应式：`@media (max-width: 960px)` 左侧栏底部边框（移动端适配）。

---

## 缺失/未完成部分

| 模块 | 缺失内容 | 严重程度 |
|------|---------|---------|
| `App.tsx` | 右侧图视图区域仍为 `PanePlaceholder`，未接入 `FlowViewer` | 高 |
| `App.tsx` | 源码编辑器区域仍为 `PanePlaceholder`，Monaco 编辑器未接入 | 高 |
| `flow/FlowViewer.tsx` | 空实现，无渲染逻辑 | 高 |
| `flow/layout.ts` | 空文件，无 dagre/elkjs 布局实现 | 高 |
| `flow/Toolbar.tsx` | 仅注释，无组件代码 | 中 |
| `AppMenu.tsx` | "保存..." 功能未实现 | 低 |
| `GuideView.tsx` | 节点重命名功能未接入（WASM `rename()` API 已存在） | 中 |
| `GuideView.tsx` | 没有键盘导航/快捷键支持 | 低 |
| `ir/state.ts` | 错误处理机制粗糙（直接抛异常，无边界捕获） | 中 |

---

## 相比旧版本的变更摘要（b2 vs b1）

### 架构层面
1. **放弃前端实体缓存**：b1 前端缓存完整 IR 实体树和各种详细信息；b2 所有数据按需从 WASM 拉取，前端仅保留展开状态 + 焦点路径。
2. **WASM 交互模式升级**：b2 引入 `IRExpandTree` 和 `IRTreeCursor`，由 Rust 侧管理树状态，JS 侧仅做 UI 映射。
3. **状态管理精简**：b1 可能有复杂的实体存储；b2 仅有一个 `IRState`（Zustand）存储 module + focus，图状态独立为 `GraphStore`。

### 新增模块
1. **`flow/` 图视图模块**：全新设计，计划支持 CallGraph、CFG、DomTree、BlockDfg、DefUse 等多种图类型。目前为骨架状态，类型定义和节点/边组件已就位。
2. **`FileLoader.tsx` / `file-load.tsx`**：独立的文件加载页面，支持拖拽上传，替代 b1 可能内嵌在编辑器中的加载方式。
3. **`AppMenu.tsx`**：顶部菜单栏（文件/帮助），使用 Headless UI 实现。

### GuideView 变更
1. **渲染引擎更换**：从 b1 的自定义 DOM 树改为 **React Flow** 实现，节点使用 dagre 自动布局。
2. **节点设计**：由纯文本列表变为**卡片式节点**（标题栏 + 子项列表），带 TypeIcon 和展开指示器。
3. **焦点可视化**：焦点路径上的边显示为蓝色虚线动画，焦点节点标题栏高亮。
4. **右键菜单**：新增节点级上下文菜单，支持图视图快捷切换。

### 布局与交互
1. **三栏拖拽布局**：b2 使用 `react-reflex` 实现可拖拽分割面板，b1 可能无此功能或实现方式不同。
2. **焦点路径 reconcile**：b2 在树刷新时自动对齐焦点路径（回退到最近可达祖先），b1 可能无此机制。
3. **父指针回溯**：b2 在 JS 侧补全 `parent` 引用以支持 `pathOfNode`，b1 可能依赖前端缓存的完整树结构。

### 占位状态变化
- b2 的源码编辑器和图视图目前均为 **PanePlaceholder**（施工中），但图视图占位会显示当前 `graphType`，为后续接入预留接口。
- b1 可能已有部分功能的初步实现，而 b2 正在重构底层，上层功能暂时回退到占位状态。
