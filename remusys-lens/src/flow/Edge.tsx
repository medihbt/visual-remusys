/**
 * # Flow View Edges
 *
 * Flow View 会展示比较复杂的图, 这个时候如果还用 React Flow 默认的 Edge 组件的话
 * 那整张图就会自己打架了. 因此我们需要自己实现一个 Edge 组件, 接入布局器的边路由算法,
 * 自己来渲染边的路径.
 *
 * 和 b1 不同的是, b2 的 edges 数据来源是 dagre 而不是 GraphViz 了.
 * 因此, edge 的边路由路径信息要改成适配 dagre 的格式.
 */

import type { IRTreeObjID } from "remusys-wasm";
import { type Edge as RFEdge, type EdgeProps, BaseEdge } from "@xyflow/react";

export type FlowEdgeData = {
  path: string;
  labelPosition: { x: number; y: number };
  isFocused?: boolean;
  irObjID?: IRTreeObjID;
};

export type FlowEdge = RFEdge<FlowEdgeData, "FlowEdge">;
export type FlowEdgeProps = EdgeProps<FlowEdge>;

export default function FlowEdgeComp(props: FlowEdgeProps) {
  const {
    data,
    style,
    label,
    labelStyle,
    markerStart,
    markerEnd,
    interactionWidth,
  } = props;
  if (!data)
    throw new Error("FlowEdge requires data prop with path and labelPosition");
  const { path, labelPosition, isFocused = false } = data;
  const strokeWidth = isFocused ? 1.5 : 1;

  return (
    <BaseEdge
      path={path}
      label={label}
      labelX={labelPosition.x}
      labelY={labelPosition.y}
      markerStart={markerStart}
      markerEnd={markerEnd}
      style={style}
      labelStyle={labelStyle}
      interactionWidth={interactionWidth}
      strokeWidth={strokeWidth}
    />
  );
}
