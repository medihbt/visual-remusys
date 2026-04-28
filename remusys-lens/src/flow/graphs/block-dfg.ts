/**
 * # BlockDFG -- 基本块内部的数据流切片
 *
 * BlockDFG 是一个针对单个基本块内部的分节图. 它的结点代表基本块内的指令, 边代表指令之间的数据流关系.
 *
 * ## 为什么要分节
 *
 * 在 CFG 中，基本块之间只有纯粹的跳转关系, 没有严格的执行顺序要求. 如果 BlockDFG 的性质和 CFG 一样简单就好了,
 * 但现实并非如此. 基本块内部的指令除了数据流关系以外还有一个很重要的关系: 执行顺序. 有时这个执行顺序是无关紧要
 * 的，比如在指令列表的某一段切片内的每一条指令都是纯粹的计算指令, 那这些指令之间只有数据流关系, 怎么打乱都没问题.
 * 但有时这个执行顺序又是非常重要的, 比如遇到了 call store 这类带副作用的指令, 此时如果用单纯的流图来表示的话
 * 就表达不出这层顺序关系, 甚至在排版时排版算法会把后执行的指令排在前面, 这就完全违背了用户的直觉.
 *
 * 因此我们需要在 BlockDFG 中引入一个新的概念: 分节. 把原本在基本块内的指令列表划分成若干段, 每段内的指令之间
 * 没有严格的执行顺序要求, 但段与段之间的执行顺序是严格的. 这样我们就可以在段与段之间添加一些虚拟的控制依赖边来
 * 表达这个顺序关系, 同时段内的指令之间就可以自由地根据数据流关系进行排布了.
 *
 * 但是, 对于那些充满了 load store call 之类的指令的基本块, 可能每条指令都必须单独成段, 这样排出来的图信息密度
 * 就非常低了. 因此我们还需要一个折衷的方案: 把连续的带副作用的指令节合并成一个，并且给每个节加上类型标签. 如果
 * 类型标签显示不是 Pure 的话, 就说明这个节内的指令之间是有严格执行顺序要求的, 这时排版算法应当该尽量保持它们的
 * 相对位置不变.
 *
 * ## 分节方案
 *
 * `BlockDfg.sections` 是一个顺序列表，直接反映了基本块内指令的执行顺序与角色分区：
 *
 * - **前两个 section 固定**：索引 0 为 `Income`（跨块输入），索引 1 为 `Outgo`（跨块输出）。
 * - **索引 2 起为内部指令段**：WASM 侧遍历块内指令列表，按 `DfgNodeRole` 将连续同角色指令
 *   合并为一段；角色发生变化时开启新段。因此段的数量和类型完全由块内指令序列决定。
 * - **Income 段**：仅收录来自块外的 `Inst` 和 `FuncArg`，全局去重。这些结点是跨块数据流的
 *   "入口"，在图中通常应放在最左侧或最上方。
 * - **Outgo 段**：收录被块外使用的结点（即 user 不在本块内的定义）。这些结点是跨块数据流的
 *   "出口"，通常与 Income 相对放置。
 * - **内部段**：按实际角色分为 `Pure`、`Effect`、`Terminator`、`Phi`。`Pure` 段内指令无
 *   副作用，可自由重排；`Effect` / `Terminator` / `Phi` 段则应尽量保持原始相对顺序。
 *
 * 每个 section 在 React Flow 中映射为一个 `groupNode`（子图容器），其内部的 `DfgNode`
 * 映射为 `elemNode`。这种双层结构（Section → Node）是 BlockDfg 与普通单层图（CFG、
 * CallGraph 等）的根本区别，也是必须使用 elkjs 而非 dagre 的原因。
 *
 * ## 前端应该做什么
 *
 * ### 1. 数据转换
 *
 * 从 `IRState.getBlockDfg(blockId)` 获取 `BlockDfg` DTO 后，需要把 `sections` 和 `edges`
 * 转换为 React Flow 的 `FlowNode[]` 和 `FlowEdge[]`：
 *
 * - **Section → `FlowGroupNode`**：id 建议用 `section-{index}`，label 显示角色名
 *   （如 `"Pure"` / `"Effect"`），`type: "groupNode"`。背景色可根据角色区分，与 WASM
 *   侧 `to_dot_text` 的配色保持一致（`Income` 蓝、`Outgo` 绿、`Phi` 黄、`Pure` 灰、
 *   `Effect` 红、`Terminator` 橙）。
 * - **DfgNode → `FlowElemNode`**：id 用 `DfgNodeID` 的字符串形式，放入对应 section
 *   容器的 `children`（或通过 elkjs 父子关系表达）。label 显示结点标识和值信息。
 * - **DfgEdge → `FlowEdge`**：WASM 侧返回的边方向为 **use → def**（`from` 是使用者，
 *   `to` 是操作数/定义者）。前端可直接沿用此方向，也可根据展示习惯反向为 def → use，
 *   但需在全图保持一致。
 *
 * ### 2. 虚拟控制依赖边（保序）
 *
 * 为了表达段与段之间的严格执行顺序，需要在相邻 section 之间插入**虚拟控制依赖边**。
 * 具体策略：
 *
 * - 方案 A（容器级）：在上一个 section 的容器结点与下一个 section 的容器结点之间加边。
 *   优点是简洁；缺点是 elkjs 对容器间边的路由支持有限。
 * - 方案 B（代理结点级）：在每个 section 内选第一个和最后一个结点作为 "段头/段尾"，
 *   段尾 → 下一段头 加虚拟边。优点是对布局影响更精确；缺点是逻辑稍复杂。
 *
 * 无论哪种方案，虚拟边应当使用虚线样式（`style: { strokeDasharray: '5,5' }`）并配以浅色，
 * 与真实数据流边区分。
 *
 * ### 3. 段内结点处理
 *
 * - **`Pure` / `Phi` 段**：结点之间没有执行顺序约束，elkjs 可根据数据流边自由排布。不需要额外处理。
 * - **`Effect` / `Terminator` 段**：应尽量保持 WASM 侧给出的原始顺序。
 *   可通过 elkjs 的 `position` 预设（如固定 y 坐标递增）或 `layering.strategy: 'LONGEST_PATH'`
 *   结合人工 layer 约束实现。若效果不佳，也可退化为段内简单的线性排列。
 *
 * ### 4. 跨块结点去重规则
 *
 * WASM 侧已经对 `Inst` 和 `FuncArg` 做了全局去重（放入 Income 段）。但其他操作数
 * （如常量、表达式、全局变量）是以 `Use(edge)` 形式分散在各使用者所在段内的，且**不去重**。
 * 这意味着同一条数据可能在图中出现多次。前端不需要也不应该再做去重，这是设计上的有意取舍：
 * 块外结点集中去重以减少混乱，块内局部数据流保持完整以避免跨段引用。
 *
 * ### 5. 交互
 *
 * 目前暂时不做交互。写论文要紧。如果后续要加，可以考虑：
 * - 点击结点高亮其上下游数据流路径。
 * - hover 结点显示完整 `ValueDt` 和 `UseKind`。
 * - 支持折叠/展开某个 section。
 *
 * ## 单独的 Elk 排版方案概述
 *
 * BlockDfg 是 Section-Node 双层嵌套图，dagre 不支持子图布局，因此必须使用 **elkjs**。
 *
 * ### 为什么选 elkjs
 *
 * - dagre 只能排单层图，无法表达 "section 容器包裹 node" 的嵌套结构。
 * - elkjs（Eclipse Layout Kernel）原生支持层级图（hierarchical graph），通过父子结点
 *   关系即可实现子图嵌套，API 虽然复杂但至少类型清晰、文档完善，不像 GraphViz 那样
 *   接口模糊且难以在前端调用。
 *
 * ### elkjs 图结构映射
 *
 * elkjs 的输入图由 ` ElkNode[]` 和 `ElkEdge[]` 组成。映射关系如下：
 *
 * | React Flow 概念 | elkjs 概念 | 说明 |
 * |----------------|-----------|------|
 * | `groupNode` (section) | `ElkNode` 含 `children: ElkNode[]` | 每个 section 是一个父结点 |
 * | `elemNode` (dfg node) | `ElkNode` 放在父结点的 `children` 中 | 实际的指令/表达式结点 |
 * | `FlowEdge` | `ElkEdge` | 数据流边 + 虚拟控制依赖边 |
 *
 * ### 推荐布局选项
 *
 * ```ts
 * const elkOptions = {
 *   'elk.algorithm': 'layered',
 *   'elk.direction': 'DOWN',              // 或 RIGHT，section 按执行顺序从上到下/左到右
 *   'elk.hierarchyHandling': 'INCLUDE_CHILDREN', // 关键：同时排布外层 section 和内层 node
 *   'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES', // 尽量尊重原始顺序
 *   'elk.spacing.nodeNode': '24',
 *   'elk.spacing.componentComponent': '40',
 * };
 * ```
 *
 * ### 布局流程
 *
 * 1. 把 `BlockDfg.sections` 转成 elkjs 的 `ElkNode` 列表，每个 section 结点包含其
 *    `DfgNode` 子结点列表。
 * 2. 把 `BlockDfg.edges` 转成 `ElkEdge` 列表（注意方向保持 use → def 或统一反转）。
 * 3. 在相邻 section 之间插入虚拟控制依赖边（如方案 A/B 所述）。
 * 4. 调用 `elk.layout(graph)` 获取排布后的绝对坐标。
 * 5. 把 elkjs 返回的坐标映射回 React Flow 的 `position: { x, y }`。
 *    - section 容器的坐标直接取 elk 父结点的 `(x, y)`，宽高取 `(width, height)`。
 *    - 内部 elemNode 的坐标需累加父容器的偏移（elk 返回的是局部坐标还是绝对坐标取决于
 *      配置，通常 `INCLUDE_CHILDREN` 会返回绝对坐标，但仍需验证）。
 * 6. 渲染。
 *
 * ### 注意事项
 *
 * - elkjs 的 `children` 坐标在 `INCLUDE_CHILDREN` 模式下通常是绝对坐标，可直接使用；
 *   若发现错位，检查是否需要加上父结点的 `x/y` 偏移。
 * - `Effect` / `Terminator` 段若需强制保序，可在段内相邻结点之间也加短虚拟边，
 *   或设置 `elk.layered.crossingMinimization.semiInteractive: true` 并预分配 layer。
 * - elkjs 是异步的（Web Worker 版），布局调用应放在 `useEffect` 或异步 action 中，
 *   避免阻塞 UI。
 */

