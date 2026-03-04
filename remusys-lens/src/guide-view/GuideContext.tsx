import React from "react";
import type { NavEvent } from "./types";

interface GuideContextValue {
  onNavEvent: (event: NavEvent) => void;
}

export const GuideContext = React.createContext<GuideContextValue>({
  onNavEvent: () => {}
});
