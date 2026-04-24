import { useCallback, useEffect, useState } from "react";
import type { SourceTy } from "remusys-wasm";
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
import { clearCachedSource, loadCachedSource, saveCachedSource } from "./source-cache";
import { useGraphState } from "./flow/state";

type LoadAction = (mode: SourceTy, text: string, filename: string) => void;

function MainPage({ onLoad }: { onLoad: LoadAction }) {
  return (
    <div className="app-root">
      <header>
        <AppMenu onLoad={onLoad} />
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
  const module = useIRStore((s) => s.module);
  const compile = useIRStore((s) => s.compile);
  const graphState = useGraphState();
  const [bootChecked, setBootChecked] = useState(false);

  const handleLoad = useCallback<LoadAction>((mode, text, filename) => {
    const moduleInfo = compile(mode, text, filename);
    graphState.initModule(moduleInfo);
    saveCachedSource({ type: mode, text, filename });
  }, [compile, graphState, module]);

  useEffect(() => {
    if (bootChecked) return;
    const cached = loadCachedSource();
    if (!cached) {
      setBootChecked(true);
      return;
    }

    try {
      const moduleInfo = compile(cached.type, cached.text, cached.filename);
      graphState.initModule(moduleInfo);
    } catch (error) {
      clearCachedSource();
      console.warn("Failed to restore cached source, fallback to FileLoader", error);
    } finally {
      setBootChecked(true);
    }
  }, [bootChecked, compile]);

  if (!bootChecked && !module) {
    return null;
  }

  return module ? <MainPage onLoad={handleLoad} /> : <FileLoader onLoad={handleLoad} />;
}

export default App;