import Elk from "elkjs/lib/elk.bundled.js";
import type {
  ElkEdgeSection,
  ElkExtendedEdge,
  ElkNode,
} from "elkjs/lib/elk-api";
import type {
  BlockDfg,
  BlockID,
  DfgEdge,
  DfgNode,
  DfgNodeRole,
  DfgSection,
  IRObjPath,
  IRTreeObjID,
} from "remusys-wasm";

import type { IRState } from "../../ir/state";
import type { FlowEdge } from "../Edge";
import {
  GROUP_NODE_SOURCE_HANDLE_ID,
  GROUP_NODE_TARGET_HANDLE_ID,
} from "../Node";
import type { FlowElemNode, FlowGroupNode, FlowNode } from "../Node";
import type { FlowGraph } from "./layout";

type SectionModel = {
  index: number;
  id: string;
  kind: DfgNodeRole;
  nodes: DfgNode[];
};

type SectionPalette = {
  bgColor: string;
  borderColor: string;
};

const elk = new Elk();

const SECTION_WIDTH = 280;
const SECTION_HEADER_HEIGHT = 30;
const SECTION_PADDING = 16;
const NODE_WIDTH = 180;
const NODE_HEIGHT = 44;
const SECTION_GAP = 48;
const ORDER_EDGE_COLOR = "#9ca3af";

