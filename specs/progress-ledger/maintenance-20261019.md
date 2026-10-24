# 2026-10-19 主线同步与候选说明修正

本轮支撑的决定是：源码说明与版本是否能准确表达当前桌面候选，以及现有轻量概览是否能继续核对。通过标准为源码、构建产物和本地 API 版本一致，文档链接有效，针对性检查通过。不能复现的远端像素失败保留为待验证；不通过删除断言或放宽像素阈值收口。

## 结果与决定

- **采用**：`git fetch origin` 后将本地 `main` 从 `6b68db9` 快进至 `0d32d2b`，与 `origin/main` 对齐。新增内容为 [#146](https://github.com/majiayu000/keepline/pull/146) 的实验 PTY 租约过期与连接隔离修复，未覆盖原有未提交改动。
- **采用**：根包与 Web 客户端版本统一为 `1.2.0`，CLI 与本地 API 从根 `package.json` 读取版本。`apiVersion` 保持协议版本 `1.0`；源码版本不等于正式发布回执。
- **采用**：中英文 README 以本地进度账本、执行证据与用户明确验收为主，删除旧主题宣传、固定模型/缓存倍率和无依据的费用承诺。旧截图标为历史参考，终端工作台保留 Spec / 0/46 的边界。
- **采用**：视觉回归中待核对卡片的操作断言跟随现有轻量概览使用「查看回复」。原稿未修改，侧栏仍要求零像素差异。
- **否定本轮删除**：`endless.mode` 和 LanceDB 仍由 memory 路由调用；Fastify 仍承载 Hook 接收器。不能按无用依赖直接删除。
- **待验证**：远端主线 5 像素失败本机未复现，连续三次视觉回归通过不能证明远端 CI 已恢复，也不替代整页视觉人工验收。

## 本地验证

| 检查 | 结果 |
| --- | --- |
| 根目录与客户端 `bun run typecheck` | 通过 |
| UI 测试类型检查 | 通过 |
| Local API v1 与 architecture-imports | 7 通过，使用测试 preload 隔离数据目录 |
| `bun run build` | Web 客户端及 Bun CLI 构建通过 |
| 源码与构建后 CLI `--version` | 均为 `1.2.0`，使用隔离数据目录 |
| 直接请求 `/meta` 路由 | HTTP 200；serviceVersion 为 `1.2.0`，apiVersion 为 `1.0` |
| 根目录与客户端 frozen lockfile 安装检查 | 通过，无依赖与锁文件变化 |
| 轻量概览与验收针对性 UI | 5 通过，含 1440 / 390 宽度及查看不触发验收 |
| v3 视觉回归连续三次 | 3 通过；侧栏零差异断言保留 |
| 完整浏览器回归 `bun run test:ui` | 80 通过，含 UI 测试类型检查 |
| 中英文 README 本地链接、`git diff --check` | 通过 |

本轮首个视觉回归在侧栏对照后因旧「生成追问」操作断言失败；修正操作断言后连续三次通过。没有将第一次失败记为通过。首次完整浏览器回归为 75 通过 / 5 失败，另发现 4 个旧直接跳转断言和 1 个旧 A 键验收断言；按先查看概览、再明确操作的契约更新后，针对性 5 项通过，完整重跑 80 项全部通过。测试的 WebSocket 代理连接日志来自未启动真实业务服务的 mock 场景，不能用这些浏览器结果证明真实 Hook 或原生通知可用。

## 远端与交付边界

2026-10-19 重新采集 GitHub 状态：

- 最新主线 [run 38370058475](https://github.com/majiayu000/keepline/actions/runs/38370058475) 仍为失败，TypeScript/Bun 与 keeplined 成功，Tauri/UI 的侧栏对照报告 5 像素差异。当前本地修正尚未提交或推送，未产生新的 CI 回执。
- [#145](https://github.com/majiayu000/keepline/pull/145) 仍为 Draft / OPEN，head 为 `ab15e9d0`，CI 3/3 成功。系统通知权限拒绝，真实投递、点击、锚点、撤回与登录周期仍未验收，修复未合入本地主线。
- 公开 Latest 仍为 [v1.1.4](https://github.com/majiayu000/keepline/releases/tag/v1.1.4)。本轮未使用正式签名身份、未公证、未发布、未修改系统通知设置或已安装应用。
- 原有轻量概览与 `specs/terminal-workbench/` 的未提交内容保留。终端工作台实现没有在本轮启动。

剩余发布门槛为真实原生验收、整页视觉取舍、#145 验收合入、最新主线 CI 恢复以及正式签名公证发布。项目地图保留这五项，不以本地构建成功替代。

本地日志：`/tmp/keepline-pixel-check.log`、`/tmp/keepline-ui-maintenance.log`（首轮完整回归失败）、`/tmp/keepline-navigation-maintenance.log`、`/tmp/keepline-ui-maintenance-final.log`（80 项通过）。这些路径是本机回执，未作为公开附件上传。
