# 待办任务

## 高优先级 - 功能增加
- [x] 添加文件/文件夹收藏功能（标记重要位置，快速访问）✅ favorites.rs + 欢迎页收藏夹
- [x] 添加批量删除功能（多选文件后一键删除）✅ Shift/Cmd+click多选、Cmd+A全选、批量删除按钮
- [x] 添加文件类型占比饼图/环形图 ✅ FileTypeChart.tsx
- [x] 添加"大文件排行榜"面板 ✅ LargeFilesPanel.tsx
- [x] 添加重复文件检测功能（基于文件哈希识别重复）✅ DuplicatesPanel.tsx + duplicates.rs
- [x] 添加文件夹对比功能（比较两个目录的差异）✅ compare.rs + ComparePanel.tsx，支持扫描/对比/结果展示
- [x] 添加扫描历史记录面板 ✅ 欢迎页显示历史
- [x] 添加导出报告功能 ✅ JSON/CSV 已实现

## 高优先级 - 交互优化
- [x] 添加键盘快捷键支持
  - [x] Cmd+O 打开文件夹
  - [x] Cmd+F 聚焦搜索框
  - [x] Esc 关闭弹窗/取消选择
  - [x] Backspace/Delete 返回上级目录
  - [x] Space 预览选中文件 ✅ Quick Look 预览
  - [x] Cmd+Delete 删除选中文件
  - [x] 方向键导航 Treemap 单元格
  - [x] Enter 进入选中目录
- [x] 添加拖放支持（拖入文件夹直接扫描）
- [x] 添加 Treemap 缩放手势（Cmd+滚轮 放大/缩小）
- [x] 添加 Treemap 平移手势（Alt+拖拽 或 中键拖拽移动视图）
- [x] 添加面包屑导航优化（可折叠长路径，悬停显示完整路径）✅ 折叠中间路径、悬停下拉菜单
- [x] 添加右键菜单增强 ✅ 复制路径、在终端打开已实现
- [x] 添加文件预览功能（图片缩略图、文本预览、视频封面）✅ FilePreviewPanel.tsx + get_file_preview Rust 命令，支持图片/文本/视频缩略图预览
- [x] 添加搜索结果高亮和跳转功能 ✅ 搜索结果计数、跳转按钮、高亮动画

## 中优先级 - 视觉优化
- [x] 优化加载动画 ✅ 显示文件数/目录数/大小/当前路径
- [x] 添加更多主题（Dracula, Nord, Solarized, GitHub Light/Dark）✅ 已添加 4 个新主题
- [x] 优化 Treemap 颜色方案（按文件类型/大小渐变着色）✅ HSL 颜色系统，大文件更亮更饱和，深度层级渐暗
- [x] 添加 Treemap 动画（展开/折叠/缩放时平滑过渡）✅ 导航动画：进入目录放大淡入、返回目录缩小淡入、单元格交错动画
- [x] 添加选中文件高亮边框和光晕效果 ✅ 已实现渐变边框动画、内外发光、gradient-rotate动画
- [x] 优化小文件块显示（聚合显示，避免过于密集）✅ 小文件聚合成 "+N" 块，点击显示详情弹窗
- [x] 添加深色/浅色模式自动切换（跟随系统）✅ ThemeSwitcher 新增 Auto 选项，监听系统 prefers-color-scheme
- [x] 响应式布局优化（窗口过小时的适配）✅ 4个断点：1600px+/1024px/768px/480px，工具栏换行、小屏隐藏次要元素、平滑过渡

## 中优先级 - 用户体验
- [x] 添加键盘快捷键帮助面板 ✅ KeyboardShortcutsPanel.tsx 组件，按 ? 或 Cmd+/ 显示，分组展示所有快捷键
- [x] 添加首次使用引导（功能介绍 tooltip）✅ OnboardingGuide.tsx 组件，7 步交互式引导，设置面板添加"Show Guide"按钮可重新显示
- [x] 添加设置面板（自定义扫描深度、忽略规则、默认主题）✅ SettingsPanel.tsx + settings.rs，三个标签页（General/Scanning/Appearance），支持忽略模式管理、主题预览选择
- [x] 添加"最近扫描"快捷入口 ✅ 欢迎页显示历史
- [x] 添加扫描进度取消功能 ✅ 已实现
- [x] 添加文件大小单位切换（KB/MB/GB 或 KiB/MiB/GiB）✅ 在设置面板中可选择 SI/Binary 单位格式
- [x] 添加排序选项（按大小、名称、修改时间）✅ 工具栏排序菜单，支持按大小/名称/日期升降序
- [x] 添加文件信息详情弹窗（完整路径、权限、创建时间等）✅ Get Info 右键菜单 + FileInfo 模态框

