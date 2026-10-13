# 2026-10-13 独立候选修复与验证

状态：软件回归、原生构建及进程重连验证通过；原生窗口交互仍待人工验收。本文不是发布回执。

## 候选范围

- 工作树：`/Users/lifcc/Desktop/code/AI/tools/keepline-candidate-20261013`
- 分支：`fix/desktop-candidate-20261013`；基线 main `d23d6a6`。
- 合并本地 `f7ea194` 的已提交增量及本地未提交改动；保留主线的命令证据新鲜度、验收清单身份、设置保留期和独立待办契约。
- 原工作区未被覆盖；外部恢复包：`/tmp/keepline-candidate-20261013-backup/`。
- 当前修复未推送、未合并远端 PR、未发布，也未替换已安装应用。

## 本次修复

1. 礼貌问句形式的明确请求保留为候选要求；事实性提问仍不进入要求清单，不把候选视为验收或完成。
2. 进程扫描缺失保留 unknown；PID 已明确属于另一会话时解除旧绑定。保留更新的 Hook 观察，输入请求计入目标待处理数。
3. 固定菜单栏失败重试测试的后台刷新时序；覆盖请求乱序、保留旧记录和原生计数一致性。
4. 设置增加可点击的 Hook 连接入口，保留已有命令，读回安装结果；分别显示配置、接收器与真实事件状态。没有实际改写用户的 Codex/Claude Hook 配置。
5. 菜单栏使用现有扫描摘要，缓存有效时不读完整详情。新接口保持状态与分组语义，验收清单编辑后失效刷新。
6. 隐藏页面不再补发悬而未决的详情请求；隐藏空的 unknown 分组；固定顶栏高度，避免无搜索框页面的 1 px 跳动。
7. README 与桌面说明改为进度账候选定位，明确源码、旧安装包、本地构建与正式发布的区别。
8. 原生实测发现窗口重建竞态：旧窗口异步销毁尚未完成，按同名重建失败。改为保留窗口并重载，在页面加载完成时注入当前凭证；保留可见性和深链。

## 软件检查

| 检查 | 结果 |
| --- | --- |
| 根目录及前端 TypeScript | 通过 |
| Bun 全量 | 1198 通过，16 跳过，0 失败；94 文件、5099 断言 |
| Playwright Chromium 全流程 | 76 通过，含 v3 固定组件像素对照 |
| 菜单栏 WebKit，重复 3 次 | 27 通过 |
| 菜单栏重试 Chromium，重复 10 次 | 10 通过 |
| Tauri Rust（重连修复后） | 29 通过 |
| 公证脚本失败拦截 | 8 通过 |
| 本地 macOS .app 构建与签名完整性 | 通过；ad-hoc，未经正式公证 |

16 个跳过项依赖 GNU sed 等当前 macOS 环境条件；不能计入通过数。浏览器测试使用模拟 API/IPC，不能单独证明原生通知、登录项或发布成功。

## 真实性能测量

读取本机真实 Codex/Claude 日志，独立 Keepline 数据目录，保留 7 天，模型关闭，5 秒扫描周期；24 小时总览有 73 个会话。

- 完整账本：6,359,442 字节；5 次请求中位数 68.85 ms。
- 菜单栏摘要：96,960 字节；5 次请求中位数 11.89 ms；响应体减少 98.48%。
- 首次扫描：28.14 秒。
- 连续 300 秒、60 次采样：进程树采样 CPU 均值 2.17%，RSS 中位数 312.06 MiB，峰值 396.61 MiB，无扫描错误。

这是短时实测，不是 24 小时稳定性或内存泄漏证明；CPU 为每 5 秒的 ps 采样，可能漏掉短时扫描子进程，不等于完整 CPU 时间积分。测量期间本机还有其他开发验证任务。
原始聚合数据见 [candidate-performance-20261013.json](candidate-performance-20261013.json)，不包含用户会话正文。

## 原生边界

- 实际运行原生应用与内嵌服务：终止服务后 8.13 秒恢复，主应用进程未退出，原生通知通道恢复心跳。
- 在隔离数据库撤销凭证后：4.02 秒重新认证，通道恢复；日志未再出现同名窗口重建错误。
- 复验包使用独立应用标识 `com.starlight.keepline.candidate20261013`；业务代码相同，避免与机器上的其他 Keepline 构建混淆。默认标识的正式候选包另行保留。
- Computer Use 多次返回 `cgWindowNotFound`，未能读取原生窗口。因此上述进程/API 证明不能代替菜单、深链、退出按钮等原生交互验收。
- 默认应用标识启动时，系统返回通知未获授权；系统横幅与点击尚未通过。没有为此修改系统通知权限。
- 未执行登录/注销周期，也未做 24 小时驻留测试。
- 测试使用 `/tmp/keepline-candidate-runtime-20261013` 数据目录，未改用户 Hook、API key 或 `.env`。测试进程已停止，首次启动自动产生的候选登录项已移到 `/tmp/keepline-candidate-test-autostart.plist`，不留在登录项目录。
- 正式签名、公证、推送合并和公开发布需要明确授权。

进程重连回执见 [candidate-native-recovery-20261013.json](candidate-native-recovery-20261013.json)。

本地包：`menubar-tauri/src-tauri/target/release/bundle/macos/Keepline.app`（1.2.0，ad-hoc）。

## 证据位置

- `/tmp/keepline-candidate-all-tests-final.log`
- `/tmp/keepline-candidate-ui-final.log`
- `/tmp/keepline-candidate-webkit-final.log`
- `/tmp/keepline-candidate-retry-final.log`
- `/tmp/keepline-candidate-rust-tests-final.log`
- `/tmp/keepline-candidate-notarize-tests.log`
- `/tmp/keepline-candidate-native-build-final.log`
- `/tmp/keepline-candidate-native-runtime-fixed.log`

原生重连实现参考 Tauri 的 [WebviewWindow API](https://docs.rs/tauri/2.10.3/tauri/webview/struct.WebviewWindow.html)，实际验收以本仓库构建与运行证据为准。

## 构建指纹

- `claude-quota-monitor` SHA-256：`b0c3494316b7a308af35732cac96476597d3dab95bd475f5146becaac6f15819`
- `keepline-service` SHA-256：`c22d57b34ed887859c757cdffbcc344e1de99407445083255685e7331ded1a21`
