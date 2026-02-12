# Visual Remusys -- Remusys IR 的可视化工具


<p align="right">-- by Medi H.B.T.</p>

---

Visual Remusys 是 Remusys-IR SDK 外围组件的一部分，用于 Remusys-IR 内存结构可视化、优化器可视化、变换规则可视化。同时，它也是我的毕业设计，作为毕设的开发目标是实现最基础的可视化功能, 成为一个简易的教具。

## 项目依赖

- [Remusys-IR](https://github.com/medihbt/remusys-ir) 是一个使用 Rust 编写的类 LLVM 中间代码系统，目标是在 Rust 平台上实现与 LLVM 相近的中间代码结构，并提供一定的教学价值。
- [Remusys-lang](https://gitee.com/medihbt/remusys-lang) 是 Remusys-IR 的 SysY 前端.

## 开发进度

### 依赖层

- [x] IR 文本前端
- [ ] Inst Transformer 开发 (可选)

### 服务层

- [x] IR 传输格式开发
- [ ] IR 快照管理

### 前端 UI

- [ ] 设计 UI 交互逻辑（几个面板给做一下）
