# R2 第一批接手审查

审查原有 screen.rs、Cargo.toml、example 和 terminal-reference 工具：样式按完整 Attrs 切分 run，隐藏字形在绘制前跳过，装饰线共享实际单元格度量；DECSCUSR 取 VT 已解析的光标形状；visual-test 功能独立，不影响正式构建默认 feature。没有发现阻止接手提交的问题。

R1 三轮默认全量测试均覆盖本批新增 3 项 screen 测试，app 15 项全过；2026-10-21 重新运行 run.sh 完成 13 个 fixture，数字见 batch1-report.json/md。SGR 目视确认 faint、隐藏、装饰线生效。**本次 Ghostty 光标焦点检查仍有警告，光标截图不作为焦点下形状对照通过；已有形状单测有效。**

这是接手基线，emoji、鼠标、格子图形、链接、CJK、焦点由后续 R2 修复。旧证据保留，不把旧截图改称新拍。只启动脚本自己的 Ghostty 和离屏渲染进程，未连接用户 Keepline 后台。
