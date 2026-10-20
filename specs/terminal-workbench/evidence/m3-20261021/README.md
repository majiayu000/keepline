# M3 鸟瞰、导航、搜索与继续

2026-10-21，采用：统一 live identity 路由、持久化离开时间、缓存内搜索和展示空间归属。无模型调用、未改用户后台。

| 项 | 结论/验证方式 | 证据 |
|---|---|---|
| 离开 30 分钟后先鸟瞰 | 通过，持久化一小时前离开时间后启动真实 GPUI | return-overview.png；边界单测、SQLite 重开测试 |
| 新/只看有变化 | 通过，2 个更新/1 个旧会话，筛选后旧卡不计入 | changed-filter.png；日志时间优先于索引 mtime 单测 |
| 空间筛选 | 通过，真实按钮；最长路径边界和明确归属单测 | changed-filter.png |
| 原话搜索及跳转 | 通过，Computer Use 粘贴“第 2 个”，命中唯一会话，开正确卡片 | search-original.png |
| 外部继续/复制 | 通过，真实按钮，原 session ID/cwd，预填不执行 | resume-prefill.png；workspace.json、invocation-count.json |
| 退出托管不能 attach | 通过，current+Running 路由单测；历史/Interrupted 必须 resume | client-tests.log |
| 左栏和底栏导航、标签角标 | 实现，工作视图截图；有等待会话的角标/菜单入口在 M4 连同通知验证 | resume-prefill.png |

16 core、22 daemon、14 client 通过；最终导航修复后再次跑客户端回归。原生 fixture 的 HOME、日志、后台均隔离，cwd 指向真实工作树仅用于目录展示；没有执行命令。由 SQL 设置离开时间是确定时间夹具，不冒充等待真实 30 分钟。短暂离开保留本轮变化基线。搜索只覆盖已索引内容，未读历史不猜结果。
