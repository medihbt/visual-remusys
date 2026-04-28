import { useCallback, useState } from "react";
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
import {
  clearCachedSource,
  loadCachedSource,
  saveCachedSource,
} from "./source-cache";
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
              <ReflexContainer
                orientation="horizontal"
                style={{ height: "100%" }}
              >
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

// ---------------------------------------------------------------------------
// One-time synchronous boot — runs once as a useState lazy initializer.
// NOT an effect, so no setState-inside-effect ESLint warning.
// In React 18 StrictMode the initializer is guaranteed to execute only once,
// even though the render function body may be invoked twice.
// ---------------------------------------------------------------------------
function tryBoot(): void {
  const cached = loadCachedSource();
  if (!cached) return; // no cache → stay on FileLoader

  try {
    // Access the Zustand store directly — this is the documented escape
    // hatch for initializing stores outside React's render cycle.
    const { compile } = useIRStore.getState();
    const moduleInfo = compile(cached.type, cached.text, cached.filename);
    useGraphState.getState().initModule(moduleInfo);
  } catch (error) {
    clearCachedSource();
    console.warn(
      "Failed to restore cached source, fallback to FileLoader",
      error,
    );
    // Store stays empty → FileLoader will be shown.
  }
}

function App() {
  // ── boot — runs exactly once before first commit ──────────────────
  useState(tryBoot);

  // ── subscriptions ─────────────────────────────────────────────────
  const module = useIRStore((s) => s.module);
  const compile = useIRStore((s) => s.compile);
  const graphState = useGraphState();

  // ── user-initiated file load ──────────────────────────────────────
  const handleLoad = useCallback<LoadAction>(
    (mode, text, filename) => {
      try {
        const moduleInfo = compile(mode, text, filename);
        graphState.initModule(moduleInfo);
        saveCachedSource({ type: mode, text, filename });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        console.error("Failed to compile source:", message);
        alert(`编译失败：${message}`);
      }
    },
    [compile, graphState],
  );

  // ── render — no more null guard, boot is already finished ─────────
  return module ? (
    <MainPage onLoad={handleLoad} />
  ) : (
    <FileLoader onLoad={handleLoad} />
  );
}

export default App;
