# Visual Remusys -- Remusys IR 的可视化工具


<p align="right">-- by Medi H.B.T.</p>

---

Visual Remusys 是 Remusys-IR SDK 外围组件的一部分，用于 Remusys-IR 内存结构可视化、优化器可视化、变换规则可视化。同时，它也是我的毕业设计，作为毕设的开发目标是实现最基础的可视化功能, 成为一个简易的教具。

## 依赖搭建

Visual Remusys 使用 NodeJS + Vite + WASM 系列套件, 因此在运行前需要按顺序安装好下面的依赖:

- Node.js: https://nodejs.org
- Rust 基础开发套件: https://rust-lang.org. Visual Remusys 默认使用最新的 stable rust 环境.
- wasm-pack: 安装好 Rust 后运行命令 `cargo install wasm-pack`
- wasm-bindgen: wasm-pack 在运行时会自己安装好 wasm-bindgen 的 CLI 版本, 预装 wasm-bindgen 没用。这个安装过程会占据很长时间并且没有任何 debug 输出, 该问题尚未解决。

## 项目环境搭建

进入项目后要初始化两个外链 git 仓库. 运行:

```bash
git submodule update --init --recursive
cd remusys-ir && git checkout refactor/unified-indexed && git pull -r
cd ../remusys-lang && git checkout mode-workspace && git pull -r
```

## 快速运行

在项目目录下执行下面的命令启动项目:

```bash
npm install
npm run wasm-build && npm run wasm-refresh
npm run dev
```

此时 vite 会启动并指引你打开一个 localhost 网址. 使用浏览器打开网址即可看到效果.