function rolePalette(role: DfgNodeRole): SectionPalette {
  switch (role) {
    case "Income":
      return { bgColor: "#dbeafe", borderColor: "#2563eb" };
    case "Outgo":
      return { bgColor: "#dcfce7", borderColor: "#16a34a" };
    case "Phi":
      return { bgColor: "#fef3c7", borderColor: "#d97706" };
    case "Pure":
      return { bgColor: "#f3f4f6", borderColor: "#4b5563" };
    case "Effect":
      return { bgColor: "#fee2e2", borderColor: "#dc2626" };
    case "Terminator":
      return { bgColor: "#ffedd5", borderColor: "#ea580c" };
  }
}

function dfgNodeToIRObjID(node: DfgNode): IRTreeObjID | null {
  switch (node.value.type) {
    case "Global":
      return { type: "Global", value: node.value.value };
    case "FuncArg":
      return { type: "FuncArg", value: node.value.value };
    case "Block":
      return { type: "Block", value: node.value.value };
    case "Inst":
      return { type: "Inst", value: node.value.value };
    default:
      return null;
  }
}

function edgeToIRObjID(edge: DfgEdge): IRTreeObjID {
  return { type: "Use", value: edge.id };
}

function focusMatchesIRObject(
  focus: IRObjPath,
  obj: IRTreeObjID | null,
): boolean {
  if (!obj || focus.length === 0) return false;
  const current = focus[focus.length - 1];
  if (current.type !== obj.type) return false;
  if (current.type === "Module" || obj.type === "Module") return true;
  return current.value === obj.value;
}

