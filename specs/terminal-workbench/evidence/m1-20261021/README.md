# M1 待办完整流程

采用：任务编辑、结局和一次性启动归属意图落到对象表；重复开始优先复用属于该待办的活跃会话，其次复用尚未执行的 shell。再开会话使用待办目录和既有标签分屏。任务完成不停止会话，也不替会话设置结局。

| 项目 | 结果与证据 |
|---|---|
| 表单：标题/项目/说明/逐条验收/Agent，保存与保存并开始 | 通过，Computer Use 填写并保存；SQLite 两条独立标准、原始说明；[预填图](prefill.png) |
| 预填 bracketed paste，无回车 | 通过，原生 shell 显示原样命令；回车前模拟执行记录为 0 |
| 重复开始 | 通过，原生再次点开始，终端总数保持 2；core 另测关闭标签仍能找到原终端 |
| 切换 Agent | 通过，原生切到 Claude，再切回 Codex；[切换图](agent-switch.png)；取消旧 shell 缓冲后等待输出再预填 |
| 回车后身份关联 | 通过，原生回车启动自编译模拟 CLI，经进程和打开的 JSONL 自动挂到待办；[读回](actions.json)、[关联图](associated.png) |
| 说明与验收原样传递 | 通过，模拟 CLI 记录参数：引号、$HOME 与换行未展开，未启动模型 |
| 清空命令/不关联 | 通过，原生点击清空后无额外执行，pending_tasks 的 NULL 覆盖落库；core 验证首次关联不挂待办，消费后不影响以后会话；[清空图](cleared.png) |
| 详情与关联列表/再开一个/完成/暂存/放弃/编辑 | 详情和再开、完成经原生；编辑、暂存必填与状态持久化经单测。每条标准显示有/无来源引用，[详情图](detail.png)；不把 Agent 自述当证据 |
| 结局不结束进程 | 通过，完成任务后已关联会话 current=1、所有 shell 仍 Running，读回见 actions |
| 归属与摆放、动态标签名、存为待办 | core 跨空间/重绑定/动态命名与已有集成 SaveSessionTask 回归通过；每条标准 ID 与证据引用编辑时保留 |

[unit.log](unit.log)：core 15 + daemon 21。较早的完整 [integration.log](integration.log)：15 个集成通过、1 ignored 为 fixture 入口；40 空闲与 4 活跃写入资源回归均通过。[client.log](client.log)：13 项通过（共享树并行 screen 测试包含在构建，源码未纳入提交）。后续 M6 补整套原生回归，不把模拟 CLI 证明说成真实 Claude/Codex 会话验收。

测试环境曾因 macOS /etc/zprofile 的 path_helper 覆盖 PATH 而打开安装版 Codex 的未登录页。立即 Ctrl-C 退出，隔离 HOME 无凭证，无登录和模型调用。改为仅在测试 HOME 的 .zprofile 设置模拟路径后重新开画面，模拟程序接收与身份链路通过。未改用户配置或后台。

`native.py` 可复现隔离宿主及无模型 CLI；截图是 Computer Use 操作真实 GPUI 窗口，不是 HTML。实验结束只停止本 fixture 的 PTY、客户端和宿主。源程序构建成功；近期最后微调为“重复开始优先选已关联会话”和从外部关联项打开鸟瞰 Here，分别由 core 测试与后续 M6 覆盖，截图拍摄时尚未包含这两处选择顺序修正。
