# Remusys Lens b2: 新 App 与 GuideView 设计说明

## 1. 设计目标

本设计文档用于约束 b2 版本在页面结构、状态边界、GuideView 交互和后续扩展上的统一实现方式。

核心目标：

1. 页面视觉布局与 b1 保持一致，避免用户认知断层。
2. 数据和状态边界采用 b2 的 wasm-first 模型，不回退到 b1 的前端缓存中心。
3. GuideView 定位为全局导航面板，不负责主图视图的数据 ownership。
4. 其余未完成区域统一使用“施工中”占位，不阻塞主干架构搭建。

## 2. 视觉布局设计

说明：本章的布局与菜单设计，不仅参考 b1 的视觉截图，也参考 b1 的实现技术栈（react-reflex 和 @headlessui/react）。

## 2.1 总体分区

页面沿用 b1 的主框架：

1. 顶部菜单栏。
2. 主内容区左右分栏。
3. 左栏再上下分栏：上方源码窗格，下方 GuideView 窗格。
4. 右栏为主图视图窗格。

布局语义：

1. GuideView 是辅助导航区，视觉权重低于右侧主图。
2. 右栏面积应大于左栏 GuideView 子区。
3. 左栏上下分区固定语义，不在本轮做可拖动分割。

b1 事实依据：

