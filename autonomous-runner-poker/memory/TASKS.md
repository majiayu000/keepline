# 待办任务

## 阶段一：项目基础设施

### 高优先级
- [x] 创建 docker-compose.yml（Nakama + CockroachDB）
- [x] 创建 Nakama TypeScript 项目结构（package.json, tsconfig.json）
- [x] 创建 main.ts 入口文件，注册 match handler
- [x] 实现基础 match handler（matchInit, matchJoin, matchLeave）
- [x] 创建 find_match RPC 函数（查找或创建匹配）

### 中优先级
- [x] 创建 React + Vite 前端项目结构
- [x] 实现 Nakama 客户端连接（@heroiclabs/nakama-js）
- [x] 实现用户认证（匿名登录或邮箱登录）
- [x] 实现 WebSocket 连接和匹配加入
- [x] 创建基础 UI 布局（牌桌、玩家位置）

## 阶段二：核心游戏逻辑

### 高优先级 - 服务器端
- [x] 实现 Deck 类（洗牌、发牌）
- [x] 实现游戏状态管理（GameState 类型定义）
- [x] 实现下注逻辑（bet, call, raise, fold, check, all-in）
- [x] 实现游戏流程控制（pre-flop → flop → turn → river → showdown）
- [x] 实现 matchLoop 处理玩家操作消息

### 高优先级 - 客户端（基础组件已创建，需要完善功能）
- [x] 实现扑克牌组件（Card.tsx）- 显示牌面
- [x] 实现玩家组件（Player.tsx）- 显示昵称、筹码、状态
- [x] 实现牌桌组件（Table.tsx）- 椭圆形布局
- [x] 实现下注控制组件（BettingControls.tsx）
- [x] 实现游戏状态接收和更新（OpCode 消息处理、turnInfo、showdown 显示）

## 阶段三：牌型评估

### 高优先级
- [x] 实现手牌评估器（HandEvaluator）
  - [x] 识别高牌
  - [x] 识别一对、两对
  - [x] 识别三条、四条
  - [x] 识别顺子
  - [x] 识别同花
  - [x] 识别葫芦
  - [x] 识别同花顺、皇家同花顺
- [x] 实现手牌比较逻辑（谁赢？平局？）
- [x] 实现最佳 5 张牌选择（从 7 张中选）
- [x] 集成到 match_handler Showdown 阶段

## 阶段四：完善游戏体验

### 中优先级
- [x] 实现游戏计时器（每回合限时）- 前端倒计时已实现
- [x] 添加手牌评估器单元测试（vitest，41 个测试用例）
- [x] 添加 betting.ts 单元测试（52 个测试用例）
- [x] 添加 game_state.ts 单元测试（93 个测试用例）
- [x] 添加 deck.ts 单元测试（78 个测试用例）
- [x] 完善边池计算（side pots for all-in）- 后端已实现，前端展示已完成
- [x] 实现玩家断线重连
- [x] 实现游戏房间列表
- [x] 实现观战模式

### 低优先级
- [x] 添加游戏音效（Web Audio API 合成音效、SoundContext 全局管理、SoundSettings 组件）
- [x] 添加扑克牌发牌动画（Card 组件 deal/flip/highlight 动画、社区牌动画触发）
- [x] 添加筹码动画（ChipAnimation 组件、下注时筹码飞向底池、胜利时筹码飞向赢家）
- [x] 添加胜利特效（VictoryEffect 组件、烟花粒子效果、WinnerGlow 高亮效果）
- [x] 实现聊天功能（Chat 组件集成、折叠/展开、未读计数、时间戳显示）
- [x] 添加键盘快捷键支持（F-fold, C-call/check, B-bet, R-raise, A-all-in, 1234-快捷下注, ↑↓-调整金额）
- [x] 移动端响应式适配（Table/Card/Player 响应式尺寸、BettingControls 移动端优化、viewport meta 配置）
- [x] 添加表情快捷发送（预设表情：GG、NH、WP 等，QuickEmotes 组件，可展开面板）
- [x] 添加玩家头像系统（默认头像基于用户ID生成渐变颜色+首字母，支持自定义头像URL）
- [x] 实现 Toast 通知系统（ToastContext + Toast 组件，4种类型，进度条动画）
- [ ] 优化加载性能（代码分割、懒加载组件）
- [x] 添加 PWA 支持（vite-plugin-pwa、OfflineIndicator、PWAUpdatePrompt、扑克主题图标）

## 阶段五：用户系统

### 中优先级
- [x] 实现用户注册/登录（AuthForm 组件支持游客/邮箱登录/邮箱注册）
- [x] 实现用户筹码余额（user_chips.ts RPC + Nakama Storage）
- [x] 实现每日签到奖励筹码（claim_daily_reward RPC，每24小时500筹码）
- [x] 实现用户统计（胜率、总收益等）（UserPanel 组件展示）
- [x] 实现排行榜（get_leaderboard RPC + Nakama Leaderboard API）

## 阶段六：优化与部署

### 低优先级
- [ ] 添加防作弊措施
- [ ] 性能优化（减少网络传输）
- [ ] 添加日志和监控
- [ ] 编写部署文档
- [ ] 添加单元测试
- [ ] 添加排行榜 UI 组件
- [ ] 添加游戏历史记录功能
- [ ] 实现多语言支持（i18n）

---
*游戏规格：*
- 2-9 人在线对战
- 标准德州扑克规则
- 无限注模式（No-Limit Texas Hold'em）
- WebSocket 实时通信
- 响应式 Web 设计

*技术栈：*
- 后端: Nakama (TypeScript) + CockroachDB
- 前端: React 18 + TypeScript + Vite + Tailwind CSS
- 通信: WebSocket (nakama-js)

*创建时间: 2025-12-29*
