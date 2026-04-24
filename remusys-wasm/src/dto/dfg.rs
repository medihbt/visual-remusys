//! # BlockDFG -- 基本块内数据流图切片
//!
//! BlockDFG 是针对基本块内部的指令和表达式构建的数据流图切片, 以便在前端进行可视化展示和交互.
//!
//! ## 基本结构
//!
//! `BlockDfg` 由两部分组成：
//!
//! - **`sections: Vec<DfgSection>`**：分节后的结点列表。每个 `DfgSection` 包含一个
//!   主导角色 `DfgNodeRole`（`kind`）以及该节内的所有 `DfgNode`。
//!   注意在 b2 规则下，`section.kind` 不保证与节内每个 `node.role` 严格一致：
//!   一些操作数会以 `Income` 角色并入 user 所在节。
//! - **`edges: Vec<DfgEdge>`**：数据流边列表。每条边以 `UseID` 为标识，记录
//!   `from`（使用点 / user）→ `to`（操作数 / 定义点）的数据依赖关系，并携带
//!   `UseKind` 标签。
//!
//! 结点统一由 `DfgNodeID` 标识，可对应 IR 中的指令（`InstID`）、表达式（`ExprID`）、
//! 基本块（`BlockID`）、全局对象（`GlobalID`）、函数参数（`FuncArg`）或临时的
//! 使用边（`UseID`）。
//!
//! 角色 `DfgNodeRole` 分为六类：
//!
//! | 角色 | 含义 |
//! |------|------|
//! | `Income` | 从其他基本块流入的数据（块外定义，块内使用） |
//! | `Outgo` | 向其他基本块流出的数据（块内定义，块外使用） |
//! | `Phi` | PHI 结点 |
//! | `Pure` | 纯计算指令（无副作用，可自由重排） |
//! | `Effect` | 带副作用的指令（如 `store`、`call` 非纯函数） |
//! | `Terminator` | 终结指令（如 `ret`、`br`、`jump`） |
//!
//! ## 为什么要分节
//!
//! 在 CFG 中，基本块之间只有纯粹的跳转关系，没有严格的执行顺序要求。但基本块**内部**的指令
//! 除了数据流关系之外，还有一个很重要的维度：**执行顺序**。
//!
//! 如果块内一段连续指令全部是纯计算（`Pure`），那么它们之间仅有数据流依赖，布局算法可以
//! 自由重排而不影响语义。但一旦遇到 `store`、`call` 等带副作用的指令，顺序就变得至关重要；
//! 若仍用单纯的无序流图表示，排版算法可能把后执行的指令画在前面，严重违背用户直觉。
//!
//! 因此，BlockDFG 引入**分节（Section）**的概念：把块内指令列表按角色切分成若干段。
//!
//! - **同一段（Section）内**的结点之间没有严格的执行顺序要求，可由布局算法根据数据流自由排布。
//! - **段与段之间**的执行顺序是严格的；前端可在相邻段之间插入虚拟控制依赖边，以保留顺序语义。
//!
//! 作为折衷，连续多条副作用指令会被合并到同一个 `Effect` 节中（而不是每条指令单独成段），
//! 并保留类型标签。前端在布局时，若发现节的类型不是 `Pure`，就应尽量保持该节内结点的
//! 相对顺序不变，从而在可视化密度与顺序正确性之间取得平衡。
//!
//! ## WASM 侧做了什么
//!
//! WASM 端的 `BlockDfgBuilder` 负责把单个基本块的 IR 翻译成上述结构。主要工作包括：
//!
//! 1. **指令分类与分节**：遍历块内所有指令，根据指令类型决定其 `DfgNodeRole`。
//!    - 跳过 `GuideNode` 和 `PhiInstEnd` 等辅助指令。
//!    - `Call` 指令通过 `calls_pure` 检查被调用函数的 `attrs().is_func_pure()`，
//!      决定归入 `Pure` 还是 `Effect`。
//!    - 相同角色的连续内部指令会被合并到同一个节中；角色变化时则开启新节。
//!
//! 2. **边构建**：对每条指令的 `operand`（定义→使用）和 `user`（使用→定义）分别调用
//!    `add_edge_with_nodes`，建立双向数据流边。
//!
//! 3. **b2 跨块结点规则**：只有来自块外的 `Inst` 和 `FuncArg` 会被放入独立的
//!    `Income` 节并做全局去重；其他操作数（如常量、表达式、全局变量等）以 `Use`
//!    形式直接放在其使用者所在的节中，且**不去重**。这样可以在不丢失信息的前提下，
//!    减少跨块结点的冗余，同时让块内局部数据流保持完整。
//!
//! 4. **调试输出**：`BlockDfg::to_dot_text` 可直接生成 Graphviz DOT 字符串，
//!    按节绘制子图（`subgraph cluster_*`），并用不同颜色区分各角色，方便后端调试。