function sectionNodeId(index: number): string {
  return `section-${index}`;
}

function sectionOrderEdgeId(fromIndex: number, toIndex: number): string {
  return `section-order:${fromIndex}->${toIndex}`;
}

function sectionLabel(section: SectionModel): string {
  return `${section.kind} ${section.index}`;
}

function makeSectionModels(blockDfg: BlockDfg): SectionModel[] {
  return blockDfg.sections.map((section, index) => ({
    index,
    id: sectionNodeId(index),
    kind: section.kind,
    nodes: section.nodes,
  }));
}

function makeGroupNodes(
  sections: SectionModel[],
  focus: IRObjPath,
): FlowGroupNode[] {
  return sections.map((section) => {
    const palette = rolePalette(section.kind);
    const focused = section.nodes.some((node) =>
      focusMatchesIRObject(focus, dfgNodeToIRObjID(node)),
    );
    return {
      id: section.id,
      type: "groupNode",
      position: { x: 0, y: 0 },
      width: SECTION_WIDTH,
      height:
        SECTION_HEADER_HEIGHT +
        SECTION_PADDING * 2 +
        Math.max(section.nodes.length, 1) * (NODE_HEIGHT + 12),
      data: {
        label: sectionLabel(section),
        focused,
        irObjID: null,
        bgColor: palette.bgColor,
      },
      style: {
        border: `1px solid ${palette.borderColor}`,
        borderRadius: 10,
      },
    };
  });
}

function makeElemNodes(
  sections: SectionModel[],
  focus: IRObjPath,
): FlowElemNode[] {
  return sections.flatMap((section) => {
    const palette = rolePalette(section.kind);
    return section.nodes.map((node, index) => ({
      id: node.id,
      type: "elemNode",
      position: {
        x: SECTION_PADDING,
        y: SECTION_HEADER_HEIGHT + SECTION_PADDING + index * (NODE_HEIGHT + 12),
      },
      width: NODE_WIDTH,
      height: NODE_HEIGHT,
      parentId: section.id,
      extent: "parent",
      data: {
        label: node.label,
        focused: focusMatchesIRObject(focus, dfgNodeToIRObjID(node)),
        irObjID: dfgNodeToIRObjID(node),
        bgColor: palette.bgColor,
      },
    }));
  });
}

function makeDataEdges(wasmEdges: DfgEdge[], focus: IRObjPath): FlowEdge[] {
  return wasmEdges.map((edge) => ({
    id: edge.id,
    source: edge.from,
    target: edge.to,
    type: "FlowEdge",
    label: edge.label,
    data: {
      path: "",
      labelPosition: { x: 0, y: 0 },
      isFocused: focusMatchesIRObject(focus, edgeToIRObjID(edge)),
      irObjID: edgeToIRObjID(edge),
    },
  }));
}

function makeSectionOrderEdges(sections: SectionModel[]): FlowEdge[] {
  const edges: FlowEdge[] = [];
  for (let index = 0; index + 1 < sections.length; index++) {
    const current = sections[index];
    const next = sections[index + 1];
    edges.push({
      id: sectionOrderEdgeId(current.index, next.index),
      source: current.id,
      sourceHandle: GROUP_NODE_SOURCE_HANDLE_ID,
      target: next.id,
      targetHandle: GROUP_NODE_TARGET_HANDLE_ID,
      type: "FlowEdge",
      selectable: false,
      focusable: false,
      label: "",
      style: {
        stroke: ORDER_EDGE_COLOR,
        strokeDasharray: "5 5",
        strokeWidth: 1,
      },
      data: {
        path: "",
        labelPosition: { x: 0, y: 0 },
        isFocused: false,
      },
    });
  }
  return edges;
}

