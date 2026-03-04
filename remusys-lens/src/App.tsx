import { ReflexContainer, ReflexElement, ReflexSplitter } from "react-reflex";
import "./App.css";
import "react-reflex/styles.css";
import LensViewer from "./editor/LensViewer";
import FlowViewer from "./flow/FlowViewer";
import React from "react";
import { GuideView } from "./guide-view/GuideView";
import FileLoader from "./FileLoader";
import {
  ModuleCache,
  selectIRError,
  selectIRModule,
  selectIRStatus,
  useIRStore,
  type IRStoreStatus,
} from "./ir/ir-state";
import type { NavEvent } from "./guide-view/types";
import { idStringify, treeRefToSourceTrackable } from "./guide-view/guide-view-tree";

// 将导航事件的处理逻辑抽出为独立函数，避免组件内堆积业务代码
function handleNavEvent(event: NavEvent | null, clear: () => void) {
  if (!event) return;
  switch (event.type) {
    case "Focus": {
      const { nodeRef, kind, label } = event;
      const refDesc = idStringify(nodeRef);
      console.debug(`GuideView: Focus event received for nodeRef=${refDesc}, kind=${kind}, label=${label}`);
      try {
        const mapped = treeRefToSourceTrackable(nodeRef);
        console.debug('App.handleNavEvent: mapped treeRef ->', mapped);
        const s = useIRStore.getState();
        if (mapped) {
          console.debug('App.handleNavEvent: calling focusOn with', mapped);
          s.focusOn(mapped);
          console.debug('App.handleNavEvent: focusOn returned');
        } else {
          // Module-level focus
          console.debug('App.handleNavEvent: calling focusOn module sentinel');
          s.focusOn({ Module: true });
          console.debug('App.handleNavEvent: focusOn returned for module');
        }
      } catch (e) {
        console.warn('GuideView: focus mapping failed', e);
      }
      break;
    }
    case "ExpandOne": case "ExpandAll": case "Collapse": {
      const { nodeRef, kind } = event;
      const refDesc = idStringify(nodeRef);
      console.debug(`GuideView: ${event.type} event received for nodeRef=${refDesc}, kind=${kind}`);
      break;
    }
    case "ShowCfg": {
      const { funcDef } = event;
      alert(`显示函数 CFG:\n函数引用: ${funcDef}`);
      break;
    }
    case "ShowDominance": {
      const { funcDef } = event;
      alert(`显示函数支配树:\n函数引用: ${funcDef}`);
      break;
    }
    case "ShowDfg": {
      const { blockID } = event;
      alert(`显示基本块 DFG:\n基本块引用: ${blockID}`);
      break;
    }
    default:
      console.warn("GuideView: unknown NavEvent type", event);
      break;
  }
  clear();
}

// 不再在首屏自动加载假数据，用户需上传源文件后再触发加载
const flowReplaceText = <>
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
</>;

export class IRFocus {
  module: ModuleCache;
  status: IRStoreStatus;
  irText: string;
  setIRText: React.Dispatch<React.SetStateAction<string>>;


  constructor() {
    this.module = useIRStore(selectIRModule)!;
    this.status = useIRStore(selectIRStatus);
    const [irText, setIRText] = React.useState(this.module.brief.overview_src);
    this.irText = irText;
    this.setIRText = setIRText;
  }
}

export function MainPage() {
  const moduleCache = useIRStore(selectIRModule);
  const irStatus = useIRStore(selectIRStatus);
  const irError = useIRStore(selectIRError);
  const [navEvent, setNavEvent] = React.useState<NavEvent | null>(null);
  const sourceText = useIRStore((s) => s.sourceText);

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
                  <LensViewer irText={sourceText} />
                </div>
              </ReflexElement>
              <ReflexSplitter />
              <ReflexElement minSize={50} flex={30}>
                {moduleCache ? (
                  <GuideView
                    key={moduleCache.moduleId}
                    moduleCache={moduleCache}
                    onNavigate={setNavEvent}
                    incomingNavEvent={navEvent}
                    onConsumeNavEvent={(ev) => handleNavEvent(ev, () => setNavEvent(null))}
                  />
                ) : (
                  <div style={{ padding: 12, fontSize: 13, color: "#666" }}>
                    {irStatus === "error"
                      ? `GuideView init failed: ${irError ?? "unknown error"}`
                      : navEvent
                        ? "Loading module..."
                        : "Preparing GuideView..."}
                  </div>
                )}
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

export default function App() {
  const compileModule = useIRStore((state) => state.compileModule);
  const moduleCache = useIRStore(selectIRModule);

  return moduleCache ? (
    <MainPage />
  ) : (
    <FileLoader onLoad={(mode, text) => compileModule(mode, text)} />
  );
}