# 待办任务

## 阶段一：项目基础设施

### 高优先级
- [x] 创建 docker-compose.yml（Nakama 3.21.1 + CockroachDB）
- [x] 创建 nakama-config.yml 服务器配置
- [x] 创建 Nakama TypeScript 项目结构（package.json, tsconfig.json）
- [x] 创建 main.ts 入口文件，注册 match handler
- [x] 创建 types.ts 类型定义（Role, GamePhase, PlayerStatus, OpCode）
- [x] 创建 match_handler.ts 核心游戏逻辑

### 中优先级
- [x] 创建 React + Vite 前端项目结构
- [x] 配置 Tailwind CSS
- [x] 实现 Nakama 客户端连接（@heroiclabs/nakama-js）
- [x] 实现用户认证（匿名登录或邮箱登录）
- [x] 创建基础 UI 布局（房间列表、游戏房间）

## 阶段二：核心游戏逻辑 - 服务器端

### 高优先级 - 角色系统
- [x] 实现角色枚举和接口定义
- [x] 实现角色分配算法（根据人数配置）
- [x] 实现预言家技能（查验身份）
- [x] 实现女巫技能（解药、毒药）
- [x] 实现守卫技能（守护）
- [x] 实现猎人技能（死亡带人）
- [x] 实现狼人技能（集体击杀）
- [x] 实现狼王技能（死亡带人）

### 高优先级 - 游戏流程
- [x] 实现 matchInit（初始化游戏状态）
- [x] 实现 matchJoin/matchLeave（玩家进出）
- [x] 实现夜晚阶段管理（依次执行技能）
- [x] 实现白天阶段管理（发言、投票）
- [x] 实现投票逻辑（票数统计、平票处理）
- [x] 实现死亡处理（遗言、技能触发）
- [x] 实现胜利判定（好人胜/狼人胜）
- [x] 实现 matchLoop 主循环

### 中优先级 - 扩展功能
- [x] 实现白痴角色（免死一次）
- [x] 实现丘比特角色（情侣连线）
- [x] 实现警长竞选系统后端（竞选、投票、移交、1.5票权）
- [x] 实现警长竞选系统前端 UI（竞选界面、投票界面、移交界面）
- [x] 实现发言计时器（DayTimer 组件）
- [x] 实现遗言系统

## 阶段三：核心游戏逻辑 - 客户端

### 高优先级 - 游戏界面
- [x] 实现 GameRoom 组件（圆桌布局）
- [x] 实现 PlayerSeat 组件（座位显示）
- [x] 实现 RoleCard 组件（角色卡牌、翻牌动画）
- [x] 实现 NightOverlay 组件（夜晚遮罩、技能选择）
- [x] 实现 VotingPanel 组件（投票界面）
- [x] 实现 ChatPanel 组件（公共聊天、狼人密聊）
- [x] 实现 DayTimer 组件（发言计时）
- [x] 实现 ResultModal 组件（游戏结果）
- [x] 实现死亡技能 UI（开枪选择界面）

### 中优先级 - 游戏状态
- [x] 实现 useNakama hook（连接、认证、匹配）
- [x] 实现 gameStore（Zustand 状态管理）
- [x] 实现 OpCode 消息处理
- [x] 实现角色技能 UI（预言家查验、女巫用药等）
- [x] 实现投票 UI（选择目标、确认投票）

## 阶段三+：大厅和房间系统

### 高优先级
- [x] 实现大厅页面 UI（Lobby 组件）
- [x] 实现房间列表 UI（RoomList 组件）
- [x] 实现创建房间弹窗（CreateRoomModal）

## 阶段四：聊天和社交

### 高优先级
- [x] 实现公共聊天频道
- [x] 实现狼人专属聊天频道
- [x] 实现死亡玩家观战聊天

### 中优先级
- [x] 实现表情包系统
- [x] 实现快捷语音（预设语音）
- [ ] 实现语音聊天（WebRTC，可选）