function sectionChildHeight(section: DfgSection): number {
  return Math.max(section.nodes.length, 1) * (NODE_HEIGHT + 12) - 12;
}

function buildElkGraph(
  sections: SectionModel[],
  nodes: FlowElemNode[],
  edges: FlowEdge[],
): ElkNode {
  const childMap = new Map<string, FlowElemNode[]>();
  for (const node of nodes) {
    const parentId = node.parentId;
    if (!parentId) continue;
    const parentNodes = childMap.get(parentId);
    if (parentNodes) parentNodes.push(node);
    else childMap.set(parentId, [node]);
  }

  return {
    id: "block-dfg-root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.hierarchyHandling": "INCLUDE_CHILDREN",
      "elk.layered.spacing.nodeNodeBetweenLayers": "40",
      "elk.spacing.nodeNode": "24",
      "elk.spacing.componentComponent": String(SECTION_GAP),
    },
    children: sections.map((section) => ({
      id: section.id,
      width: SECTION_WIDTH,
      height:
        SECTION_HEADER_HEIGHT +
        SECTION_PADDING * 2 +
        sectionChildHeight({ kind: section.kind, nodes: section.nodes }),
      layoutOptions: {
        "elk.algorithm": "layered",
        "elk.direction": "DOWN",
        "elk.padding": `[top=${SECTION_HEADER_HEIGHT + SECTION_PADDING},left=${SECTION_PADDING},bottom=${SECTION_PADDING},right=${SECTION_PADDING}]`,
        "elk.spacing.nodeNode": "12",
      },
      children: (childMap.get(section.id) ?? []).map((node) => ({
        id: node.id,
        width: node.width ?? NODE_WIDTH,
        height: node.height ?? NODE_HEIGHT,
      })),
    })),
    edges: edges.map<ElkExtendedEdge>((edge) => ({
      id: edge.id,
      sources: [edge.source],
      targets: [edge.target],
    })),
  };
}

function findElkNode(root: ElkNode, id: string): ElkNode | null {
  if (root.id === id) return root;
  for (const child of root.children ?? []) {
    const found = findElkNode(child, id);
    if (found) return found;
  }
  return null;
}

function edgePathFromSections(sections: ElkEdgeSection[]): {
  path: string;
  labelPosition: { x: number; y: number };
} {
  const points: Array<{ x: number; y: number }> = [];
  for (const section of sections) {
    if (section.startPoint) points.push(section.startPoint);
    for (const point of section.bendPoints ?? []) points.push(point);
    if (section.endPoint) points.push(section.endPoint);
  }
  if (points.length === 0) {
    return { path: "", labelPosition: { x: 0, y: 0 } };
  }
  const [firstPoint, ...restPoints] = points;
  const path = [
    `M ${firstPoint.x} ${firstPoint.y}`,
    ...restPoints.map((point) => `L ${point.x} ${point.y}`),
  ].join(" ");
  const middlePoint = points[Math.floor(points.length / 2)] ?? firstPoint;
  return {
    path,
    labelPosition: { x: middlePoint.x, y: middlePoint.y },
  };
}

function resolveChildNodePosition(
  elkNode: ElkNode,
  parent: ElkNode | null,
): { x: number; y: number } {
  const rawX = elkNode.x ?? 0;
  const rawY = elkNode.y ?? 0;
  if (!parent) {
    return { x: rawX, y: rawY };
  }

  const parentX = parent.x ?? 0;
  const parentY = parent.y ?? 0;
  const parentW = parent.width ?? 0;
  const parentH = parent.height ?? 0;

  // ELK 在不同版本/配置下，children 坐标可能是相对父节点，也可能是绝对坐标。
  // 这里优先选择能落入父容器范围的解释，避免出现整体漂移。
  const relativeCandidate = { x: rawX, y: rawY };
  const absoluteCandidate = { x: rawX - parentX, y: rawY - parentY };

  const inParent = (pos: { x: number; y: number }) =>
    pos.x >= -1 && pos.y >= -1 && pos.x <= parentW + 1 && pos.y <= parentH + 1;

  const relativeInParent = inParent(relativeCandidate);
  const absoluteInParent = inParent(absoluteCandidate);

  if (relativeInParent && !absoluteInParent) {
    return relativeCandidate;
  }
  if (!relativeInParent && absoluteInParent) {
    return absoluteCandidate;
  }

  // 双方都成立或都不成立时，优先采用绝对->相对，和现有 edge 路由坐标系更一致。
  return absoluteCandidate;
}

