# R14 七页视觉复核 · 2026-10-21

已完成七页修复前/后原生与原稿对照；已知的左栏、搜索、标签、状态栏与卡片锚定问题已修。**不将此记录等同于用户人工接受**，tasks.md 的人工接受项仍未勾选。

打开 [index.html](index.html) 可逐组翻看，并切换修复前/后。每组也有独立 `before/*-side-by-side.png` 和 `after/*-side-by-side.png`。

## 方法与隔离

- 六个主界面来自实际 GPUI `Window::render_to_image`，Retina PNG 2880×1800，对应 **1440×900** 逻辑视口；导出时逐页断言尺寸，见 `before/native-render.log`、`after/native-render.log`。不是用 HTML 重画原生界面。
- Menubar 使用实际 AppKit `NSPopover` 的 view 原生绘制，720×960 PNG 对应设计的 **360×480**。并排工具把它放入 1440×900 画布，不拉伸成全屏。
- 原稿直接读取 `design/*.dc.html`；缺少的 support.js 由对照脚本仅展开原稿模板/原稿数据，原设计文件未修改。网络字体请求禁用，使用本机字体回退。
- `fixture.py` 创建独立 HOME/runtime、5 个空间、40 个合成会话、3 条待办及逐条验收证据，含等待、错误、待核对、暂停、运行、很久没动、暂存与完成。通过真实日志解析、数据库、PTY 和组件渲染；不把假数据放入产品代码，不读用户日志、凭据或配置，不调用模型。
- 交互验证和最终图片使用同结构的独立 fixture；最终重新创建 fixture，避免多次截图预填叠加或交互保存改变示例。运行分组在截图前仅向隔离日志追加新的 task_started，防止复核耗时让它自然转为暂停。
- 此处原生渲染截图证明样式；真实点击/粘贴验证另列，不能混作输入法、真实 CLI 或用户人工视觉验收。

## 七页差异与处理

| 页面 | 修复前问题 | 本轮处理 | 保留差异 / 边界 |
|---|---|---|---|
| [Main](after/main-side-by-side.png) | 空间是纯文字计数；待办占两行大按钮；需要你缺空间；搜索为按钮；状态栏为 Shell/已连接 | 空间分组点与双色角标；待办副标题和同行开始；需要你色点+空间；实际输入框+设置齿轮；待办勾选图标与分组点；状态栏顺序为标签→摘要→Agent·时间→你在这里→分屏→菜单→关闭 | Herdr 三叶嵌套树，无空位、无数量上限；只有一画面不画分屏边框。空 shell 无 AgentSession，不伪造分组/摘要。字体行高沿用已验收的 Ghostty 1.2，而非旧稿 1.6 |
| [Overview](after/overview-side-by-side.png) | 顶栏搜索为按钮；长 cwd 挤占卡片；五列因小数舍入变四列 | 输入框与齿轮；卡片元信息显示所属空间；列宽向下取整保证五列 | 40 会话比原稿示例长，需滚动；保留现有每卡一个主按钮与动作菜单。不伪造离开时长、“新”标记或验收结果 |
| [TodoNew](after/todonew-side-by-side.png) | 无标题，640 宽；五个项目全宽竖排；默认 Agent 只有轮换按钮 | 520 宽卡片；标题；项目紧凑选择；两个明确 Agent 选项；保存并开始强调；说明预填不执行 | 当前项目用可换行选项而非原稿下拉；验收标准仍按每行一条输入，未增加独立拖拽/新增行控件；卡片置于工作台内容区，不伪造系统模态窗 |
| [TodoDetail](after/tododetail-side-by-side.png) | 铺满内容区；验收标准为大块紫底；关联会话为居中按钮 | 480 宽卡片；真实项目/路径；逐条证据勾选及分隔；关联会话的分组点、状态与打开入口；再开会话主色 | 真实对象无创建时间，不显示原稿假日期；现有撤销结局、返回画面仍保留，操作区较原稿多一行 |
| [TodoStart](after/todostart-side-by-side.png) | 左栏及状态栏仍为旧骨架 | 共用新左栏、标签与状态栏；说明和三个预填操作保留 | 真实 zsh bracketed paste 的反色是 shell 行为；不自动回车、不执行模型；无旧空位 |
| [Cleanup](after/cleanup-side-by-side.png) | 标题/返回分散；四字段双层大留白；操作顺序不同 | 同行标题/返回/进度；会话信息进入浅色卡；定义列表；推断浅底；继续→完成→暂存→放弃 | 暂存沿用现有必填原因/小时数表单，不加入原稿“明天上午/下周一”新控件；不伪造已处理数 |
| [Menubar](after/menubar-side-by-side.png) | 黑字大按钮；无色点、时间、主次关系；打开工作台灰按钮 | AppKit 分组色点、浅色选中底、摘要色、时间、标题/计数层次、全宽靛蓝按钮 | 显示最多五条真实需要你事项；空间不足的其余数量有提示。未实现的“专注 30 分钟”不做假按钮；不自动添加通知权限 |

时间采用内容层最后活动时间，不拿首次索引的身份更新时间冒充最近活动。按钮悬停只改变描边，修复浅色页面主按钮被通用深色 hover 覆盖的问题。表单焦点有强调色描边，搜索占位符使用次要色。所有新增样式值收敛到 Theme；没有新增快捷键。

## Computer Use 真实控件验证

| 操作 | 结果与证据 |
|---|---|
| 点击顶栏输入框、粘贴“离线” | 通过，出现真实待办搜索结果：[search.png](interaction/search.png)。这是粘贴验证，不是拼音组字验证 |
| 点击搜索结果 | 通过，打开“给 CLI 加离线模式”详情，AX 出现编辑/再开会话/返回画面 |
| 编辑项目与默认 Agent，再保存 | 通过，点击 sys、claude 后保存，RPC 读回 space=s1、agent=Claude：[选择截图](interaction/task-project-agent.png)、[落库回执](interaction/task-save-receipt.json)。未启动 Claude |
| 返回分屏，点击“你在这里” | 通过，所属画面内右上角打开真实卡片：[here-open.png](interaction/here-open.png) |
| 原生菜单栏面板点击 | 菜单入口能调用，但 CUA 未取得独立 popover 的可交互 AX；不计面板内点击通过。外观由真实 AppKit 导出复核 |

## 自动检查

见 [checks.txt](checks.txt)。本轮完整 `cargo test --workspace`：91 通过、0 失败、1 ignored（专用子进程 fixture 入口）。未新增镜像样式实现的单测。格式、严格 clippy 与最终客户端回归另外记录。

## 复现

从仓库根目录运行 fixture prepare；在 next 目录按固定工具链构建 `cargo build -p keepline-app --features visual-test`，再 `fixture.py capture after`。`render-design.cjs`、`compare.cjs` 使用 `PLAYWRIGHT_MODULE` 指定本机 Playwright。结束运行 `fixture.py stop`，只停止该 fixture 的终端与后台。

决定：采用本轮组件修复；保留 Herdr 布局和已有功能边界。剩余差异已公开列出，用户人工视觉接受仍待完成，不以编译或截图生成代替。
