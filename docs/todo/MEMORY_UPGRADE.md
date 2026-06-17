# Memory System 2.0 升级计划

> 将 Codex Hub 的 Memory 系统升级到行业领先水平，对标 claude-mem 和 Mem0

## 实施状态

**状态: ✅ 已完成** (2024-12)

所有 5 个阶段已全部实现：
- Phase 1: LanceDB 向量数据库集成 ✅
- Phase 2: AI 语义压缩 ✅
- Phase 3: 扩展 Hooks 系统 ✅
- Phase 4: 自动上下文注入 ✅
- Phase 5: Endless Mode 双层架构 ✅

## 目标

- 从简单的正则匹配升级到 AI 语义理解
- 从 SQLite 单表存储升级到 LanceDB 向量数据库
- 实现自动上下文注入和 Endless Mode
- 压缩比达到 10:1~100:1

## 技术选型

| 组件 | 选择 | 理由 |
|------|------|------|
| 向量数据库 | LanceDB | 嵌入式、TypeScript 原生支持、无需服务器 |
| Embedding | Anthropic Voyage / OpenAI Ada | 高质量文本向量 |
| 压缩 | Claude Agent SDK (Haiku) | 低成本语义压缩 |
| 元数据存储 | SQLite (现有) | 保持兼容 |

---

## Phase 1: LanceDB 向量数据库集成 ✅

### 1.1 安装 LanceDB 依赖 ✅

- [x] 安装 `@lancedb/lancedb` 包
- [x] 安装 `apache-arrow` 用于数据序列化
- [x] 配置 LanceDB 数据目录 (`~/.codex-hub/lancedb/`)

```bash
bun add @lancedb/lancedb apache-arrow
```

### 1.2 创建 VectorStore 适配器 ✅

实现文件: `src/infrastructure/vector/lancedb.adapter.ts`

- [x] 创建 `VectorStore` 接口
  - `initialize()` - 初始化数据库
  - `insert(observation, vector)` - 插入数据
  - `insertBatch(items)` - 批量插入
  - `search(queryVector, options)` - 向量搜索
  - `getById(id)` - 按 ID 查询
  - `getBySessionId(sessionId)` - 按 session 查询
  - `delete(id)` - 删除数据
  - `deleteBySessionId(sessionId)` - 按 session 删除
  - `count()` - 计数
- [x] 创建 `observations` 表 schema

```typescript
interface ObservationSchema {
  id: string;
  sessionId: string;
  content: string;
  vector: number[]; // embedding
  category: 'decision' | 'bugfix' | 'feature' | 'refactor' | 'discovery' | 'change';
  files: string[];
  concepts: string[];
  timestamp: Date;
  tokenCount: number;
  compressed: boolean;
}
```

### 1.3 实现 Embedding 生成 ✅

实现文件: `src/infrastructure/vector/embedding.service.ts`

- [x] 支持多个 embedding 提供者
  - Anthropic Voyage (推荐)
  - OpenAI text-embedding-ada-002
  - 本地 embedding 回退
- [x] 实现批量 embedding 生成
- [x] 添加缓存层避免重复计算

### 1.4 添加语义搜索 API ✅

实现在: `src/web/api/routes/memory.ts`

- [x] `GET /api/memory/search` 端点
- [x] 支持参数：
  - `query` - 搜索文本
  - `limit` - 返回数量 (默认 10)
  - `sessionId` - 可选，限定 session
  - `category` - 可选，限定分类
  - `minScore` - 可选，最小相似度
- [x] 返回相似度分数

---

## Phase 2: AI 语义压缩 ✅

### 2.1 集成 Anthropic SDK ✅

- [x] 安装 `@anthropic-ai/sdk`
- [x] 配置 API key (环境变量 `ANTHROPIC_API_KEY`)

### 2.2 创建 TranscriptCompressor 服务 ✅

实现文件: `src/services/transcript.compressor.ts`

- [x] 实现压缩 prompt 模板
- [x] 使用 Claude Haiku 进行低成本压缩
- [x] 支持 fallback 压缩策略

### 2.3 实现 10:1 压缩 ✅

- [x] 接收原始工具输出 (1,000-10,000 tokens)
- [x] 使用 Claude Haiku 压缩
- [x] 输出 ~500 tokens 的语义观察
- [x] 存储到 LanceDB

### 2.4 添加分类系统 ✅

- [x] 定义 6 个分类：
  - `decision` - 架构/技术决策
  - `bugfix` - Bug 修复
  - `feature` - 新功能实现
  - `refactor` - 代码重构
  - `discovery` - 代码发现/理解
  - `change` - 文件变更
- [x] 在压缩时自动分类
- [x] 支持按分类过滤搜索

---

## Phase 3: 扩展 Hooks 系统 ✅

### 3.1 添加 UserPromptSubmit Hook ✅

实现在: `src/adapters/hook/server.ts`

- [x] 添加 `UserPromptSubmit` 事件类型
- [x] 在首次 prompt 时触发上下文注入
- [x] 存储用户输入用于相关性搜索

