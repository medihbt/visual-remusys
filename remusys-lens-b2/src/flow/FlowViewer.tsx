/**
 * # FlowViewer -- 流图视图
 *
 * 查看焦点处或者其他选点的流图。
 * 
 * ## 支持什么流图
 * 
 * 支持的流图类型参见 `state.ts` 中的 `GraphType` 定义，目前包括：
 * 
 * - 空图（Empty）
 * - 错误图（Error）：不是正经流图, 是项目出错了, 刚好有个界面可以显示错误信息。
 * - 焦点图（Focus）：不是某个特定的流图，而是根据当前焦点的类型自动选择一个图来显示。
 * - 调用图（CallGraph）：显示函数之间的调用关系。
 *   Focus 的备选之一, 当焦点为全局变量、外部函数、模块时, Focus 图会自动切换到调用图。
 * - 函数控制流图（FuncCfg）：显示一个函数内部的基本块和它们之间的控制流关系。
 *   Focus 的备选之一, 当焦点在函数定义及以下时, Focus 图会自动切换到函数控制流图。
 * - 函数支配树（FuncDom）：显示一个函数内部的基本块和它们之间的支配关系。
 *   与 Focus 无关, 任何情况下都不会自动切换, 需要通过菜单手动切换
 * - 基本块数据流图（BlockDfg）：显示一个基本块内部的指令和它们之间的数据流关系。
 *   与 Focus 无关, 任何情况下都不会自动切换, 需要通过菜单手动切换
 * - 定义-使用链（DefUse）：以某条指令为中心，显示与它相关的定义-使用关系。
 *   与 Focus 无关, 任何情况下都不会自动切换, 需要通过菜单手动切换
 * 
 * ## 数据来源在哪儿
 * 
 * 目前的设计是，FlowViewer 直接从 IRStore 获取数据， IRStore 直接调用 WASM API
 * 获取数据并进行必要的转换。这样可以不用维护复杂的前端缓存, 美哉
 * 
 * ## 怎么排版
 * 
 * 除了 BlockDfg 之外, 其他图都是没有子图的单层图, 所以使用 dagre 做结点排版+边路由.
 * 相比 GraphViz, dagre 的 API 更加清晰，更容易维护.
 * 
 * BlockDfg 是 Section-Node 双层图, 有子图结构, 因此使用 Elk.js 来排版. Elkjs 比较
 * 复杂，但至少不像 GraphViz 那样接口模糊不清，而且 Elkjs 能排带子图的图, dagre 不行.
 * 
 * ## 交互
 * 
 * 目前暂时不做什么交互, 等后面再说. 写论文要紧.
 */

import { useGraphState } from "./state";

export default function FlowViewer() {
  const graphStore = useGraphState();
}