## 阶段五：房间和匹配

### 高优先级
- [x] 实现 find_match RPC（快速匹配）
- [x] 实现 create_room RPC（创建房间）
- [x] 实现 list_rooms RPC（房间列表）
- [x] 实现房间设置（人数、角色配置）

### 中优先级
- [x] 实现私密房间（密码保护）
- [x] 实现好友邀请
- [x] 实现观战模式

## 阶段六：用户系统

### 中优先级
- [x] 实现用户注册/登录
- [x] 实现用户统计（胜率、场次）
- [x] 实现等级系统
- [x] 实现成就系统
- [x] 实现排行榜
- [x] 添加国际化支持（i18n）
- [x] 添加游戏回放功能
- [x] 将 i18n 集成到其他组件（RoomList, GameRoom, ChatPanel 等）
- [x] 扩展 i18n 到 UserStats 组件（角色名称、统计标签）
- [x] 扩展 i18n 到 SheriffPanel 组件（警长竞选文本）
- [x] 扩展 i18n 到 AchievementPanel 组件（成就名称和描述）
- [x] 扩展 i18n 到 LevelDisplay 组件（等级称号）
- [x] 扩展 i18n 到 InvitePanel 组件（邀请相关文本）
- [x] 扩展 i18n 到 LeaderboardPanel 组件（排行榜文本）
- [x] 扩展 i18n 到 ReplayPanel 组件（回放相关文本）
- [x] 扩展 i18n 到 AuthPanel 组件（登录/注册文本）
- [ ] 扩展 i18n 到 DayTimer 组件（发言计时器文本）
- [ ] 扩展 i18n 到 QuickPhrases 组件（快捷短语文本）
- [ ] 扩展 i18n 到 EmojiPicker 组件（表情选择器文本）

## 阶段七：UI/UX 优化

### 中优先级
- [x] 添加游戏音效（夜晚、投票、死亡）
- [x] 添加 SoundSettings 音效控制面板（音量滑块、设置持久化）
- [x] 添加角色卡牌动画（RoleCard 组件已实现3D翻牌）
- [x] 添加夜晚/白天过渡动画（PhaseTransition 组件）
- [x] 添加投票动画（VoteAnimation 组件：飞行票据、连线可视化、票数弹跳、结果揭示）
- [x] 添加死亡特效

### 低优先级
- [x] 移动端响应式适配
- [x] 添加键盘快捷键
- [x] 添加主题切换（暗黑/光明）
- [x] 添加 PWA 支持（manifest.json、Service Worker、安装提示、离线支持）

## 阶段八：测试和优化

### 中优先级
- [x] 添加角色技能单元测试
- [x] 添加投票逻辑单元测试
- [x] 添加游戏流程集成测试
- [x] 性能优化（减少网络传输）

### 低优先级
- [x] 添加防作弊措施
- [x] 添加日志和监控
- [x] 编写部署文档
- [x] 创建 README.md 项目说明文档

---

## 游戏规格

- 6-18 人在线对战
- 标准狼人杀规则（支持多种角色配置）
- WebSocket 实时通信
- 响应式 Web 设计

## 技术栈

- 后端: Nakama (TypeScript) + CockroachDB
- 前端: React 18 + TypeScript + Vite + Tailwind CSS
- 通信: WebSocket (nakama-js)
- 状态: Zustand
- 动画: Framer Motion

## 角色配置表

| 总人数 | 狼人 | 预言家 | 女巫 | 守卫 | 猎人 | 村民 |
|--------|------|--------|------|------|------|------|
| 6人    | 2    | 1      | 0    | 0    | 1    | 2    |
| 8人    | 2    | 1      | 1    | 0    | 1    | 3    |
| 9人    | 3    | 1      | 1    | 1    | 0    | 3    |
| 12人   | 4    | 1      | 1    | 1    | 1    | 4    |

*创建时间: 2025-12-30*
