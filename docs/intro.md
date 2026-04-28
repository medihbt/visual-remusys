# the Visual Remusys -- Remusys-IR 可视化分析工具

Visual Remusys 是一个为解决类 LLVM IR 晦涩、不直观而设计的实时交互可视化查看器，目标使用场景是教学和小规模 IR 分析。同时，Visual Remusys 也是我的毕设。

## 界面简介

通过 `build.sh && cd remusys-lens && npm run dev` 运行项目, 访问 Vite 给出的 URL, 加载 SysY 或者 Remusys-IR 文本文件后, 你会见到这样的界面:

![Visual Remusys Web UI](1-1-1-visual-remusys-webui.png)

左上角是基于 Monaco Editor 的 IR 文本视图 SourceView，展示的是经过解析后再序列化出来的 IR 文本。除了文本以外，SourceView 会高亮当前关注的 IR 对象及其引用，方便开发者理清“我在哪儿，关注着什么”。由于 Remusys-IR 没有实现增量语法分析, SourceView 是只读的, 如果你要修改 IR 文本的话通常会被拒绝。但重命名操作不会破坏 IR 结构, 所以 SourceView 支持对选中的点或者区域重命名，重命名成功后会刷新所有 IR 组件。

左下角是基于 React Flow 编写的 IR 树-导航视图 GuideView. GuideView 是 Visual Remusys 的导航中心, 不仅表示中间代码的树结构, 而且还可以

## 前置项目: Remusys-IR
