import { ReflexContainer, ReflexElement, ReflexSplitter } from "react-reflex";
import "./App.css";
import "react-reflex/styles.css";
import "@xyflow/react/dist/style.css";
import LensViewer from "./editor/LensViewer";
import FlowViewer, { type FlowGraphType } from "./flow/FlowViewer";
import React from "react";
import { GuideView } from "./guide-view/GuideView";
import FileLoader from "./FileLoader";
import {
  selectIRError,
  selectIRModule,
  selectIRStatus,
  useIRStore,
} from "./ir/ir-state";
import type { FocusEvent, NavEvent } from "./guide-view/types";
import TopMenu from "./TopMenu";
import { sourceTrackableToString } from "./ir/ir";

// 将导航事件的处理逻辑抽出为独立函数，避免组件内堆积业务代码
function handleNavEvent(
  event: NavEvent | null,
  setGraph: React.Dispatch<React.SetStateAction<FlowGraphType | undefined>>,
  clear: () => void,
) {
  if (!event) return;
  switch (event.type) {
    case "Focus": {
      handleNavFocus(event);
      setGraph(undefined);
      break;
    }
    case "ExpandOne":
    case "ExpandAll":
    case "Collapse": {
      const { nodeRef, kind } = event;
      const refDesc = sourceTrackableToString(nodeRef);
      console.debug(
        `GuideView: ${event.type} event received for nodeRef=${refDesc}, kind=${kind}`,
      );
      break;
    }
    case "ShowCfg": {
      const { funcDef } = event;
      console.debug(`GuideView: ShowCfg event for funcDef=${funcDef}`);
      setGraph({ type: "FuncCfg", func: funcDef });
      break;
    }
    case "ShowDominance": {
      const { funcDef } = event;
      console.debug(`GuideView: ShowDominance event for funcDef=${funcDef}`);
      setGraph({ type: "FuncDom", func: funcDef });
      break;
    }
    case "ShowDfg": {
      const { blockID } = event;
      console.debug(`GuideView: ShowDfg event for blockID=${blockID}`);
      setGraph({ type: "BlockDfg", block: blockID });
      break;
    }
    case "ShowValueDefUse": {
      const { valueID } = event;
      console.debug(`GuideView: ShowValueDefUse event for valueID=${valueID}`);
      setGraph({ type: "DefUse", center: valueID });
      break;
    }
    default:
      console.warn("GuideView: unknown NavEvent type", event);
      setGraph(undefined);
      break;
  }
  clear();
}

function handleNavFocus(event: FocusEvent) {
  const { nodeRef, kind, label } = event;
  const refDesc = sourceTrackableToString(nodeRef);
  console.debug(
    `GuideView: Focus event received for nodeRef=${refDesc}, kind=${kind}, label=${label}`,
  );
  try {
    const mapped = nodeRef;
    console.debug("App.handleNavFocus: mapped treeRef ->", mapped);
    const s = useIRStore.getState();
    if (mapped) {
      console.debug("App.handleNavFocus: calling focusOn with", mapped);
      s.focusOn(mapped);
      console.debug("App.handleNavFocus: focusOn returned");
    } else {
      // Module-level focus
      console.debug("App.handleNavFocus: calling focusOn module sentinel");
      s.focusOn({ type: "Module" });
      console.debug("App.handleNavFocus: focusOn returned for module");
    }
  } catch (e) {
    console.warn("GuideView: focus mapping failed", e);
  }
}

// 不再在首屏自动加载假数据，用户需上传源文件后再触发加载
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
        <a href="https://reactflow.dev">React Flow</a>: 这玩意搞树状结构或者 DAG
        很好，但处理带环图非常糟糕，前向边和回边会交叉在一起打架
      </li>
      <li>
        <a href="">Cytoscape</a>: 没用过，不知道怎么个事儿
      </li>
    </ul>
  </>
);

export function MainPage() {
  const compileModule = useIRStore((state) => state.compileModule);
  const moduleCache = useIRStore(selectIRModule);
  const moduleId = moduleCache?.moduleId ?? null;
  const irStatus = useIRStore(selectIRStatus);
  const irError = useIRStore(selectIRError);
  const [navEvent, setNavEvent] = React.useState<NavEvent | null>(null);
  const sourceText = useIRStore((s) => s.sourceText);
  const [graph, setGraph] = React.useState<FlowGraphType | undefined>(
    undefined,
  );

  React.useEffect(() => {
    setNavEvent(null);
    setGraph(undefined);
  }, [moduleId]);

  let guideViewStatus: string;
  if (irStatus === "error") {
    guideViewStatus = `GuideView init failed: ${irError ?? "unknown error"}`;
  } else if (navEvent) {
    guideViewStatus = "Loading module...";
  } else {
    guideViewStatus = "Preparing GuideView...";
  }

  return (
    <div className="app-root">
      <TopMenu
        onLoad={(ty, src) => {
          compileModule(ty, src);
        }}
      />
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
                  <LensViewer irText={sourceText} srcType="ir" />
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
                    onConsumeNavEvent={(ev) =>
                      handleNavEvent(ev, setGraph, () => setNavEvent(null))
                    }
                  />
                ) : (
                  <div style={{ padding: 12, fontSize: 13, color: "#666" }}>
                    {guideViewStatus}
                  </div>
                )}
              </ReflexElement>
            </ReflexContainer>
          </div>
        </ReflexElement>

        <ReflexSplitter />

        <ReflexElement flex={60}>
          <React.Suspense fallback={flowReplaceText}>
            <FlowViewer fgGraph={graph} />
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
    <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      <MainPage />
    </div>
  ) : (
    <FileLoader onLoad={(mode, text) => compileModule(mode, text)} />
  );
}
