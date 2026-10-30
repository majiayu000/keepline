# Keepline 设置改进

采用 Warp 的分类设置、选项说明和即时应用原则，落实到 Keepline 原生工作台。
本轮只对照用户指定的 Warp 设置模块。

[看实际亮色设置](evidence/native/appearance-light.png) · [看实际暗色设置](evidence/native/appearance-dark.png) · [看通用设置](evidence/native/general.png)

- [对照依据与覆盖边界](01-competitor-research.md)
- [实现与验收记录](02-my-plan.md)

## 决定

采用：左侧分类、设置搜索、通用偏好、终端文字预览与持久化、通知入口。
继续使用现有的工作区设置协议和数据库；字体独立保存在用户配置目录。
后续扩展：独立的光标偏好、透明度等需要完整终端行为设计，本轮未实现。

## 覆盖边界

读过 Warp 官方设置、外观和字体文档，检查过本机 Warp 安装包的设置 schema。
未在 Warp GUI 中逐项操作，不评价两个产品谁更好用。
Keepline 基线包括用户已有本地改动，本轮开始时已保存相关文件快照以便核对。
未运行真实 Agent 模型调用、未切换用户的系统通知权限、未替换正在运行的应用。
用户约定不使用子代理，本轮没有第三方代理审查。
