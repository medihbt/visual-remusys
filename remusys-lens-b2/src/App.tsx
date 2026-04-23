import { ReflexContainer, ReflexElement, ReflexSplitter } from "react-reflex";
import "react-reflex/styles.css";
import GuideView from "./guide_view/GuideView";
import "./App.css";
import AppMenu from "./AppMenu";
import { useIRStore } from "./ir/state";
import FileLoader from "./FileLoader";
import { useGraphState } from "./flow/state";

import "@xyflow/react/dist/style.css";

interface PanePlaceholderProps {
  title: string;
  description?: string | React.ReactNode;
}

function PanePlaceholder({ title, description = "施工中" }: PanePlaceholderProps) {
  return (
    <div className="pane-placeholder">
      <h3>{title}</h3>
      <p>{description}</p>
    </div>
  );
}

function MainPage() {
  const compileModule = useIRStore((st) => st.compile);
  return (
    <div className="app-root">
      <header>
        <AppMenu onLoad={compileModule} />
      </header>

      <main className="app-main">
        <ReflexContainer orientation="vertical" style={{ height: "100%" }}>
          <ReflexElement minSize={50} flex={40}>
            <section className="panel-left">
              <ReflexContainer orientation="horizontal" style={{ height: "100%" }}>
                <ReflexElement minSize={50} flex={60}>
                  <div className="left-top">
                    <PanePlaceholder title="源码编辑器" />
                  </div>
                </ReflexElement>

                <ReflexSplitter />

                <ReflexElement minSize={50} flex={40}>
                  <GuideView />
                </ReflexElement>
              </ReflexContainer>
            </section>
          </ReflexElement>

          <ReflexSplitter />

          <ReflexElement minSize={50} flex={60}>
            <PanePlaceholder title="图视图" description={`当前图类型: ${JSON.stringify(useGraphState().graphType)}`} />
          </ReflexElement>
        </ReflexContainer>
      </main>
    </div>
  );
}

function App() {
  const module = useIRStore(s => s.module);
  const onLoad = useIRStore(s => s.compile);
  return module ? <MainPage /> : <FileLoader onLoad={onLoad} />;
}

export default App;