## 低优先级 - 性能优化
- [x] 实现 Treemap 虚拟化（大量节点时只渲染可见区域）✅ useVirtualizedRects hook，基于视口+zoom+pan计算可见区域，>100个单元格时启用，150px overscan预渲染
- [x] 优化搜索性能（添加防抖，大数据集时使用 Web Worker）✅ 使用 debouncedSearchText 防抖过滤，150ms 延迟
- [x] 优化重新扫描时的 UI 更新（避免全量重渲染）✅ 删除文件后保持当前导航位置，自动回退到最近的父目录
- [x] 检查并优化内存使用（大目录扫描时）✅ TempNode 使用 Box<str> 节省 8 字节/节点、文件节点不分配 children_paths Vec（节省 24 字节/文件）、Phase 5 显式释放 DashMap/DashSet、显示 peak/final 内存统计

## 低优先级 - 集成完善
- [x] 集成 SettingsPanel、OnboardingGuide、KeyboardShortcutsPanel 组件到 App.tsx ✅ 添加快捷键、工具栏按钮、面板渲染
- [x] 实际应用 appSettings 配置值 ✅ SettingsContext 全局共享设置，size_unit 应用于 formatSize、default_theme 启动时应用、enable_cache 控制扫描缓存、auto_expand/large_files_count 应用于 LargeFilesPanel、duplicate_min_size 应用于 DuplicatesPanel
- [x] 添加设置导入/导出功能（备份和迁移设置）✅ export_settings/import_settings Rust 命令 + SettingsPanel UI 按钮，支持 JSON 文件导入导出
- [x] 扫描器应用设置中的忽略模式 ✅ scanner.rs 添加 should_skip_path 函数，支持精确匹配和通配符模式，应用 show_hidden_files 和 max_scan_depth 设置

## 低优先级 - 代码质量
- [x] 将 App.tsx 拆分为多个组件 ✅ 从 1714 行减少到 1137 行（减少 577 行，34%），拆分为 9 个组件：TreemapCells, Toolbar, WelcomeScreen, ScanningProgress, ContextMenu, StatusBar, Breadcrumb, DiskOverview
- [x] 统一错误处理，添加用户友好的错误提示 ✅ 创建 ErrorNotificationContext，全部 20 处 try-catch 使用统一的错误通知系统
- [x] 完善 TypeScript 类型，消除 any 类型 ✅ 28 个完整类型定义，零 any 类型，类型安全评分 A+
- [x] 添加单元测试（至少覆盖核心算法）✅ Vitest 框架，59 个测试用例：treemap.test.ts（18个）+ types.test.ts（41个），覆盖布局算法、排序、聚合、工具函数

## 未来优化方向

### 新功能
- [ ] 添加 Undo/Redo 功能（撤销文件删除，支持从回收站恢复）
- [x] 添加高级搜索功能（正则表达式搜索）✅ 搜索框添加正则模式切换按钮(.*)，支持正则匹配和高亮
- [ ] 添加文件内容搜索功能（搜索文件内部内容）
- [x] 添加文件压缩建议（检测可压缩文件，如 node_modules、日志文件）✅ cleanable.rs + CleanableFilesPanel.tsx，8种类别30+模式检测，批量选择删除
- [x] 添加扫描配置文件支持（.spaceignore 类似 .gitignore）✅ scanner.rs 支持解析 .spaceignore 文件，自动合并到忽略模式列表

### 代码质量
- [x] 修复 Rust Clippy 警告 ✅ 修复 lib.rs/settings.rs/cache.rs/compare.rs/scanner.rs/duplicates.rs/bench.rs 中的所有警告

### 测试与质量
- [ ] 添加 E2E 测试（Playwright 自动化测试）
- [ ] 添加性能基准测试（扫描速度、内存使用）
- [x] 添加 Accessibility 审计 ✅ 模态框 role="dialog" + aria-modal、焦点捕获、标签页 ARIA 语义、右键菜单 role="menu"、跳转链接

### 用户体验
- [ ] 添加国际化支持 (i18n)（支持中文、英文等多语言）
- [ ] 添加自定义右键菜单功能（用户可配置菜单项）
- [x] 添加扫描结果比对功能（对比两次扫描结果的变化）✅ snapshot.rs + ScanComparePanel.tsx + 集成到 App.tsx，支持保存/删除快照、选择两个快照对比、显示新增/删除/变化的文件

---
*优化方向：*
- 功能增加 > 交互优化 > 视觉优化 > 代码质量
- 每次改动后确保 `pnpm build` 通过
- 保持现有功能正常工作
- 新功能要符合 macOS 原生应用交互习惯