### 3.2 添加 SessionEnd Hook ✅

- [x] 添加 `session:end` 事件类型
- [x] 在 Stop 事件时发出
- [x] 触发 session 归档

### 3.3 实现异步压缩处理 ✅

实现文件: `src/services/compression.queue.ts`

- [x] 创建压缩队列
- [x] PostToolUse 触发异步压缩
- [x] 不阻塞主流程
- [x] 支持重试机制
- [x] 统计信息跟踪

---

## Phase 4: 自动上下文注入 ✅

### 4.1 设计注入策略 ✅

实现文件: `src/services/context.injection.ts`

- [x] 默认注入最多 10 条观察
- [x] 可配置最大 tokens (默认 2000)
- [x] 按分类权重排序（decision > bugfix > feature > ...）

### 4.2 实现 Session Start 注入 ✅

- [x] 在 UserPromptSubmit 首次时触发
- [x] 生成格式化的上下文块
- [x] 按 category 分组显示

### 4.3 添加相关性排序 ✅

- [x] 使用项目路径 + 用户 prompt 生成查询
- [x] 按向量相似度排序历史观察
- [x] 应用分类权重调整分数
- [x] 按 token 限制截断

---

## Phase 5: Endless Mode 双层架构 ✅

### 5.1 设计双层 Memory 架构 ✅

实现文件: `src/services/endless.mode.ts`

```
┌──────────────────────────────────────┐
│ Working Memory (in context)          │
│ └── 最近的工具输出 (~50k tokens)      │
├──────────────────────────────────────┤
│ Archive Memory (LanceDB)             │
│ └── 压缩后的观察 (无限制)             │
└──────────────────────────────────────┘
```

- [x] 定义 Working Memory 上限 (默认 50k tokens)
- [x] 超出时自动归档到 Archive Memory
- [x] 保持最近 N 条在 Working Memory

### 5.2 实现实时 Transcript 转换 ✅

- [x] 订阅 `tool:post` 事件
- [x] 自动添加到 working memory
- [x] 格式化为可读内容

### 5.3 添加 Token 使用监控 ✅

- [x] 跟踪每个 session 的 token 使用
- [x] `getSessionTokenCount(sessionId)` 方法
- [x] `getTotalTokenCount()` 全局统计

### 5.4 实现自动压缩触发 ✅

- [x] 设置触发阈值 (默认 80%)
- [x] 自动压缩最旧的记录
- [x] 保持 context 在可控范围
- [x] 保留最少 N 条记录 (默认 5)

---

## API 端点

### 语义搜索
- `GET /api/memory/search?query=...` - 语义搜索观察

### 观察管理
- `GET /api/memory/observations` - 列出所有观察
- `GET /api/memory/observations/:id` - 获取单个观察
- `POST /api/memory/observations` - 添加新观察
- `DELETE /api/memory/observations/:id` - 删除观察
- `DELETE /api/memory/observations/session/:sessionId` - 删除 session 所有观察

### Endless Mode
- `GET /api/memory/endless/stats` - 获取 endless mode 统计
- `GET /api/memory/endless/working/:sessionId` - 获取 session 的 working memory
- `GET /api/memory/endless/context/:sessionId` - 获取格式化的上下文
- `POST /api/memory/endless/compress/:sessionId` - 手动触发压缩
- `POST /api/memory/endless/archive/:sessionId` - 归档整个 session

### 压缩队列
- `GET /api/memory/compression/stats` - 获取压缩队列统计

### Embedding
- `GET /api/memory/embedding/stats` - 获取 embedding 服务统计

---

## 成功指标

| 指标 | 之前 | 之后 |
|------|------|------|
| Session 长度 | ~50 tool uses | 无限制 (Endless Mode) |
| 上下文注入 | 0 tokens | ~2,000 tokens |
| 压缩比 | 1:1 | 10:1~100:1 |
| 搜索类型 | 无 | 语义搜索 (向量) |
| 分类 | 4 类固定 | 6 类智能分类 |
| 存储 | SQLite 单表 | SQLite + LanceDB |

---

## 新增文件列表

```
src/infrastructure/vector/
├── types.ts               # 向量存储类型定义
├── lancedb.adapter.ts     # LanceDB 适配器
├── embedding.service.ts   # Embedding 服务
└── index.ts               # 导出

src/services/
├── transcript.compressor.ts  # AI 压缩服务
├── compression.queue.ts      # 异步压缩队列
├── context.injection.ts      # 上下文注入服务
└── endless.mode.ts           # Endless Mode 服务
```

---

## 参考资料

- [LanceDB Documentation](https://lancedb.com/documentation/)
- [LanceDB TypeScript SDK](https://github.com/lancedb/lancedb)
- [claude-mem Architecture](https://github.com/thedotmack/claude-mem)
- [Mem0 Design](https://github.com/mem0ai/mem0)
- [Anthropic SDK](https://github.com/anthropics/anthropic-sdk-typescript)