use remusys_ir::ir::*;
use serde::{Deserialize, Serialize, Serializer};
use smol_str::{SmolStr, ToSmolStr, format_smolstr};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use wasm_bindgen::JsError;

use crate::{ModuleInfo, dto::ValueDt, fmt_jserr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DfgNodeID {
    Inst(InstID),
    Expr(ExprID),
    Block(BlockID),
    FuncArg(GlobalID, u32),
    Global(GlobalID),
    Use(UseID),
}
impl From<UserID> for DfgNodeID {
    fn from(value: UserID) -> Self {
        match value {
            UserID::Inst(x) => Self::Inst(x),
            UserID::Expr(x) => Self::Expr(x),
            UserID::Global(x) => Self::Global(x),
        }
    }
}

impl ToSmolStr for DfgNodeID {
    fn to_smolstr(&self) -> SmolStr {
        let s = match self {
            DfgNodeID::Inst(inst_id) => inst_id.to_strid(),
            DfgNodeID::Expr(expr_id) => expr_id.to_strid(),
            DfgNodeID::Block(block_id) => block_id.to_strid(),
            DfgNodeID::Global(global_id) => global_id.to_strid(),
            DfgNodeID::FuncArg(func_id, index) => {
                format_smolstr!("FuncArg({}, {index})", func_id.to_strid())
            }
            DfgNodeID::Use(use_id) => use_id.to_strid(),
        };
        format_smolstr!("node:{s}")
    }
}
impl Serialize for DfgNodeID {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_smolstr())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DfgNodeRole {
    /// 从其他基本块输入的数据流结点
    Income,
    /// 向其他基本块输出的数据流结点
    Outgo,
    /// Phi 结点
    Phi,
    /// 纯计算结点（不包含副作用）
    Pure,
    /// 包含副作用的计算结点
    Effect,
    /// 终结指令结点
    Terminator,
}

/// 基本块的数据流图结点
#[derive(Debug, Clone, Serialize)]
pub struct DfgNode {
    pub id: DfgNodeID,
    pub label: SmolStr,
    pub value: ValueDt,
    pub role: DfgNodeRole,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfgSection {
    pub kind: DfgNodeRole,
    pub nodes: Vec<DfgNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfgEdge {
    pub id: UseID,
    pub label: UseKind,
    pub from: DfgNodeID,
    pub to: DfgNodeID,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockDfg {
    pub sections: Vec<DfgSection>,
    pub edges: Vec<DfgEdge>,
}

impl BlockDfg {
    pub fn new(module: &ModuleInfo, block_id: BlockID) -> Result<Self, JsError> {
        BlockDfgBuilder::new(module, block_id).build()
    }

    pub fn to_dot_text(&self) -> String {
        fn dot_escape(text: &str) -> String {
            let mut out = String::with_capacity(text.len());
            for ch in text.chars() {
                match ch {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    _ => out.push(ch),
                }
            }
            out
        }

        fn role_style(role: DfgNodeRole) -> (&'static str, &'static str) {
            match role {
                DfgNodeRole::Income => ("#dbeafe", "#2563eb"),
                DfgNodeRole::Outgo => ("#dcfce7", "#16a34a"),
                DfgNodeRole::Phi => ("#fef3c7", "#d97706"),
                DfgNodeRole::Pure => ("#f3f4f6", "#4b5563"),
                DfgNodeRole::Effect => ("#fee2e2", "#dc2626"),
                DfgNodeRole::Terminator => ("#ffedd5", "#ea580c"),
            }
        }

        let mut output = String::from(
            "digraph BlockDfg {\n  rankdir=LR;\n  compound=true;\n  graph [fontname=\"Helvetica\"];\n  node [shape=box, style=filled, fontname=\"Helvetica\"];\n  edge [fontname=\"Helvetica\"];\n",
        );

        let mut node_names = HashMap::new();
        for (section_idx, section) in self.sections.iter().enumerate() {
            let section_label = dot_escape(&format!("Section {section_idx}: {:?}", section.kind));
            let _ = writeln!(output, "  subgraph cluster_{section_idx} {{");
            let _ = writeln!(output, "    label=\"{section_label}\";");
            let _ = writeln!(output, "    color=\"#94a3b8\";");
            let _ = writeln!(output, "    style=rounded;");

            for (node_idx, node) in section.nodes.iter().enumerate() {
                let node_name = format!("n_{section_idx}_{node_idx}");
                let label = dot_escape(&format!(
                    "{}\\n{:?}\\n{:?}",
                    node.id.to_smolstr(),
                    node.role,
                    node.value
                ));
                let (fillcolor, border_color) = role_style(node.role);
                let _ = writeln!(
                    output,
                    "    {node_name} [label=\"{label}\", fillcolor=\"{fillcolor}\", color=\"{border_color}\"] ;"
                );
                node_names.insert(node.id, node_name);
            }

            let _ = writeln!(output, "  }}");
        }

        for edge in &self.edges {
            let Some(from) = node_names.get(&edge.from) else {
                continue;
            };
            let Some(to) = node_names.get(&edge.to) else {
                continue;
            };
            let edge_label = dot_escape(&edge.id.to_strid());
            let _ = writeln!(output, "  {from} -> {to} [label=\"{edge_label}\"];");
        }

        output.push_str("}\n");
        output
    }
}

#[derive(Debug, Clone, Copy)]
struct InstInfo {
    id: InstID,
    role: DfgNodeRole,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct NodeIndex {
    section: usize,
    unit_idx: usize,
}

struct BlockDfgBuilder<'ir> {
    module: &'ir Module,
    names: &'ir IRNameMap,
    sections: Vec<DfgSection>,
    node_map: HashMap<DfgNodeID, NodeIndex>,
    inst_list: Vec<InstInfo>,
    edges: Vec<DfgEdge>,
    edge_set: HashSet<UseID>,
}

impl<'ir> BlockDfgBuilder<'ir> {
    fn new(ir: &'ir ModuleInfo, block: BlockID) -> Self {
        let allocs = &ir.module().allocs;
        let mut inst_list = Vec::with_capacity(block.get_insts(allocs).len());
        for (id, inst) in block.insts_iter(allocs) {
            let Some(role) = Self::inst_role(ir.module(), inst) else {
                continue;
            };
            inst_list.push(InstInfo { id, role });
        }
        Self {
            module: ir.module(),
            names: ir.names(),
            sections: vec![
                DfgSection {
                    kind: DfgNodeRole::Income,
                    nodes: Vec::new(),
                },
                DfgSection {
                    kind: DfgNodeRole::Outgo,
                    nodes: Vec::new(),
                },
            ],
            node_map: HashMap::new(),
            inst_list,
            edges: Vec::new(),
            edge_set: HashSet::new(),
        }
    }

    fn inst_role(module: &Module, inst: &InstObj) -> Option<DfgNodeRole> {
        use InstObj::*;

        let role = match inst {
            GuideNode(_) | PhiInstEnd(_) => return None,
            Phi(_) => DfgNodeRole::Phi,
            Unreachable(_) | Ret(_) | Jump(_) | Br(_) | Switch(_) => DfgNodeRole::Terminator,
            Store(_) | AmoRmw(_) => DfgNodeRole::Effect,
            Call(call) => {
                if Self::calls_pure(module, call) {
                    DfgNodeRole::Pure
                } else {
                    DfgNodeRole::Effect
                }
            }
            _ => DfgNodeRole::Pure,
        };
        Some(role)
    }

    fn calls_pure(module: &Module, call: &inst::CallInst) -> bool {
        let ValueSSA::Global(global) = call.get_callee(module) else {
            return false;
        };
        let Some(func) = FuncID::try_from_global(module, global) else {
            return false;
        };
        func.deref_ir(module).attrs().is_func_pure()
    }

    fn add_dedup_node(
        &mut self,
        id: DfgNodeID,
        value: ValueSSA,
        role: DfgNodeRole,
        section_id: usize,
    ) -> Result<usize, JsError> {
        if let Some(existing) = self.node_map.get(&id) {
            return Ok(existing.section);
        }
        let Some(section) = self.sections.get_mut(section_id) else {
            return fmt_jserr!(Err
                "section id overflow: {section_id} >= {}",
                self.sections.len()
            );
        };
        let node_idx = section.nodes.len();
        let value = ValueDt::from(value);
        section.nodes.push(DfgNode {
            id,
            value,
            label: value.get_name(self.module, self.names)?,
            role,
        });
        self.node_map.insert(
            id,
            NodeIndex {
                section: section_id,
                unit_idx: node_idx,
            },
        );
        Ok(section_id)
    }

    fn add_nodedup_node(
        &mut self,
        id: DfgNodeID,
        value: ValueSSA,
        role: DfgNodeRole,
        section_id: usize,
    ) -> Result<usize, JsError> {
        let Some(section) = self.sections.get_mut(section_id) else {
            return fmt_jserr!(Err
                "section id overflow: {section_id} >= {}",
                self.sections.len()
            );
        };
        let value = ValueDt::from(value);
        section.nodes.push(DfgNode {
            id,
            label: value.get_name(self.module, self.names)?,
            value,
            role,
        });
        Ok(section_id)
    }

    fn push_internal_inst(&mut self, InstInfo { id, role }: InstInfo) -> Result<(), JsError> {
        let reuse_last = match self.sections.last() {
            Some(last) => last.kind == role,
            None => false,
        };

        let section_id = if reuse_last {
            self.sections.len() - 1
        } else {
            self.sections.push(DfgSection {
                kind: role,
                nodes: Vec::with_capacity(1),
            });
            self.sections.len() - 1
        };

        let section = &mut self.sections[section_id];
        let node_idx = section.nodes.len();
        section.nodes.push(DfgNode {
            id: DfgNodeID::Inst(id),
            value: ValueDt::Inst(id),
            label: ValueDt::Inst(id).get_name(self.module, self.names)?,
            role,
        });
        self.node_map.insert(
            DfgNodeID::Inst(id),
            NodeIndex {
                section: section_id,
                unit_idx: node_idx,
            },
        );
        Ok(())
    }

    fn add_edge_with_nodes(&mut self, edge: UseID) -> Result<(), JsError> {
        if self.edge_set.contains(&edge) {
            return Ok(());
        }
        let allocs = &self.module.allocs;
        let useobj = edge.deref_ir(allocs);
        let Some(user) = useobj.user.get() else {
            return fmt_jserr!(Err "internal error: dangling edge {edge:?} has no user");
        };
        let operand = useobj.operand.get();

        let user_id = DfgNodeID::from(user);
        let user_section_id = self.add_dedup_node(
            user_id,
            user.into_ir(),
            DfgNodeRole::Outgo,
            1, // Outgo section
        )?;

        // b2 规则: 只有 Inst / FuncArg 放在 Income section 并去重;
        // 其他操作数都放在 user 所在 section，作为 Income 角色且不去重。
        let (operand_id, _) = match operand {
            ValueSSA::Inst(inst_id) => (
                DfgNodeID::Inst(inst_id),
                self.add_dedup_node(
                    DfgNodeID::Inst(inst_id),
                    operand,
                    DfgNodeRole::Income,
                    0, // Income section
                )?,
            ),
            ValueSSA::FuncArg(func_id, idx) => (
                DfgNodeID::FuncArg(func_id.raw_into(), idx),
                self.add_dedup_node(
                    DfgNodeID::FuncArg(func_id.raw_into(), idx),
                    operand,
                    DfgNodeRole::Income,
                    0, // Income section
                )?,
            ),
            _ => (
                DfgNodeID::Use(edge),
                self.add_nodedup_node(
                    DfgNodeID::Use(edge),
                    operand,
                    DfgNodeRole::Income,
                    user_section_id,
                )?,
            ),
        };

        self.edges.push(DfgEdge {
            id: edge,
            label: edge.get_kind(self.module),
            from: user_id,
            to: operand_id,
        });
        self.edge_set.insert(edge);
        Ok(())
    }

    fn build(mut self) -> Result<BlockDfg, JsError> {
        let inst_list = std::mem::take(&mut self.inst_list);
        for &info in &inst_list {
            self.push_internal_inst(info)?;
        }

        let allocs = &self.module.allocs;
        for &InstInfo { id, .. } in &inst_list {
            for edge in id.get_operands(allocs) {
                self.add_edge_with_nodes(edge)?;
            }
            for (edge, _) in id.deref_ir(allocs).user_iter(allocs) {
                self.add_edge_with_nodes(edge)?;
            }
        }

        Ok(BlockDfg {
            sections: self.sections,
            edges: self.edges,
        })
    }
}
