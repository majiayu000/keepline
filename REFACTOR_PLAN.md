# Tasker 前端重构计划：HTML → React + TypeScript + Vite

## 概述

将 1932 行的单体 HTML 文件重构为模块化的 React + TypeScript 应用。

**技术栈：**
- React 18
- TypeScript 5
- Vite (构建工具)
- React-Markdown (Markdown 渲染)
- CSS Modules 或 styled-components (样式)

---

## 重构阶段

### Phase 1: 项目结构搭建 ⏱️
**目标：** 创建 Vite + React 项目结构，保持后端 API 不变

**步骤：**
1. 在 `src/web/` 下创建 `client/` 目录
2. 初始化 Vite React-TS 项目
3. 配置构建输出到 `src/web/public/dist/`
4. 更新后端 server.ts 支持 SPA 路由
5. 验证：空白 React 页面可正常显示

**文件结构：**
```
src/web/
├── client/                 # 新 React 应用
│   ├── src/
│   │   ├── main.tsx       # 入口
│   │   ├── App.tsx        # 根组件
│   │   ├── index.css      # 全局样式（主题变量）
│   │   ├── types/         # TypeScript 类型
│   │   ├── hooks/         # 自定义 Hooks
│   │   ├── components/    # UI 组件
│   │   ├── services/      # API 调用
│   │   └── utils/         # 工具函数
│   ├── index.html
│   ├── vite.config.ts
│   ├── tsconfig.json
│   └── package.json
├── api/
│   └── server.ts          # 后端 API（保持不变）
└── public/
    └── index.html         # 旧版备份
```

---

### Phase 2: 类型定义与 API 层 ⏱️
**目标：** 建立类型安全的 API 通信层

**步骤：**
1. 创建 `types/session.ts` - 复用后端 Session 类型
2. 创建 `types/api.ts` - API 响应类型
3. 创建 `services/api.ts` - 封装 fetch 调用
4. 验证：控制台能正确打印 sessions 数据

**API 函数：**
```typescript
// services/api.ts
fetchSessions(): Promise<ApiResponse<SessionsData>>
syncSessions(): Promise<ApiResponse<SyncResult>>
recoverSession(id: string, method: string): Promise<ApiResponse>
completeSession(id: string): Promise<ApiResponse>
stopSession(id: string, force: boolean): Promise<ApiResponse>
```

---

### Phase 3: 主题系统迁移 ⏱️
**目标：** 将 5 个主题的 CSS 变量迁移到 React 上下文

**步骤：**
1. 创建 `index.css` - 所有 CSS 变量（保持原有定义）
2. 创建 `hooks/useTheme.ts` - 主题切换 Hook
3. 创建 `contexts/ThemeContext.tsx` - 主题上下文
4. 创建 `components/ThemeSwitcher.tsx` - 主题切换器
5. 验证：可以切换 5 个主题，样式正确

**主题列表：**
- cyberpunk (默认)
- matrix
- synthwave
- minimal
- tokyo

---

### Phase 4: 基础组件开发 ⏱️
**目标：** 创建可复用的基础 UI 组件

**组件列表：**
1. `Button` - 按钮（primary, danger, success, warning 变体）
2. `Modal` - 模态框
3. `Toast` - 提示消息
4. `Spinner` - 加载指示器

**步骤：**
1. 每个组件创建 `.tsx` + `.module.css`
2. 迁移对应 CSS 样式
3. 验证：组件在各主题下显示正确

---

### Phase 5: 布局组件 ⏱️
**目标：** 创建页面布局结构

**组件列表：**
1. `Header` - 顶部导航（Logo + ThemeSwitcher + 操作按钮）
2. `StatsBar` - 统计条（Running/Waiting/Idle/Lost 计数）
3. `Layout` - 整体布局容器

**步骤：**
1. 迁移 Header HTML 和样式
2. 实现 StatsBar 动态数据绑定
3. 验证：页面布局与原版一致

---

### Phase 6: Session 组件（核心）⏱️
**目标：** 实现会话卡片和详情面板

**组件列表：**
1. `SessionCard` - 会话卡片（状态指示、路径、任务）
2. `SessionDetailPanel` - 展开详情面板
3. `SessionList` - 会话列表（分组：Active/Standby/Disconnected）
4. `ResponsePanel` - 可调整大小的回复面板

**步骤：**
1. 创建 SessionCard 基础结构
2. 实现展开/收起逻辑（useState）
3. 迁移 ResponsePanel 的拖拽调整功能
4. 集成 react-markdown 渲染内容
5. 验证：点击卡片可展开，拖拽调整大小正常

---

### Phase 7: 交互功能 ⏱️
**目标：** 实现所有操作功能

**功能列表：**
1. Recover Session（Resume/Continue）
2. Stop Session（SIGTERM/SIGKILL）
3. Complete/Clear Session
4. 确认对话框

**步骤：**
1. 创建 `ConfirmModal` 组件
2. 实现操作按钮的 API 调用
3. 添加 Toast 反馈
4. 验证：所有操作功能正常

---

### Phase 8: 状态管理与自动刷新 ⏱️
**目标：** 实现全局状态和自动刷新

**步骤：**
1. 创建 `hooks/useSessions.ts` - 会话数据 Hook
2. 实现 30 秒自动刷新
3. 添加键盘快捷键（R 刷新，1-5 切换主题，Esc 关闭）
4. 验证：数据自动更新，快捷键正常

---

### Phase 9: 清理与优化 ⏱️
**目标：** 移除旧代码，优化构建

**步骤：**
1. 删除旧的 `public/index.html`
2. 更新后端 server.ts 指向新的构建产物
3. 更新 package.json 构建脚本
4. 添加生产构建优化
5. 验证：生产构建正常，所有功能完整

---

## 每阶段验证清单

| Phase | 验证项 |
|-------|--------|
| 1 | React 页面可访问 http://localhost:3377 |
| 2 | 控制台打印 sessions 数据 |
| 3 | 5 个主题切换正常 |
| 4 | Button/Modal/Toast 组件工作 |
| 5 | Header + StatsBar 显示正确 |
| 6 | Session 卡片展开/收起/拖拽正常 |
| 7 | Recover/Stop/Complete 操作成功 |
| 8 | 自动刷新 + 快捷键工作 |
| 9 | 生产构建完整功能 |

---

## 风险与回退

- **回退方案：** 保留原 index.html 作为 backup
- **并行开发：** 新旧版本可同时运行（不同路由）
- **渐进迁移：** 每阶段完成后测试再进行下一阶段

---

## 预估文件数

| 类型 | 数量 |
|------|------|
| 组件 (.tsx) | ~15 |
| 样式 (.css/.module.css) | ~10 |
| Hooks | ~4 |
| Services | ~1 |
| Types | ~3 |
| 配置文件 | ~4 |
| **总计** | ~37 文件 |

---

## 开始执行

确认此计划后，将从 **Phase 1** 开始逐步实施。
