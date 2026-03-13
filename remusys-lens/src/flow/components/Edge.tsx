import type { Edge as RFEdge, EdgeProps } from "@xyflow/react";
import type { SourceTrackable } from "../../ir/ir";

export type FlowEdgeData = {
  mainPaths: string[];
  arrowPaths: string[];
  labelX: number;
  labelY: number;
  label: string;
  strokeColor?: string;
  irObjID?: SourceTrackable;
}

export type FlowEdge = RFEdge<FlowEdgeData, "flowEdge">;
export type FlowEdgeProps = EdgeProps<FlowEdge>;

export const FlowEdgeComp: React.FC<FlowEdgeProps> = (props) => {
  let mainPaths: string[] = props.data?.mainPaths ?? [];
  let arrowPaths: string[] = props.data?.arrowPaths ?? [];

  let id = props.id ?? '';
  if (typeof id !== 'string')
    throw new Error(`Edge id is expected to be string, got ${typeof id}`);
  const strokeColor = props.data?.strokeColor ?? '#222';

  const mainElems = mainPaths.map((path, idx) => (
    <path key={`m-${idx}`} id={`${id}-main-${idx}`} d={path} stroke={strokeColor} strokeWidth={1} fill="none" />
  ));
  const arrowElems = arrowPaths.map((path, idx) => (
    <path key={`a-${idx}`} id={`${id}-arrow-${idx}`} d={path} stroke={strokeColor} strokeWidth={1} fill={strokeColor} />
  ));
  const textElem = props.label ? (
    <text
      x={props.data?.labelX} y={props.data?.labelY}
      textAnchor="middle" dominantBaseline="central"
      fontSize={9} fill="#222"
    >
      {props.label}
    </text>
  ) : null;

  return <g id={id}>{mainElems}{arrowElems}{textElem}</g>;
}
export const FlowEdgeTypes = {
  flowEdge: FlowEdgeComp,
}
