import { create } from "zustand";
import type { FlowGraphType } from "./FlowViewer";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

export type FlowState = {
  graphType: FlowGraphType;
};
export type FlowAction = {
  setGraphType: (type: FlowGraphType) => void;
  restoreGraphType: () => void;
};
export type FlowStore = FlowState & FlowAction;

export const useFlowStore = create<FlowStore>()(
  devtools(
    immer((set, _get) => ({
      graphType: { type: "Focus" },

      setGraphType(type: FlowGraphType) {
        set((state) => {
          state.graphType = type;
        });
      },
      restoreGraphType() {
        set((state) => {
          state.graphType = { type: "Focus" };
        });
      },
    })),
  ),
);
