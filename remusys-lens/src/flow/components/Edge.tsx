import type { Edge as RFEdge, EdgeProps } from "@xyflow/react";
import type { SourceTrackable } from "../../ir/ir";

export type FlowEdgeData = {
  mainPaths: string[];
  arrowPaths: string[];
  labelX: number;
  labelY: number;
  label: string;
  strokeColor?: string;
  isFocused?: boolean;
  dashPattern?: `${number} ${number}` | "none";
  dashAndLine?: boolean;
  irObjID?: SourceTrackable;
};

export type FlowEdge = RFEdge<FlowEdgeData, "flowEdge">;
export type FlowEdgeProps = EdgeProps<FlowEdge>;

export const FlowEdgeComp: React.FC<FlowEdgeProps> = (props) => {
  const mainPaths: string[] = props.data?.mainPaths ?? [];
  const arrowPaths: string[] = props.data?.arrowPaths ?? [];

  const id = props.id;
  const strokeColor = props.data?.strokeColor ?? "#222";
  const strokeWidth = props.data?.isFocused ? 1.5 : 1;

  const mainElems = mainPaths.map((path, idx) => (
    <path
      key={`m-${idx}`}
      id={`${id}-main-${idx}`}
      d={path}
      stroke={strokeColor}
      strokeWidth={strokeWidth}
      fill="none"
      strokeDasharray={props.data?.dashPattern ?? "none"}
    />
  ));
  if (props.data?.dashAndLine && props.data?.dashPattern && props.data.dashPattern !== "none") {
    mainElems.push(...mainPaths.map((path, idx) => (
      <path
        key={`d-${idx}`}
        id={`${id}-dash-${idx}`}
        d={path}
        stroke={strokeColor}
        strokeWidth={strokeWidth * 0.7}
        fill="none"
      />
    )));
  }
  const arrowElems = arrowPaths.map((path, idx) => (
    <path
      key={`a-${idx}`}
      id={`${id}-arrow-${idx}`}
      d={path}
      stroke={strokeColor}
      strokeWidth={strokeWidth}
      fill={strokeColor}
    />
  ));
  const textElem = props.label ? (
    <text
      x={props.data?.labelX}
      y={props.data?.labelY}
      textAnchor="middle"
      dominantBaseline="central"
      fontSize={9}
      fontWeight={props.data?.isFocused ? "bold" : "normal"}
      fill={strokeColor}
    >
      {props.label}
    </text>
  ) : null;

  return (
    <g id={id}>
      {mainElems}
      {arrowElems}
      {textElem}
    </g>
  );
};
export const FlowEdgeTypes = {
  flowEdge: FlowEdgeComp,
};
