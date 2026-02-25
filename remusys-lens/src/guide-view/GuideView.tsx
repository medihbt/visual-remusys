import type { Node, NodeProps } from '@xyflow/react';
import { ReactFlow, Controls, Background } from '@xyflow/react';
import React from 'react';

/* DO NOT DELETE: load this if fail */
export const GuideViewText = <>
  <h3>导航视图</h3>
  <p>从 Module 全局对象到当前锁定对象的 React Flow 树状视图, 展现“模块 - 函数 - 基本块 - 指令”的 IR 模块层次架构，在这个架构上提供导航和聚焦功能。</p>
  <h3>结点</h3>
  <p>圆角矩形小窗口, 圆角只有 3px. 通过 React Flow 的 handle 与其他结点连接. 组成包括:</p>
  <ul>
    <li>顶栏: 展示类型图标、结点名称, 有个 <code>⋯</code> 形状的按钮, 点击按钮即可展开对这个结点的操作菜单(右键单击顶栏也可以)</li>
    <li>
      子结点列表视图，每行一个子结点概述（类型图标、名称）。最右侧有一个表示展开或者关闭的 radio button.
      点击列表整行的任意位置都展开这个位置上的子结点, 再次点击则收回.
      收回后销毁子结点状态, 也就是如果收回前这个子结点被展开成一棵子树, 下次展开时会全部重新请求数据, 子结点不会保持展开状态.
    </li>
  </ul>
  <h3>图标</h3>
  <p>圆形 SVG 图标, 直径 16px, 文字 9pt. 字体为 Cascadia Mono, 各类型文字分别是:</p>
  <ul>
    <li>模块: M, 红底白字.</li>
    <li>全局变量: Gv, 定义为靛蓝底白字, 声明为灰 (<code>#666</code>) 底白字.</li>
    <li>函数: Fx, 定义为黄底黑字, 声明为灰(<code>#666</code>)底白字.</li>
    <li>基本块: B, 橙底白字.</li>
    <li>Phi 指令: Φ, 浅蓝底黑字.</li>
    <li>终止指令: Ti, 橙底白字.</li>
    <li>普通指令: I, 浅绿底黑字.</li>
  </ul>
  <h3>总体交互</h3>
  <p>系统刚刚加载完一段 IR, 进入阅读界面, 出现这个导航视图. 此时导航视图只有一个 Module, 聚焦在 Module 里.</p>
  <p>Module 的动作选项包括:</p>
  <ul>
    <li>(所有结点都这样)聚焦: 焦点放到这个结点上, 触发 Monaco 区域显示该结点关联的源码并聚焦到该结点的源码位置</li>
    <li>(所有结点都这样)展开本级: 展开该结点下的所有子结点 1 层</li>
    <li>(所有结点都这样)展开所有: 递归展开该结点下的所有子结点</li>
    <li>(所有结点都这样)重命名: 检查名称是否冲突, 重新设置结点的名称, 更新所有关联的图和源码映射</li>
    <li>显示引用图: 展示全局变量引用图(函数调用图)</li>
  </ul>
  <p>函数的动作选项包括:</p>
  <ul>
    <li>(三选一:1, 默认)显示 CFG</li>
    <li>(三选一:2)显示支配树</li>
    <li>(三选一:3)显示调用者图</li>
    <li>应用 Pass: 选择一个 Pass, 给这个函数应用这个 Pass, 在时间线上展示这个 Pass 的执行过程和结果</li>
  </ul>
  <p>基本块的动作选项包括:</p>
  <ul>
    <li>显示 DFG</li>
    <li>分析顺序依赖分割点: 计算这个基本块的顺序依赖分割点, 在 CFG 里高亮显示</li>
    <li>前驱后继: 展示这个基本块的前驱和后继基本块列表, 点击列表项可以聚焦到对应基本块</li>
  </ul>
  <p>指令的动作选项包括:</p>
  <ul>
    <li>显示数据流依赖: 展示这个指令的数据流依赖图, 包括它依赖的其他指令和依赖它的其他指令</li>
    <li>相关指令列表: 展示与这个指令相关的其他指令列表, 包括同一基本块里的其他指令、使用了同一变量的其他指令等</li>
    <li>(对于调用指令)内联: 创建快照并内联该调用点</li>
  </ul>
  <p>在 demo 阶段, 所有与具体功能相关联的选项都不要实现, 弹一个 alert 出来即可.</p>
</>;

export type GuideNode = Node<{ name: string }, 'guideNode'>;
export type GuideNodeProps = NodeProps<GuideNode>;

const GuideNode: React.FC<GuideNodeProps> = (props) => {
  return <div>{props.data.name}</div>;
}

export default function GuideView() {
  const nodeTypes = React.useMemo(() => ({ guideNode: GuideNode }), [])
  return <React.Suspense fallback={GuideViewText}>
    <ReactFlow nodeTypes={nodeTypes}>
      <Background />
      <Controls />
    </ReactFlow>
  </React.Suspense>;
}