function routeSectionOrderEdge(
  edge: FlowEdge,
  groupNodeMap: Map<string, FlowGroupNode>,
): void {
  const source = groupNodeMap.get(edge.source);
  const target = groupNodeMap.get(edge.target);
  if (!edge.data || !source || !target) {
    return;
  }

  const sourceWidth = source.width ?? SECTION_WIDTH;
  const sourceHeight =
    source.height ?? SECTION_HEADER_HEIGHT + SECTION_PADDING * 2 + NODE_HEIGHT;
  const targetHeight =
    target.height ?? SECTION_HEADER_HEIGHT + SECTION_PADDING * 2 + NODE_HEIGHT;

  const startX = source.position.x + sourceWidth;
  const startY = source.position.y + sourceHeight / 2;
  const endX = target.position.x;
  const endY = target.position.y + targetHeight / 2;
  const dx = Math.max((endX - startX) * 0.35, 20);

  edge.data.path = `M ${startX} ${startY} C ${startX + dx} ${startY} ${endX - dx} ${endY} ${endX} ${endY}`;
  edge.data.labelPosition = {
    x: (startX + endX) / 2,
    y: (startY + endY) / 2,
  };
}

async function layoutBlockDfgWithOrderEdges(
  nodes: FlowNode[],
  dataEdges: FlowEdge[],
  orderEdges: FlowEdge[],
  sections: SectionModel[],
): Promise<FlowGraph> {
  const elemNodes = nodes.filter(
    (node): node is FlowElemNode => node.type === "elemNode",
  );
  const groupNodes = nodes.filter(
    (node): node is FlowGroupNode => node.type === "groupNode",
  );
  // ELK 在层级图中混用“父容器边 + 子结点边”时有概率触发内部异常（minified: reading 'a').
  // 这里仅将真实数据流边交给 ELK，section 顺序边改为布局后手工路由。
  const elkGraph = buildElkGraph(sections, elemNodes, dataEdges);
  const layout = await elk.layout(elkGraph);

  for (const node of groupNodes) {
    const elkNode = findElkNode(layout, node.id);
    if (!elkNode) continue;
    node.position = { x: elkNode.x ?? 0, y: elkNode.y ?? 0 };
    node.width = elkNode.width ?? node.width;
    node.height = elkNode.height ?? node.height;
  }

  for (const node of elemNodes) {
    const elkNode = findElkNode(layout, node.id);
    if (!elkNode) continue;
    const parent = findElkNode(layout, node.parentId ?? "");
    node.position = resolveChildNodePosition(elkNode, parent);
    node.width = elkNode.width ?? node.width;
    node.height = elkNode.height ?? node.height;
  }

  const layoutEdges = new Map(
    (layout.edges ?? []).map((edge) => [edge.id, edge]),
  );
  for (const edge of dataEdges) {
    const layoutEdge = layoutEdges.get(edge.id);
    if (!edge.data || !layoutEdge?.sections) continue;
    const { path, labelPosition } = edgePathFromSections(layoutEdge.sections);
    edge.data.path = path;
    edge.data.labelPosition = labelPosition;
  }

  const groupNodeMap = new Map(groupNodes.map((node) => [node.id, node]));
  for (const edge of orderEdges) {
    routeSectionOrderEdge(edge, groupNodeMap);
  }

  return { nodes, edges: [...dataEdges, ...orderEdges] };
}

export async function getBlockDfg(
  irState: IRState,
  blockID: BlockID,
): Promise<FlowGraph> {
  const blockDfg = irState.getModule().get_block_dfg(blockID);
  const focus = irState.focus;
  const sections = makeSectionModels(blockDfg);
  const groupNodes = makeGroupNodes(sections, focus);
  const elemNodes = makeElemNodes(sections, focus);
  const dataEdges = makeDataEdges(blockDfg.edges, focus);
  const orderEdges = makeSectionOrderEdges(sections);
  return layoutBlockDfgWithOrderEdges(
    [...groupNodes, ...elemNodes],
    dataEdges,
    orderEdges,
    sections,
  );
}
