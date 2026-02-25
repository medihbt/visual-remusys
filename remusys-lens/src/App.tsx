import { ReflexContainer, ReflexElement, ReflexSplitter } from "react-reflex";
import "./App.css";
import "react-reflex/styles.css";
import LensViewer from "./editor/LensViewer";
import FlowViewer from "./flow/FlowViewer";
import React from "react";
import GuideView from "./guide-view/GuideView";

const IR_SOUCE: string = `define i32 @main() {
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

; 文本框里的文本虽然不允许用户修改, 但不是一成不变的
; 绝大多数时候 Remusys Lens 的代码框不会存放整个 IR 文本, 只会存放当前锁定对象所在函数的 IR 文本片段
; 锁定的对象切换时, 如果前后两个对象不在同一个函数里, 则 IR 文本会被更新成新的函数的 IR 文本片段.
; 如果前后两个对象在同一个函数里, 则 IR 文本不变, 但编辑器会聚焦到新的锁定对象所在行,
; 以便用户看到当前锁定对象在 IR 中的位置.
`;

export default function App() {
  const flowReplaceText = (
    <>
      <h3>可视化视图, 使用 React Flow</h3>
      <p>根据导航视图中锁定的对象展示不同的图</p>
      <ul>
        <li>模块全局: 函数调用图</li>
        <li>函数: CFG / 支配树</li>
        <li>基本块: DFG</li>
        <li>指令: 数据流依赖图</li>
      </ul>
      <p>选择的框架</p>
      <ul>
        <li>
          <a href="https://reactflow.dev">React Flow</a>: 这玩意搞树状结构或者
          DAG 很好，但处理带环图非常糟糕，前向边和回边会交叉在一起打架
        </li>
        <li>
          <a href="">Cytoscape</a>: 没用过，不知道怎么个事儿
        </li>
      </ul>
    </>
  );
  return (
    <div className="app-root">
      {/* 左右分栏：左侧编辑器，右侧流程图 */}
      <ReflexContainer orientation="vertical" style={{ height: "100%" }}>
        <ReflexElement minSize={50} flex={40}>
          <div
            className="left-panel"
            style={{ height: "100%", display: "flex", flexDirection: "column" }}
          >
            {/* 上下分栏: 上面 Monaco Editor 只读视图, 下面多标签栏 */}
            <ReflexContainer
              orientation="horizontal"
              style={{ height: "100%" }}
            >
              <ReflexElement minSize={50} flex={70}>
                <div className="editor-wrap" style={{ flex: 1 }}>
                  <LensViewer irText={IR_SOUCE} />
                </div>
              </ReflexElement>
              <ReflexSplitter />
              <ReflexElement minSize={50} flex={30}>
                <GuideView />
              </ReflexElement>
            </ReflexContainer>
          </div>
        </ReflexElement>

        <ReflexSplitter />

        <ReflexElement flex={60}>
          <React.Suspense fallback={flowReplaceText}>
            <FlowViewer />
          </React.Suspense>
        </ReflexElement>
      </ReflexContainer>
    </div>
  );
}
