/**
 * # Guide View 树加载与最小状态
 *
 * 一个 IR Module 下的树结构非常庞大. 倘若全部展示, 且不说性能问题, 那么多结点会直接
 * 淹没用户. 因此 Remusys-Lens 的树导航视图不会一次性全部展示所有结点. 在启动时,
 * 视图只展示一个表示 Module 的根节点. 每个树导航视图控件都有一个子结点列表, 用户可以
 * 点击列表中表示子结点的子项展开或者收起这个子结点.
 * 
 * 那...这些结点的数据从哪里来?
 * 
 * 在旧版本 b1 分支中, 前端不仅有完整的 IR 树缓存, 还缓存了整个中间代码系统的各种详细
 * 信息. wasm 服务层会把 IR 对象的具体类型等信息做一些概括后按需发送到前端缓存, JS 侧的
 * `TreeNodeStorage` 再从这些具体的 IR 对象类型中获取内容, 按不同对象类型组织成树结构,
 * 点击展开时还可以根据关联的子结点 IR 对象类型分派不同的结点构造逻辑和子结点加载逻辑.
 * 这样当然很好用, 但它依赖于“前端持有一份可长期信任的实体缓存”这个前提.
 *
 * b2 要求放弃这个前提. 这一版不允许前端缓存 IR 实体树, 所有对象数据都直接从 WASM 服务层
 * 获取. Guide View 也一样: 它唯一可信的数据来源是 IR Tree, 但只截取
 * Module-Global/Func-Block-Inst-(Inst 的直接 Use) 这几层有一一映射的主干树. 前端负责的
 * 不是“保存一棵树”, 而只是保存最小交互状态, 例如哪些结点当前处于展开态.
 *
 * 这里要严格区分两件事:
 *
 * 1. 同一 Module 生命周期内的 IR 树变化
 *    例如展开、收起、重命名、局部插入、局部删除、局部替换. 这些变化发生时, 前端不会继承
 *    b1 那种实体缓存, 而是基于新的 IR Tree 重新构造当前可见主干树. 但用户的展开状态不能
 *    平白丢失, 因此前端仍需要维护少量 UI 状态, 并在树刷新时做 reconciliation:
 *    尽量保留仍然合法的展开状态; 当前焦点若失效, 则向上回退到最近仍存在的祖先.
 *
 * 2. 重新编译 / 重新加载 Module
 *    这不属于“IR 树变化”, 而是整个数据宇宙被替换掉. 旧 Module 被丢弃后, 旧的树路径、
 *    旧的对象 ID、旧的展开状态和旧的焦点都不再具有语义合法性. 因此这里不做连续性保留,
 *    而是直接废弃全部前端状态, 从新的 Module 根重新初始化 Guide View.
 *
 * 因而本文件要解决的问题不是“如何在前端缓存一棵完整 IR 树”, 而是:
 *
 * 1. 如何只根据当前展开集按需从 wasm 拉取可见树切片;
 * 2. 如何把这些切片组织成 React Flow 需要的主干树;
 * 3. 如何在同一 Module 内的树刷新后尽量保持展示不突兀.
 */

import { useIRStore, type IRState, type IRStorage } from "../ir/state";
import type { IRTreeObjID } from "../ir/types";
import type { GuideNodeExpand } from "./Node";

import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

/**
 * @returns GuideNodeExpand 返回新树结构的根节点.
 */
export function loadExpandedGuideTree(): GuideNodeExpand {
    const irStat = useIRStore();
    throw new Error("Not implemented; stat: " + JSON.stringify(irStat));
}
