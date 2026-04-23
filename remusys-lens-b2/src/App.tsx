import { ReflexContainer, ReflexElement, ReflexSplitter } from "react-reflex";
import "react-reflex/styles.css";
import GuideView from "./guide_view/GuideView";
import "./App.css";
import AppMenu from "./AppMenu";
import { useIRStore } from "./ir/state";
import FileLoader from "./FileLoader";
import FlowViewer from "./flow/FlowViewer";

import "@xyflow/react/dist/style.css";
import SourceView from "./source_view/SourceView";

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
                  <SourceView />
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
            <FlowViewer />
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