1. b1 在 [remusys-lens/src/App.tsx](remusys-lens/src/App.tsx) 中使用两层 [react-reflex](https://www.npmjs.com/package/react-reflex) 进行分割：
第一层左右分割（ReflexContainer vertical），第二层左栏内上下分割（ReflexContainer horizontal）。
2. b1 在 [remusys-lens/package.json](remusys-lens/package.json) 中声明了 react-reflex 依赖。

b2 设计决策：

1. 本轮直接使用 react-reflex 建立双层可拖拽分栏，对齐 b1 的布局技术路径。
2. 静态 CSS 仅承担视觉皮肤，不承担主分割逻辑。

## 2.2 顶部菜单

本轮仅保留一个菜单项：

1. 文案：施工中。
2. 点击行为：alert("正在施工")。

该菜单用于保留交互入口形态，后续菜单体系可以在此基础上扩展。

b1 事实依据：

1. b1 在 [remusys-lens/src/TopMenu.tsx](remusys-lens/src/TopMenu.tsx) 中使用 @headlessui/react 的 Menu、MenuButton、MenuItems、MenuItem 构建菜单。
2. 菜单项分组为“文件”和“帮助”，包含打开、保存、关于等动作。
3. TopMenu 主要采用内联样式对象（menuStyle、btnStyle、itemsStyle），而不是单独的 TopMenu.css。

b2 设计决策：

1. 本轮保留单入口菜单，先建立交互位置和触发行为。
2. 后续要恢复多级菜单时，优先沿用 b1 的 Headless UI 方案，保证可访问性和键盘交互一致。

## 2.3 占位视觉规范

除 GuideView 外，其他窗格都采用统一占位组件：

1. 标题 + 施工中文案。
2. 白灰底色 + 点阵背景，模拟图形区域的空间感。
3. 样式统一，避免每个占位区域风格割裂。

## 2.4 GuideView 外观规范

本轮 GuideView 先使用壳层占位，但样式需与未来目标一致：

1. 独立边框容器。
2. 头部标题区 + 主体内容区。
3. 主体区保留点阵背景，未来用于承载 React Flow 画布。

## 3. 架构与状态边界

## 3.1 全局状态

全局状态由 IRStore 管理：

1. module。
2. source。
3. focus。

约束：

1. focus 在全局只有一个真相源。
2. GuideView 不能维护长期本地 focus 副本。

## 3.2 GuideView 局部状态

GuideViewTreeStore 管理最小 UI 编排状态：

1. expandTree。
2. root。
3. treeEpoch。
4. moduleId。

约束：

1. 仅持有与 GuideView 渲染直接相关的局部状态。
2. 所有树结构有效性由 wasm 返回结果决定。

## 3.3 模块生命周期

需要严格区分两个层面：

1. 同一 module 下的刷新：refreshSameModule。
2. 新 module 的替换：resetForNewModule。

规则：

1. 同 module 变化尽量保留合法展开/焦点路径。
2. 新 module 一律硬重置，不保留旧对象语义。

## 4. GuideView 组件设计

## 4.1 组件职责

GuideView 的职责：

1. 消费全局 module/focus。
2. 消费 GuideViewTreeStore 的可见树 root。
3. 组织画布渲染和菜单交互。
4. 将导航行为上送给 App 分发层。

非职责：

1. 不直接管理右侧图视图的展示状态。
2. 不作为跨视图状态中心。

## 4.2 组件拆分

建议拆分如下：

1. GuideView：容器组件，负责状态订阅和事件编排。
2. Node：单个导航节点渲染（已存在）。
3. NodeMenu：右键菜单（已存在）。
4. 占位/空态组件：无 module、加载中、异常等状态展示。

## 4.3 关键交互

1. 双击节点：请求聚焦该节点路径。
2. 子项点击：展开或收起节点。
3. 右键菜单：构建动作项并执行。

交互约束：

1. 动作输入尽量用节点对象或路径，不依赖不稳定 id。
2. 每次动作后以重载树结果为准，不做乐观缓存推断。

## 5. App 组件设计

App 作为编排层，职责如下：

1. 提供全局布局壳。
2. 接收 GuideView 导航事件。
3. 路由事件到 IRStore 或右侧图视图状态。

与 b1 的技术对齐：

1. 布局层：b1 的 ReflexContainer/ReflexElement/ReflexSplitter 结构是可直接迁移的目标形态。
2. 样式层：b1 的 [remusys-lens/src/App.css](remusys-lens/src/App.css) 已定义了 guide-view 与 react-flow 的关键样式约束（例如 handle 可见性）。
3. 菜单层：b1 的 TopMenu 采用 headless 菜单原语，建议在 b2 多菜单阶段保持同样技术路径。

导航事件分层：

1. 本地立即动作：ExpandOne/ExpandAll/Collapse/Focus。
2. 外部视图动作：ShowCfg/ShowDominance/ShowDfg/ShowValueDefUse。

即便右侧图视图本轮是占位，也要保留该事件协议，防止后续重复改契约。

## 6. 数据流设计

主路径：

1. 用户在 GuideView 操作。
2. GuideViewTreeStore 调 wasm (expand/collapse/load_tree)。
3. root 更新，GuideView 重绘。
4. 必要时同步更新 IRStore.focus。
5. App 根据导航事件更新右侧图视图选择状态。

异常路径：

1. module 不存在时，GuideView 显示空态。
2. module 变化时，GuideView 强制 resetForNewModule。

## 7. 视觉与实现一致性清单

必须满足：

1. 左上源码窗格、左下 GuideView、右侧主图区三者同屏。
2. 顶部菜单只保留“施工中”。
3. 非 GuideView 窗格显示统一施工中占位。
4. GuideView 保留未来 React Flow 承载容器的视觉结构。

## 8. 实施阶段建议

Phase A: 壳层落地

1. 替换 App 与全局样式。
2. 放置占位组件与 GuideView 壳。
3. 保留顶部菜单行为。

Phase A+（对齐 b1 交互框架）

1. 若需要还原 b1 的完整菜单组，替换当前按钮为 @headlessui/react Menu 结构。
2. 根据业务需求补齐 b1 的打开/保存/关于菜单动作。

Phase B: GuideView 接树

1. GuideView 消费 guide-view-tree store。
2. 接入 Node/NodeMenu。
3. 完成本地导航动作闭环。

Phase C: 右侧图视图接线

1. 新增 graph selection state。
2. 将菜单动作映射到图类型。
3. 逐步替换占位为真实图组件。

## 9. 验证标准

1. 页面结构与 b1 截图一致性：分区位置、权重、层级。
2. 顶部菜单行为正确：点击弹出“正在施工”。
3. 其余窗格占位一致：文案和样式统一。
4. GuideView 可独立承载后续功能：容器结构、样式边界已准备好。
5. TypeScript 无新增错误。
6. 技术一致性可追溯：文档中的布局/菜单方案可在 b1 源码中找到对应实现。

## 10. b1 技术事实摘录

为避免后续设计漂移，这里固定 b1 的关键实现事实：

1. 分栏库：react-reflex（见 [remusys-lens/package.json](remusys-lens/package.json) 和 [remusys-lens/src/App.tsx](remusys-lens/src/App.tsx)）。
2. 顶部菜单库：@headlessui/react（见 [remusys-lens/package.json](remusys-lens/package.json) 和 [remusys-lens/src/TopMenu.tsx](remusys-lens/src/TopMenu.tsx)）。
3. 菜单动作语义：文件（打开/保存）+ 帮助（关于）。
4. 菜单样式组织：TopMenu 内联样式对象为主，不依赖独立 TopMenu.css。
5. 布局骨架：顶部菜单 + 左右分栏 + 左栏内上下分栏。
