const blockInfo = [
  {
    /**
     * 这种 ID 被称为 `IndexedID`, 语法是: `{pool type}:{real index}:{generation}`.
     * 在 WASM 后端中, 这个 ID 会作为内存池的 Key 参与索引等.
     *
     * ## 不同类型的 ID 之间会不会冲突?
     *
     * 在同一个 Module 里的所有 Indexed ID 都不会冲突, 因为它们的 pool type 不同.
     * 但是不同 Module 里的 Indexed ID 可能会冲突, 因为它们的 pool type 和 real index 都可能相同.
     * 好在 Visual Remusys 中不怎么需要处理多个 Module 之间的关系, 也就不需要担心这个问题了.
     *
     * ## 这个 ID 的 generation 是什么?
     *
     * generation 是一个版本号, 每当这个 ID 被重新分配时, generation 就会增加.
     * 这样就可以区分同一个 real index 的不同版本的 ID.
     * 
     * generation 的取值是 [1, 65535], 0 被保留作为无效 ID 的 generation.
     * 也就是说, 当 generation 达到 65535 时, 再增加就会回绕到 1.
     */
    id: "b:1:1",
    name: "entry",
    sourceRange: {
      // 示例 Source Range, 实际定义参照 Monaco Editor 的 IRange 接口
      begin: { line: 2, column: 1 }, end: { line: 4, column: 1 }
    },
    insts: [
      {
        id: "i:1:1",
        opcode: "br",
        kind: "terminator",
        sourceRange: {
          begin: { line: 3, column: 3 }, end: { line: 3, column: 23 }
        }
      }
    ]
  }, {
    id: "b:2:1",
    name: "while.cond",
    sourceRange: {
      begin: { line: 4, column: 1 }, end: { line: 7, column: 1 }
    },
    insts: [
      {
        id: "i:2:1",
        opcode: "call",
        kind: "normal",
        sourceRange: {
          begin: { line: 5, column: 3 }, end: { line: 5, column: 26 }
        }
      },
      {
        id: "i:3:1",
        opcode: "br",
        kind: "terminator",
        sourceRange: {
          begin: { line: 6, column: 3 }, end: { line: 6, column: 46 }
        }
      }
    ]
  }, {
    id: "b:3:1",
    name: "while.body",
    sourceRange: {
      begin: { line: 7, column: 1 }, end: { line: 10, column: 1 }
    },
    insts: [
      {
        id: "i:4:1",
        opcode: "call",
        kind: "normal",
        sourceRange: {
          begin: { line: 8, column: 3 }, end: { line: 8, column: 24 }
        }
      },
      {
        id: "i:5:1",
        opcode: "br",
        kind: "terminator",
        sourceRange: {
          begin: { line: 9, column: 3 }, end: { line: 9, column: 23 }
        }
      }
    ]
  }, {
    id: "b:4:1",
    name: "exit",
    sourceRange: {
      begin: { line: 10, column: 1 }, end: { line: 12, column: 1 }
    },
    insts: [
      {
        id: "i:6:1",
        opcode: "ret",
        kind: "terminator",
        sourceRange: {
          begin: { line: 11, column: 3 }, end: { line: 11, column: 19 }
        }
      }
    ]
  }
];

const moduleInfo = {
  name: "demo-module.ll",
  overview_source: `
@arr = global [10 x i32] zeroinitializer, align 16
@arr2 = external global [10 x i32], align 16

define i32 @main() { ... }
`,
  gvars: [
    {
      id: "g:1:1",
      name: "arr",
      type_id: "[10 x i32]",
      source: "@arr = global [10 x i32] zeroinitializer, align 16",
      operands: [
        {
          id: "u1:1",
          type_id: "[10 x i32]",
          source_range: {
            begin: { line: 1, column: 26 }, end: { line: 1, column: 41 }
          }
        }
      ]
    },
    {
      id: "g:2:1",
      name: "arr2",
      type_id: "[10 x i32]",
      source: "@arr2 = external global [10 x i32], align 16",
      operands: [],
    }
  ],
  funcs: [
    {
      id: "g:3:1",
      name: "main",
      source: `define i32 @main() {
entry:
  br label %while.cond
while.cond:
  %cond = call i1 @cond()
  br i1 %cond, label %while.body, label %exit
while.body:
  call void @body()
  br label %while.cond
exit:
  ret i32 0
}
`,
      blocks: blockInfo,
    }
  ]
};

/* DO NOT DELETE: load this if fail */
const GuideViewText = <>
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

export default function GuideView() {
  return GuideViewText;
}