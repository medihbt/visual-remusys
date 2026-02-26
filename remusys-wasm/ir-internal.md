# 已经对接到 JS 的数据结构

## 所有的 **ID 类型

Remusys-IR 已经给这些 ID 实现了 `serde::Serialize` 和 `serde::Deserialize`，JS 前端准确地反映了这些类型序列化出来的字符串要遵循什么约束.

包括 `GlobalID` `InstID` `BlockID` `ExprID` `UseID` `JumpTargetID` 这 6 个 ID 类型.

## Opcode 枚举

Opcode 对接到 serde, 且在 JS 处有对应的类型定义

## **Kind 枚举

包括 UseKind 和 JumpTargetKind, 也对接到 serde, 且在 JS 处有对应的类型定义

## ValTypeID 枚举

对接到了 serde, 且在 JS 处有对应的类型定义. ValTypeID 的大量变体在序列化时都不表达语义, 只表达内存池索引, 所以需要一个额外的 API 来获取 ValTypeID 的字符串名称.