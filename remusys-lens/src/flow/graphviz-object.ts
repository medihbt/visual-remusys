/**
 * Graphviz JSON output format TypeScript definitions
 * Based on official Graphviz JSON schema
 */

export type Point = [number, number];
export type Point3 = [number, number, number];
export type Rectangle = [number, number, number, number]; // [x1, y1, x2, y2]
export type PointList = Point[];
export type Color = string; // Format: "#RGB" or "#RRGGBB" or "#RRGGBBAA"

export interface Stop {
  frac: number;
  color: Color;
}

export type DrawOp = 
  | Ellipse
  | Polygon
  | Polyline
  | BSpline
  | Text
  | FontStyle
  | DrawColor
  | Font
  | Style;

export interface Ellipse {
  op: "e" | "E";
  rect: Rectangle;
}

export interface Polygon {
  op: "p" | "P";
  points: PointList;
}

export interface Polyline {
  op: "L";
  points: PointList;
}

export interface BSpline {
  op: "b" | "B";
  points: PointList;
}

export interface Text {
  op: "T";
  pt: Point;
  align: "l" | "c" | "r";
  text: string;
  width: number;
}

export interface FontStyle {
  op: "t";
  fontchar: number; // 0-127 ASCII
}

export interface Font {
  op: "F";
  size: number; // >= 0
  face: string;
}

export interface DrawColor {
  op: "c" | "C";
  grad: "none" | "linear" | "radial";
  color?: Color;
  p0?: Point | Point3;
  p1?: Point | Point3;
  stops?: Stop[];
}

export interface Style {
  op: "S";
  style: string;
}

export type DrawOps = DrawOp[];

/**
 * Graphviz JSON MetaNode definition
 * 
 * @property _gvid Unique integer ID for this node or subgraph
 * @property name The node or subgraph name
 * @property _draw_ Optional array of draw operations for this node/subgraph
 * @property _ldraw_ Optional array of draw operations for this node/subgraph's label
 * @property nodes Index of a node in this subgraph
 * @property edges Index of an edge in this subgraph
 * @property subgraphs Index of a child subgraph
 * @property [key: string] Any additional attributes from the Graphviz JSON output
 */
export interface MetaNode {
  _gvid: number;
  name: string;
  _draw_?: DrawOps;
  _ldraw_?: DrawOps;
  nodes?: number[];
  edges?: number[];
  subgraphs?: number[];
  [key: string]: string | number | DrawOps | number[] | undefined;
}

export interface Edge {
  _gvid: number;
  tail: number; // _gvid of tail node
  head: number; // _gvid of head node
  _draw_?: DrawOps;
  _ldraw_?: DrawOps;
  _hdraw_?: DrawOps;
  _hldraw_?: DrawOps;
  _tdraw_?: DrawOps;
  _tldraw_?: DrawOps;
  [key: string]: string | number | DrawOps | undefined;
}

export interface GraphvizJSON {
  name: string; // The graph name
  directed: boolean; // True if the graph is directed
  strict: boolean; // True if the graph is strict
  _subgraph_cnt: number; // Number of subgraphs in the graph
  _draw_?: DrawOps;
  _ldraw_?: DrawOps;
  objects?: MetaNode[]; // The graph's subgraphs followed by the graph's nodes
  edges?: Edge[];
  [key: string]: string | boolean | number | DrawOps | MetaNode[] | Edge[] | undefined;
}