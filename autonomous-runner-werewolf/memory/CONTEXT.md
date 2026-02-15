# 项目上下文

## 项目概述

开发一个基于 Nakama 游戏服务器的在线多人狼人杀（Werewolf/Mafia）游戏。

**核心功能：**
1. 多人实时在线对战（6-18 人一局）
2. 完整的狼人杀规则实现（多种角色）
3. 实时游戏状态同步
4. 语音/文字聊天系统
5. 投票和技能系统
6. 房间和匹配系统

## 技术栈

### 后端 - Nakama 游戏服务器
- **Nakama Server**: 开源实时多人游戏服务器
- **运行时语言**: TypeScript（类型安全）
- **数据库**: CockroachDB / PostgreSQL
- **协议**: WebSocket（实时通信）、gRPC/HTTP（API）
- **端口**:
  - 7350: HTTP API
  - 7351: 开发者控制台
  - 7349: gRPC API

### 前端 - Web 客户端
- **框架**: React 18 + TypeScript + Vite
- **Nakama SDK**: @heroiclabs/nakama-js
- **样式**: Tailwind CSS
- **状态管理**: Zustand
- **动画**: Framer Motion
- **音频**: Web Audio API

## 狼人杀游戏规则

### 阵营
1. **好人阵营（村民方）**
   - 村民（Villager）：无特殊能力
   - 预言家（Seer）：每晚可查验一人身份
   - 女巫（Witch）：有一瓶解药和一瓶毒药
   - 猎人（Hunter）：死亡时可带走一人
   - 守卫（Guard）：每晚可守护一人免受狼人袭击
   - 白痴（Idiot）：被投票出局时可翻牌免死一次

2. **狼人阵营**
   - 狼人（Werewolf）：每晚可集体杀死一名玩家
   - 狼王（Alpha Wolf）：死亡时可带走一人

3. **第三方阵营（可选）**
   - 丘比特（Cupid）：首夜连接两人为情侣
   - 盗贼（Thief）：首夜可偷取一个角色

### 游戏流程
1. **首夜**：特殊角色执行首夜技能（丘比特连线等）
2. **夜晚阶段**：
   - 狼人睁眼，选择击杀目标
   - 女巫使用解药/毒药
   - 预言家查验
   - 守卫守护
3. **白天阶段**：
   - 公布昨晚死亡情况
   - 发言阶段（顺序/自由发言）
   - 投票阶段
   - 处决或平票
4. **胜利条件**：
   - 好人胜利：所有狼人死亡
   - 狼人胜利：狼人数量 ≥ 好人数量

### 人数配置（标准局）
| 总人数 | 狼人 | 神职 | 村民 |
|--------|------|------|------|
| 6人    | 2    | 2    | 2    |
| 8人    | 2    | 3    | 3    |
| 9人    | 3    | 3    | 3    |
| 12人   | 4    | 4    | 4    |

## Nakama 核心概念

### Match Handler（匹配处理器）
关键函数：
- `matchInit`: 初始化游戏状态、分配角色
- `matchJoinAttempt`: 验证玩家加入请求
- `matchJoin`: 玩家成功加入后执行
- `matchLeave`: 玩家离开时执行
- `matchLoop`: 游戏主循环（处理夜晚/白天阶段、投票、技能）
- `matchSignal`: 处理外部信号
- `matchTerminate`: 游戏结束时清理

### 消息协议（OpCode）
```typescript
enum OpCode {
  // 游戏状态
  GAME_STATE = 1,
  ROLE_ASSIGNED = 2,
  PHASE_CHANGE = 3,

  // 玩家操作
  PLAYER_ACTION = 10,
  VOTE = 11,
  USE_SKILL = 12,

  // 聊天
  CHAT_MESSAGE = 20,
  WOLF_CHAT = 21,  // 狼人专属频道

  // 结果
  NIGHT_RESULT = 30,
  VOTE_RESULT = 31,
  GAME_OVER = 32,

  // 系统
  PLAYER_READY = 40,
  PLAYER_DISCONNECTED = 41,
  PLAYER_RECONNECTED = 42,
}
```

## 项目结构

```
workspace/
├── docker-compose.yml      # Docker 配置
├── nakama-config.yml       # Nakama 服务器配置
├── nakama/                 # Nakama 服务器代码
│   ├── package.json
│   ├── tsconfig.json
│   ├── src/
│   │   ├── main.ts         # 入口文件
│   │   ├── werewolf/
│   │   │   ├── types.ts          # 类型定义（角色、阶段、状态）
│   │   │   ├── match_handler.ts  # 匹配处理器
│   │   │   ├── game_state.ts     # 游戏状态管理
│   │   │   ├── roles.ts          # 角色技能实现
│   │   │   ├── voting.ts         # 投票逻辑
│   │   │   ├── night_actions.ts  # 夜晚行动处理
│   │   │   └── day_actions.ts    # 白天行动处理
│   │   └── rpc/
│   │       ├── find_match.ts     # 匹配查找
│   │       ├── list_rooms.ts     # 房间列表
│   │       └── user_profile.ts   # 用户数据
│   └── build/                    # 编译输出
├── client/                 # React 前端
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── App.tsx
│   │   ├── components/
│   │   │   ├── GameRoom.tsx      # 游戏房间
│   │   │   ├── PlayerSeat.tsx    # 玩家座位
│   │   │   ├── RoleCard.tsx      # 角色卡牌
│   │   │   ├── VotingPanel.tsx   # 投票面板
│   │   │   ├── ChatPanel.tsx     # 聊天面板
│   │   │   ├── NightOverlay.tsx  # 夜晚遮罩
│   │   │   ├── DayTimer.tsx      # 白天计时器
│   │   │   └── ResultModal.tsx   # 结果弹窗
│   │   ├── hooks/
│   │   │   ├── useNakama.ts      # Nakama 连接
│   │   │   ├── useGameState.ts   # 游戏状态
│   │   │   └── useSoundEffects.ts # 音效
│   │   ├── store/
│   │   │   └── gameStore.ts      # Zustand 状态
│   │   ├── types/
│   │   │   └── werewolf.ts       # 前端类型
│   │   └── assets/
│   │       ├── roles/            # 角色图片
│   │       └── sounds/           # 音效文件
│   └── public/
└── README.md
```

## 开发环境设置

### 启动 Nakama 服务器
```bash
# 编译 TypeScript
cd nakama && npm install && npm run build

# 启动 Docker（Nakama + CockroachDB）
docker-compose up --build
```

### 启动前端开发服务器
```bash
cd client && npm install && npm run dev
```

### 端口
- Nakama API: http://localhost:7350
- Nakama Console: http://localhost:7351 (admin/password)
- 前端: http://localhost:3000

## 最近完成的工作

- **2026-01-01**: 创建 docker-compose.yml（Nakama 3.21.1 + CockroachDB 23.1.11）
- **2026-01-01**: 创建 nakama-config.yml 服务器配置（含控制台、WebSocket、匹配系统配置）
- **2026-01-01**: 完善 Nakama TypeScript 项目结构，添加 nakama-runtime 依赖
- **2026-01-01**: 创建 types.ts 完整类型系统（10种角色、游戏阶段、玩家状态、消息协议等）
- **2026-01-01**: 创建 main.ts 入口文件，注册 match handler 和 RPC 端点（create_match, find_match, list_matches）
- **2026-01-01**: 创建 match_handler.ts 核心游戏逻辑（~800行），包含:
  - 完整的 Nakama Match Handler 实现
  - 游戏状态初始化和角色分配
  - 夜晚阶段管理（狼人击杀、预言家查验、女巫用药、守卫守护）
  - 白天阶段管理（讨论、投票、处决）
  - 胜利条件判定（好人胜/狼人胜）
  - 聊天系统（公共聊天、狼人密聊）
  - 玩家加入/离开/重连处理
- **2026-01-01**: 创建 nakama-runtime.d.ts 类型定义（Nakama Runtime API 类型）
- **2026-01-01**: 验证编译成功，输出 build/index.js (31.7kb)
- **2026-01-01**: 修复 TypeScript 编译错误（ctx.matchId、TextDecoder、setTimeout 与 Nakama 运行时兼容性问题）
- **2026-01-01**: 创建 React + Vite 前端项目结构，包含:
  - package.json (React 18, Vite 5, Tailwind CSS, Zustand, Framer Motion, nakama-js)
  - vite.config.ts (代理配置到 Nakama 7350 端口)
  - Tailwind CSS 配置（狼人杀主题色、游戏专用组件样式）
  - 基础 UI 布局（大厅、房间列表、游戏房间三个页面）
  - types/werewolf.ts 前端类型定义（与后端保持一致）
  - store/gameStore.ts Zustand 状态管理
- **2026-01-01**: 创建 useNakama hook（~600行），实现:
  - Nakama 客户端连接和 WebSocket 管理
  - 设备认证（匿名登录）
  - 房间操作（创建、加入、离开、列表、快速匹配）
  - 游戏消息处理（OpCode 消息解析和分发）
  - 技能/投票/聊天消息发送
  - TypeScript 类型检查通过
- **2026-01-01**: 创建 PlayerSeat 组件（座位显示），包含:
  - 玩家头像、名称、座位号显示
  - 存活/死亡/发言状态可视化
  - 角色图标（自己可见）
  - 准备状态标识
  - 投票目标指示
  - Framer Motion 动画效果
- **2026-01-01**: 创建 GameRoom 组件（圆桌布局），包含:
  - 座位圆形排列算法（支持6-12人）
  - 游戏阶段显示（夜晚/白天/投票/结算）
  - 夜晚遮罩效果（径向渐变）
  - 投票/技能目标选择
  - 角色技能使用界面
  - 胜利结算显示
  - 与 Nakama 完全集成
- **2026-01-01**: 创建 ChatPanel 组件（聊天面板），包含:
  - 公共聊天频道和狼人密语频道（标签切换）
  - 消息发送/显示（Framer Motion 动画）
  - 系统消息特殊样式（黄色标签）
  - 夜晚发言限制（好人禁言、狼人可密聊）
  - 死亡玩家禁言提示
  - 与 GameRoom 完全集成
- **2026-01-01**: 实现猎人/狼王死亡带人技能（完整实现），包含:
  - 添加 OpCode.DEATH_SKILL_PROMPT (35) 和 OpCode.DEATH_SKILL_RESULT (36)
  - 添加 PlayerStatus.DEAD_BY_ALPHA_WOLF 状态
  - 添加 GameState 死亡技能相关字段（pendingShooters, currentShooter, shooterTarget 等）
  - 实现 canUseDeathSkill 函数（猎人被毒死不能开枪）
  - 实现 checkAndEnterDeathSkillPhase 函数（检测并进入死亡技能阶段）
  - 实现 startNextShooter 和 processShooterAction 函数（处理开枪逻辑）
  - 实现链式反应（猎人开枪打死狼王，狼王也能开枪）
  - 修改 processNightResults 在夜晚死亡后触发死亡技能
  - 修改 transitionPhase LAST_WORDS case 在投票出局后触发死亡技能
  - 添加 DEATH_SKILL 阶段超时处理（15秒不选择自动跳过）
  - 编译通过，输出 build/index.js (38.7kb)
- **2026-01-01**: 实现前端死亡技能 UI（开枪选择界面），包含:
  - useNakama hook 添加 sendShoot 函数和 DEATH_SKILL 阶段消息处理
  - GameRoom 组件添加开枪阶段的目标选择逻辑
  - GameRoom 添加"确认开枪"和"放弃开枪"按钮
  - PlayerSeat 组件添加 isShooter 属性，显示开枪者标识（🔫图标）
  - 中心提示区显示开枪阶段专属提示
  - TypeScript 编译通过
- **2026-01-01**: 实现 VotingPanel 组件（独立投票界面），包含:
  - 创建 VotingPanel.tsx（~350行），包含 VoteOption、VoteStats、VoteResultDisplay 子组件
  - 实时票数统计和进度条显示
  - 投票选项卡片（玩家信息、被投票者列表、票数）
  - 倒计时显示（紧急时变红闪烁）
  - 投票确认和弃权按钮
  - 投票结果展示（平票/处决/票数排行）
  - VotingModal 弹窗模式支持
  - 更新 gameStore 添加 voteResult 状态
  - 更新 GameRoom 添加侧边栏标签切换（聊天/投票）
  - 投票阶段自动切换到投票面板
  - Framer Motion 动画效果
  - TypeScript 编译通过
- **2026-01-01**: 实现 RoleCard 组件（角色卡牌、翻牌动画），包含:
  - 创建 RoleCard.tsx（~400行），支持正反面展示
  - CardBack 卡牌背面组件（月亮动画装饰）
  - CardFront 卡牌正面组件（阵营颜色、角色图标、技能描述）
  - 3D 翻牌动画（rotateY 变换、光效）
  - 多尺寸支持（sm/md/lg/xl）
  - RoleRevealModal 角色揭示弹窗（游戏开始时使用）
  - SeerCheckResult 预言家查验结果组件（factionOnly 模式）
  - TypeScript 编译通过
- **2026-01-01**: 实现 NightOverlay 组件（夜晚技能选择界面），包含:
  - 创建 NightOverlay.tsx（~450行），完整的夜晚阶段 UI
  - StarField 星空背景动画（30 个随机闪烁的星星）
  - PlayerCard 玩家选择卡片（头像、座位号、存活状态、选中标记）
  - WitchPanel 女巫药水面板（解药/毒药选择、被杀玩家信息）
  - 针对不同角色显示对应的技能界面（狼人/预言家/女巫/守卫）
  - 非行动角色显示等待动画（闭眼休眠状态）
  - 确认/跳过按钮
  - 与 gameStore 完全集成（夜晚技能状态管理）
  - TypeScript 编译通过
- **2026-01-01**: 实现 ResultModal 组件（游戏结束弹窗），包含:
  - 创建 ResultModal.tsx（~380行），完整的游戏结果展示
  - PlayerResultCard 玩家结果卡片（角色、存活状态、胜负标记）
  - GameStats 统计面板（总玩家、幸存者、狼人数、好人数、游戏天数）
  - Fireworks 烟花庆祝动画（20 个随机彩色粒子）
  - 按阵营分组展示玩家（狼人/好人/中立）
  - GameOverBanner 横幅组件（游戏结束顶部通知）
  - 阵营特色样式（狼人红/好人绿/中立紫）
  - 再来一局/返回大厅按钮
  - TypeScript 编译通过
- **2026-01-01**: 实现 DayTimer 组件（发言计时器），包含:
  - 创建 DayTimer.tsx（~480行），完整的白天发言计时系统
  - CircularProgress 圆形进度条组件（SVG 动画、紧急状态闪烁）
  - 支持顺序发言和自由发言两种模式
  - 当前发言者信息展示（头像、座位号、发言状态动画）
  - 发言顺序进度条（已发言/当前/待发言状态）
  - CompactTimer 紧凑计时器（顶部状态栏使用）
  - SpeakingQueue 发言队列组件（侧边栏显示完整发言顺序）
  - 紧急状态（最后10秒）视觉效果（红色闪烁、光晕）
  - 控制按钮（跳过发言、结束发言）
  - gameStore 添加 SpeakingState 发言状态管理
  - 集成到 GameRoom 组件（圆桌中心、顶部状态栏、侧边栏）
  - TypeScript 编译通过
- **2026-01-01**: 增强 NightOverlay 组件和 gameStore，包含:
  - 更新 gameStore 添加夜晚技能状态：seerCheckResult（预言家查验结果）、witchPotions（女巫药品状态）、nightKillInfo（今晚被杀者信息）、lastGuardTarget（守卫上一晚守护目标）、witchAction（女巫选择的技能类型）
  - 新增 SeerCheckResult、WitchPotions、NightKillInfo 接口定义
  - 改进 WitchPanel 女巫面板：显示被杀玩家信息（平安夜提示）、药品使用状态可视化、三选一按钮（解药/毒药/不使用）
  - 新增 GuardPanel 守卫面板：不能连续守护同一人的警告提示、存活玩家列表、确认守护/不守护按钮
  - 新增 SeerResultPanel 预言家查验结果面板：翻牌动画展示查验结果、阵营颜色区分
  - TypeScript 编译通过
- **2026-01-01**: 实现白痴角色完整功能（投票出局免死一次），包含:
  - 后端 types.ts 添加 OpCode.IDIOT_REVEALED (37) 消息类型
  - 后端 match_handler.ts processVotes 函数完善白痴翻牌逻辑：翻牌后免死、广播 IDIOT_REVEALED 消息
  - 后端 handleVote 函数添加已翻牌白痴投票权限检查（翻牌后不能投票）
  - 后端投票完成检查排除已翻牌白痴（votingPlayers 过滤）
  - 前端 werewolf.ts 添加 OpCode.IDIOT_REVEALED 和 Player.idiotRevealed 字段
  - 前端 useNakama hook 添加 handleIdiotRevealed 消息处理函数
  - 前端 VotingPanel 组件添加白痴投票权限检查和提示信息
  - 前端投票统计排除已翻牌白痴（eligibleVoters 计算）
  - 编译通过，后端 build/index.js (39.6kb)
- **2026-01-01**: 实现情侣系统（丘比特角色）完整功能，包含:
  - 后端 types.ts 添加 OpCode.CUPID_LINK (38)、OpCode.LOVER_DEATH (39)、Faction.LOVERS
  - 后端 types.ts 添加 GameState 丘比特相关字段（cupidTarget1、cupidTarget2、loversLinked、loversFaction）
  - 后端 match_handler.ts 实现情侣辅助函数：areLoversAlive、getLoverIds、areLoversFromDifferentFactions、processLoverDeath
  - 后端 checkWinCondition 添加情侣获胜条件判定（情侣双方存活且仅剩情侣时获胜）
  - 后端 handleUseSkill 添加 Role.CUPID case（首夜选择两名玩家成为情侣）
  - 后端 processNightResults 添加情侣殉情逻辑（一方死亡另一方随死）
  - 后端 processVotes 添加情侣殉情逻辑（投票出局触发殉情）
  - 后端 processShooterAction 添加情侣殉情逻辑（开枪击杀触发殉情）
  - 后端 endGame 添加情侣获胜消息展示
  - 前端 werewolf.ts 添加 OpCode.CUPID_LINK、OpCode.LOVER_DEATH、Player.isLover、Player.loverId、LoverInfo 接口
  - 前端 gameStore.ts 添加 myLoverInfo、cupidTargets、nightSubPhase 状态
  - 前端 NightOverlay.tsx 添加 CupidPanel 组件（选择两名玩家成为情侣的 UI）
  - 前端 ResultModal.tsx 添加 Faction.LOVERS 样式（情侣胜利展示）
  - 前端 useNakama.ts 添加 handleLoverDeath 消息处理
  - 编译通过，后端 build/index.js (47.5kb)
- **2026-01-01**: 实现游戏音效系统（完整实现），包含:
  - 创建 useSoundEffects.ts hook（~600行），使用 Web Audio API
  - SoundType 枚举定义 23 种音效类型（阶段、技能、事件、界面）
  - SoundManager 类：AudioContext 管理、合成音效播放、自定义音频加载
  - 预设音效配置：狼嚎（夜晚）、鸟鸣（白天）、投票提示、技能音效等
  - 支持多音符序列播放（复杂音效）和单音符播放（简单音效）
  - 自动响应游戏状态：阶段变化、玩家加入/离开、玩家死亡、消息接收
  - 便捷方法：playPhaseSound、playSkillSound
  - 集成到 GameRoom 组件：音效开关按钮、准备/投票/技能使用时播放音效
  - TypeScript 编译通过
- **2026-01-01**: 完善游戏音效系统（增加设置组件和更多集成），包含:
  - 创建 SoundSettings.tsx 音效控制面板（~220行）
  - 音量滑块、开关按钮、测试音效按钮
  - 本地存储设置持久化（localStorage）
  - FloatingSoundControl 浮动按钮、SoundToggleButton 简洁开关
  - 集成到 ResultModal：胜利/失败音效（根据玩家阵营判断）、按钮点击音效、只播放一次防止重复
  - 集成到 VotingPanel：倒计时警告音效（最后10秒每秒播放，30秒内每5秒滴答）、选择/确认/弃权按钮音效
  - TypeScript 编译通过
- **2026-01-01**: 实现警长竞选系统前端 UI（完整实现），包含:
  - 创建 SheriffPanel.tsx（~550行），完整的警长系统界面
  - CampaignPhase 竞选报名阶段：加入/退出竞选按钮、候选人列表、倒计时
  - SpeechPhase 竞选发言阶段：当前发言者展示、发言进度条、候选人列表
  - VotingPhase 警长投票阶段：候选人卡片、投票选择、票数显示、确认投票按钮
  - TransferPhase 警徽移交阶段：可选玩家列表、移交/撕毁按钮
  - SheriffBadge 警徽组件：显示当前警长信息和 1.5 票权重
  - SheriffStatusBar 状态条：游戏界面顶部显示当前警长
  - ElectionResult 选举结果弹窗：当选/平票展示
  - 更新 useNakama hook 添加警长系统消息处理（9 个 OpCode 处理函数）
  - 添加 joinSheriffCampaign、quitSheriffCampaign、sendSheriffVote、sendSheriffTransfer 操作函数
  - 更新 gameStore 添加警长状态管理和重置逻辑
  - 集成到 GameRoom 组件：侧边栏警长标签、警长状态栏、自动切换到警长面板
  - 更新 getPhaseDisplay 支持警长阶段显示（竞选/发言/投票/移交）
  - 更新 App.tsx 传递警长相关回调到 GameRoom
  - TypeScript 编译通过，前后端代码完整对接
- **2026-01-01**: 实现遗言系统（完整实现），包含:
  - 后端 types.ts 添加 OpCode.LAST_WORDS_* (60-63) 消息类型
  - 后端 types.ts 添加 GameState.lastWordsSpeaker、lastWordsMessage、lastWordsDeathCause 字段
  - 后端 match_handler.ts 实现 startLastWordsPhase 函数（开始遗言阶段，广播被处决玩家信息）
  - 后端 handleLastWordsSpeak 函数（处理遗言发言，限制500字符）
  - 后端 handleLastWordsSkip 函数（放弃发言）
  - 后端 finishLastWordsPhase 函数（结束遗言阶段，触发死亡技能/警徽移交/进入夜晚）
  - 后端 getDeathCauseString 辅助函数（死因转换为可读字符串）
  - 后端修改 processVotes 使用 startLastWordsPhase 代替手动设置阶段
  - 后端修改 transitionPhase LAST_WORDS case 使用 finishLastWordsPhase
  - 前端 werewolf.ts 添加 GamePhase.LAST_WORDS、OpCode.LAST_WORDS_*、LastWordsInfo 接口
  - 前端 gameStore.ts 添加 lastWordsInfo、lastWordsMessage、isMyLastWords 状态
  - 创建 LastWordsPanel.tsx（~300行），包含遗言输入/显示界面、LastWordsStatusBar 状态栏、LastWordsModal 弹窗
  - 前端 useNakama.ts 添加 handleLastWordsStart、handleLastWordsSpeak、handleLastWordsEnd 消息处理函数
  - 添加 sendLastWords、sendLastWordsSkip 操作函数
  - 更新 getPhaseMessage 添加 LAST_WORDS 阶段消息
  - 集成到 GameRoom 组件：遗言状态栏、遗言弹窗（发言时显示）、阶段显示
  - 更新 App.tsx 传递遗言相关回调到 GameRoom
  - TypeScript 编译通过，后端 build/index.js (64.4kb)
- **2026-01-01**: 实现大厅页面和房间列表 UI（完整实现），包含:
  - 创建 Lobby.tsx（~350行），游戏大厅首页
  - StarBackground 星空动画背景、MoonDecoration 月亮装饰、RoleDecorations 角色图标浮动动画
  - 玩家名输入框（长度限制12字符、验证）
  - 进入游戏按钮（连接状态反馈）、演示模式按钮
  - RulesCard 可折叠游戏规则说明卡片
  - ConnectionStatus 连接状态指示器
  - 创建 RoomList.tsx（~500行），房间列表页面
  - QuickMatchCard 快速匹配卡片（动画背景）
  - RoomCard 房间卡片（状态显示、玩家数进度条、加入按钮）
  - FilterBar 筛选器（全部/等待中/游戏中）和刷新按钮
  - EmptyRoomList 空列表提示、LoadingState 加载骨架屏
  - CreateRoomModal 创建房间弹窗（房间名、人数选择、角色配置预览）
  - RoleConfigPreview 根据人数显示角色配置预览
  - 自动刷新房间列表（每10秒）
  - 更新 App.tsx 集成 Lobby 和 RoomList 组件，简化代码结构
  - TypeScript 编译通过
- **2026-01-01**: 实现死亡玩家观战聊天频道（完整实现），包含:
  - 后端 match_handler.ts handleChat 添加 DEAD_CHAT 频道支持
  - 后端验证死亡玩家权限（只有死亡玩家可使用观战频道）
  - 后端消息包含 role 和 seatNumber 字段（观战频道显示角色信息）
  - 前端 types/werewolf.ts 添加 OpCode.DEAD_CHAT (22) 和 ChatMessage.role/seatNumber 字段
  - 前端 ChatPanel.tsx 添加 👻观战频道 标签页
  - 死亡后自动切换到观战频道
  - 观战消息显示玩家角色标签（阵营颜色区分）
  - 前端 useNakama.ts 更新 sendChat 支持 'public'|'wolf'|'dead' 三种类型
  - 更新 GameRoom.tsx onSendMessage 类型定义
  - 编译通过，后端 build/index.js (65.1kb)
- **2026-01-01**: 实现夜晚/白天过渡动画（完整实现），包含:
  - 创建 PhaseTransition.tsx 组件（~400行），实现阶段切换过渡动画
  - Stars 组件：夜晚星星闪烁效果（30个随机分布的星星，带闪烁动画）
  - Clouds 组件：云朵飘过动画（3朵云从屏幕一侧移动到另一侧）
  - Moon 组件：月亮升起/落下动画（带月牙效果、纹理和光晕）
  - Sun 组件：太阳升起/落下动画（12道光芒、呼吸效果、光晕）
  - Particles 组件：粒子上升效果（白天橙色、夜晚紫色）
  - NightText 组件：「天黑请闭眼」文字动画（带眨眼动画、天数显示）
  - DayText 组件：「天亮了」文字动画（带公鸡打鸣动画、天数显示）
  - 渐变背景色变化：从白天到夜晚/夜晚到白天的5阶渐变过渡
  - usePhaseTransition hook：检测阶段变化，返回过渡类型
  - 集成到 GameRoom 组件，阶段切换时自动触发动画
  - 过渡动画时播放对应音效（NIGHT_START / DAY_START）
  - TypeScript 编译通过
- **2026-01-01**: 添加投票动画系统（完整实现），包含:
  - 创建 VoteAnimation.tsx 组件（~500行），完整的投票动画系统
  - VoteBallot 组件：投票时票据飞行动画（从投票者飞向目标、贝塞尔曲线、拖尾光效）
  - VoteLines 组件：SVG 投票连线可视化（渐变色、箭头标记、发光滤镜、动画绘制）
  - VoteCountBubble 组件：票数弹跳动画（新票时旋转弹入、尺寸支持 sm/md/lg）
  - VoteTargetRing 组件：被投票者座位高亮环（外圈脉冲动画、内圈、票数气泡）
  - VoteResultReveal 组件：投票结果揭示动画（逐行计票、平票/处决结果展示、死亡图标）
  - VoteProgressBar 组件：投票进度条（渐变色、光效扫过、紧急状态闪烁）
  - VoteConfirmButton 组件：确认投票按钮（渐变背景、光效扫过、点击波纹）
  - useVoteAnimation hook：管理飞行事件、结果揭示状态
  - 集成到 GameRoom：圆桌连线可视化、飞行票据、座位票数高亮、结果揭示弹窗
  - 集成到 VotingPanel：增强的票数显示、进度条、确认按钮
  - 监听玩家投票变化自动触发飞行动画并播放音效
  - TypeScript 编译通过
- **2026-01-01**: 集成死亡特效系统到游戏（完整实现），包含:
  - DeathEffect.tsx 已存在完整的死亡特效组件（~960行）：
    - SoulParticle/SoulParticles：灵魂粒子飘散效果
    - DeathFlash：屏幕闪烁效果（不同死因不同颜色）
    - SkullExplosion：骷髅爆炸效果（8个骷髅向外扩散）
    - BloodSplatter：血迹飞溅效果（狼人击杀专用，12个血滴）
    - PoisonMist：毒雾效果（女巫毒杀专用，毒雾圆环+毒气泡）
    - GavelStrike：法槌敲击效果（投票处决专用，冲击波动画）
    - BulletImpact：子弹冲击效果（猎人/狼王开枪专用，弹道+碎片）
    - HeartBreak：心碎效果（殉情专用，破碎心形+心形粒子）
    - DeathBanner：死亡公告横幅（阵营颜色、角色揭示、死因展示）
    - PlayerDeathMask：玩家座位死亡遮罩（灰度渐变、X标记）
    - DeathEffectOverlay：完整特效叠加组件（序列触发所有效果）
  - gameStore.ts 添加死亡事件管理：
    - deathEvents：死亡事件队列
    - addDeathEvent/addDeathEvents：添加死亡事件
    - popDeathEvent：从队列获取事件
    - currentDeathEvent：当前播放的死亡事件
    - clearDeathEvents：清空事件队列
  - useNakama.ts 更新触发死亡事件：
    - handleNightResult：支持新旧两种死亡信息格式，触发死亡事件
    - handleVoteResult：投票处决时触发死亡事件
    - handleDeathSkillResult：猎人/狼王开枪时触发死亡事件
    - handleLoverDeath：殉情时触发死亡事件
    - statusToCause 映射：PlayerStatus 到 DeathCause 的转换
  - GameRoom.tsx 集成死亡特效：
    - 处理死亡事件队列的 useEffect（根据座位号计算位置）
    - 播放死亡音效（SoundType.PLAYER_DEATH）
    - 渲染 DeathEffectOverlay 组件
    - handleDeathEffectComplete 回调处理
  - TypeScript 编译通过，前后端代码完整
- **2026-01-01**: 实现房间设置（人数、角色配置），包含:
  - 后端 match_handler.ts createInitialState 增强：
    - 解析 roles 参数（JSON 格式角色数组）
    - 解析 discussionTime/votingTime/nightActionTime/lastWordsTime 时间配置
    - 解析 allowSheriff/allowLastWords 功能开关
    - getMatchLabel 包含完整房间设置信息
  - 后端 assignRoles 函数优化：
    - 优先使用自定义角色配置（config.roles）
    - 其次使用预设配置（PRESET_CONFIGS）
    - 最后动态生成（按人数计算狼人/神职/村民比例）
  - 前端 types/werewolf.ts 新增类型：
    - RoomSettings 接口（roomName/maxPlayers/roles 等）
    - PRESET_ROLE_CONFIGS 预设角色配置（6/8/9/10/12人）
  - 前端 RoomList.tsx CreateRoomModal 增强（~300行新增）：
    - ROLE_CONFIG_OPTIONS 角色配置项（9种角色含描述/限制/颜色）
    - 自定义角色开关（toggle switch）
    - 角色计数器 UI（+/- 按钮，限制最大数量）
    - 实时角色统计（当前/目标人数）
    - RoleConfigPreview 组件复用
  - 前端 useNakama.ts createMatch 增强：
    - 支持完整 RoomSettings 参数
    - 将 roles 数组转换为 JSON 字符串传递
  - 前端 App.tsx handleCreateRoom 更新：
    - 传递完整 RoomSettings 到 createMatch
  - TypeScript 编译通过，后端 build/index.js (66.6kb)
- **2026-01-01**: 实现表情包系统（完整实现），包含:
  - 创建 EmojiPicker.tsx 组件（~280行），游戏聊天表情包选择器
  - EmojiCategory/Emoji 类型定义（表情分类和单个表情）
  - GAME_EMOJIS 游戏主题表情数据（4分类64个表情）：
    - 狼人杀分类：狼人🐺、夜晚🌙、预言家🔮、女巫🧪、猎人🔫、守卫🛡️等
    - 表情分类：开心😀、笑哭😂、思考🤔、惊恐😱、恶魔😈等
    - 动作分类：挥手👋、点赞👍、鼓掌👏、指人🫵、骷髅💀等
    - 符号分类：红心❤️、火🔥、星星⭐、问号❓、庆祝🎉等
  - QUICK_EMOJIS 快捷表情条（12个常用表情）
  - 表情搜索功能（支持中英文关键词）
  - 最近使用记录（localStorage 持久化，最多16个）
  - 分类标签切换（图标标签页）
  - 点击外部自动关闭
  - Framer Motion 表情选择动画（hover 放大、tap 缩小）
  - EmojiButton 触发按钮组件
  - 更新 ChatPanel.tsx 集成表情包系统：
    - 输入框旁添加表情按钮
    - 表情选择器弹出面板（position="top"）
    - handleEmojiSelect 处理表情插入到光标位置
    - ESC 键关闭选择器
  - TypeScript 编译通过
- **2026-01-01**: 实现移动端响应式适配（完整实现），包含:
  - tailwind.config.js 添加 xs 断点（375px）和 touch 媒体查询
  - index.css 添加移动端全局样式：
    - iOS 安全区域变量（safe-area-inset）
    - 动态视口高度（100dvh）
    - 触摸优化（禁用高亮、防止下拉刷新）
    - 最小按钮高度（44px）
    - 横屏模式优化
  - index.html 添加移动端 meta 标签：
    - viewport-fit=cover（全面屏适配）
    - apple-mobile-web-app-capable（iOS 全屏）
    - theme-color（状态栏颜色）
  - GameRoom.tsx 移动端适配：
    - useIsMobile hook 检测移动端
    - 侧边栏改为底部抽屉式（AnimatePresence 动画）
    - 移动端聊天按钮（带红点提示）
    - 顶部状态栏响应式布局（简化文字）
    - 圆桌区域响应式尺寸（280px-2xl）
    - 底部操作按钮紧凑化
    - 我的角色信息面板响应式
    - 背景遮罩点击关闭抽屉
  - PlayerSeat.tsx 移动端适配：
    - 座位尺寸响应式（14-20）
    - 头像尺寸响应式（10-14）
    - 字体大小响应式
    - 玩家名长度截断（>4字符）
  - Lobby.tsx 移动端适配：
    - 标题字体响应式（5xl-7xl）
    - 月亮装饰响应式尺寸
    - 输入区域响应式间距
    - 连接状态响应式布局
  - TypeScript 编译通过
- **2026-01-01**: 实现快捷语音系统（预设短语消息），包含:
  - 创建 QuickPhrases.tsx 组件（~280行），游戏聊天快捷短语选择器
  - PhraseCategory/Phrase 类型定义（短语分类和单个短语）
  - GAME_PHRASES 游戏主题短语数据（6分类50+短语）：
    - 基础分类：过、好人过、同意、反对、我信你、我怀疑你等
    - 身份分类：我是村民、我是预言家、我验了是好人/狼等（角色限制）
    - 行动分类：投他、跟票、弃票、查一下他、守他、救他、毒他等
    - 分析分类：他是狼、他是好人、假预言家、狼坑、神坑等
    - 狼队分类：刀他、自刀、等一下、刀预言家、我来跳等（狼人专用）
    - 表情分类：哈哈哈、GG、打得好、不好意思、快点等
  - QUICK_PHRASES 快捷短语条（6个常用短语）
  - WOLF_QUICK_PHRASES 狼人专用快捷短语条（5个狼队短语）
  - QuickPhraseBar 组件：输入框上方快捷短语横条
  - WolfPhraseBar 组件：狼人密语频道专用短语条（红色主题）
  - QuickPhraseButton 组件：展开完整短语面板按钮
  - 角色限制过滤：根据当前角色显示对应短语
  - 分类标签切换（图标标签页）
  - 点击短语直接发送消息
  - Framer Motion 动画效果
  - 更新 ChatPanel.tsx 集成快捷语音系统：
    - 输入框上方显示快捷短语条
    - 狼人密语频道显示狼队专用短语条
    - 输入框左侧添加快捷短语按钮
    - 点击按钮展开完整短语选择面板
    - ESC 键关闭选择器
    - handlePhraseSelect 处理短语直接发送
  - TypeScript 编译通过
- **2026-01-01**: 实现键盘快捷键系统（完整实现），包含:
  - 创建 useKeyboardShortcuts.ts hook（~220行），支持 20+ 快捷键
  - 快捷键定义：ESC 取消/关闭、Enter/Space 确认、1-9/0 选择玩家、R 准备
  - 导航快捷键：C 聊天、V 投票、H 警长、S 音效、T 聚焦聊天输入框
  - Tab 切换聊天频道、? 显示帮助
  - ShortcutDefinition 接口、ShortcutCategory 分类
  - getShortcutsByCategory、formatShortcut 辅助函数
  - 输入框中禁用大部分快捷键（ESC 仍可用）
  - 创建 KeyboardShortcutsHelp.tsx 组件（~180行）
  - 快捷键帮助弹窗（4 分类显示、数字键合并说明）
  - ShortcutHint 底部提示条、ShortcutBadge 快捷键徽章
  - 集成到 GameRoom 组件
  - 顶部状态栏添加快捷键帮助按钮（桌面端）
  - 等待阶段底部显示快捷键提示
  - handleKeyboardSelectPlayer 数字键快速选择玩家
  - handleKeyboardConfirm 确认操作（准备/投票/技能/开枪）
  - handleKeyboardSkip 放弃开枪
  - handleKeyboardCancel 取消选择/关闭弹窗
  - TypeScript 编译通过
- **2026-01-01**: 实现 PWA 支持（完整实现），包含:
  - 安装 vite-plugin-pwa 和 workbox-window 依赖
  - 配置 vite.config.ts PWA 插件：
    - manifest.json 自动生成（应用名、图标、主题色、启动 URL）
    - Service Worker 缓存策略（NetworkFirst API、CacheFirst 静态资源）
    - 自动更新注册（autoUpdate）
    - 离线支持配置（navigateFallback）
  - 创建 maskable-icon.svg 可遮罩图标（512x512 安全区域）
  - 创建 generate-icons.html 图标生成工具（64/192/512/180px PNG）
  - 创建 browserconfig.xml Windows Tiles 配置
  - 更新 index.html：
    - apple-touch-icon 链接
    - mask-icon Safari 固定标签页图标
    - Open Graph 和 Twitter Card 社交分享元数据
    - PWA 安装提示脚本（beforeinstallprompt 事件处理）
  - 创建 usePWA.tsx hook（~250行）：
    - isInstallable/isInstalled/isUpdateAvailable/isOffline 状态
    - install() 安装方法、update() 更新方法
    - Service Worker 更新检测
    - 在线/离线状态监听
  - PWAInstallPrompt 组件：安装提示弹窗（狼人杀主题样式）
  - PWAUpdateNotification 组件：新版本可用通知
  - OfflineIndicator 组件：离线状态顶部提示条
  - 集成到 App.tsx：大厅页面显示安装提示、24小时内不重复显示
  - TypeScript 编译通过
- **2026-01-01**: 实现主题切换系统（暗黑/光明），包含:
  - 创建 useTheme.tsx hook（~330行）：
    - ThemeProvider 主题上下文提供者
    - useTheme hook 主题状态管理
    - ThemeColors 完整配色方案接口（背景/文字/边框/游戏/状态颜色）
    - dark/light/system 三种模式支持
    - darkThemeColors/lightThemeColors 预设配色
    - localStorage 持久化
    - 系统主题检测（prefers-color-scheme）
    - applyThemeToDOM CSS 变量应用
  - 创建 ThemeToggle.tsx 组件（~380行）：
    - ThemeToggleButton：仅图标切换按钮（旋转动画）
    - ThemeSwitch：带滑块动画的开关（星星/云朵装饰）
    - ThemeDropdown：下拉菜单选择器（三种模式）
    - ThemeSettingsPanel：完整设置面板（带描述）
    - FloatingThemeToggle：固定角落浮动按钮
    - SunIcon/MoonIcon/SystemIcon SVG 图标组件
  - 更新 tailwind.config.js：
    - 添加 darkMode: 'class' 策略
  - 更新 index.css：
    - :root 默认暗黑主题 CSS 变量
    - :root.light 光明主题 CSS 变量
    - 背景色、文字颜色、边框颜色、游戏专用色、状态颜色
    - 更新组件样式使用 CSS 变量（game-table/player-seat/role-card 等）
    - 添加主题相关工具类（theme-card/theme-panel/theme-text-*）
  - 更新 App.tsx：
    - 添加 ThemeProvider 包装
    - 主容器使用 CSS 变量颜色
  - 更新 Lobby.tsx：
    - 导入 ThemeToggleButton 和 useTheme
    - 左上角添加主题切换按钮
    - 背景色根据主题动态切换（暗黑渐变/光明渐变）
    - 暗黑模式显示星空背景
  - TypeScript 编译通过
- **2026-01-01**: 实现观战模式（完整实现），包含:
  - 后端 types.ts 添加 OpCode.SPECTATOR_* (70-73) 消息类型（JOIN/LEAVE/CHAT/FULL_STATE）
  - 后端 types.ts 添加 Spectator 接口和 GameState.spectators 字段
  - 后端 types.ts Player 接口添加 isSpectator 字段
  - 后端 match_handler.ts matchJoinAttempt 支持观战者加入进行中的游戏
  - 后端 match_handler.ts matchJoin 自动将游戏进行中加入的玩家设为观战者
  - 后端 matchLeave 处理观战者离开
  - 后端 sendSpectatorFullState 函数：向观战者发送完整游戏状态（包含所有角色信息）
  - 后端 broadcastToSpectators 函数：向所有观战者广播消息
  - 后端 handleChat 修改：观战者可以看到所有聊天频道（公共、狼人、死者）
  - 后端 handleSpectatorChat 函数：观战者专属聊天
  - 前端 werewolf.ts 添加 Spectator、SpectatorFullState、SpectatorPlayerView 接口
  - 前端 werewolf.ts ChatMessage 添加 isSpectator 字段
  - 前端 gameStore.ts 添加 isSpectator、spectators、spectatorFullState 状态
  - 前端 useNakama.ts 添加观战消息处理函数（handleSpectatorJoin/Leave/Chat/FullState）
  - 前端 useNakama.ts sendChat 支持 'spectator' 类型
  - 创建 SpectatorView.tsx（~550行），完整的观战者专用视图：
    - SpectatorPlayerCard：显示所有玩家角色、阵营、夜晚行动标记
    - NightInfoPanel：显示夜晚行动信息（狼人目标、守卫保护、预言家查验、女巫药水）
    - SpectatorStatusBar：观战模式状态栏
    - SpectatorChat：支持查看所有频道（公共/狼人/死者/观战）的聊天
    - 按阵营分组显示玩家（狼人/好人/中立）
    - SpectatorBadge：观战模式浮动标识
  - GameRoom.tsx 添加观战模式检测，自动切换到 SpectatorView
  - 编译通过，后端 build/index.js (72.1kb)
- **2026-01-01**: 实现私密房间（密码保护）功能（完整实现），包含:
  - 后端 types.ts GameState 添加 password 和 roomName 字段
  - 后端 match_handler.ts MatchLabel 接口添加 isPrivate 和 roomName 字段
  - 后端 createInitialState 解析 password 和 roomName 参数
  - 后端 getMatchLabel 包含 isPrivate 标记（不暴露实际密码）
  - 后端 matchJoinAttempt 添加密码验证逻辑
  - 后端 main.ts rpcListMatches 返回 isPrivate、roomName、maxPlayers 字段
  - 后端 rpcFindMatch 只匹配公开房间（过滤私密房间）
  - 前端 werewolf.ts RoomSettings 接口添加 password 字段
  - 前端 useNakama.ts MatchInfo 接口添加 isPrivate、roomName、maxPlayers
  - 前端 useNakama.ts createMatch 支持 password 参数
  - 前端 useNakama.ts joinMatch 支持 password 参数（通过 metadata 传递）
  - 前端 useNakama.ts listMatches 使用自定义 RPC 获取扩展字段
  - 前端 RoomList.tsx RoomCard 显示锁图标🔒（私密房间）
  - 前端 RoomList.tsx PasswordEntryModal 组件（密码输入弹窗）
  - 前端 CreateRoomModal 添加私密房间开关和密码输入
  - 前端 App.tsx handleJoinRoom/handleCreateRoom 支持密码参数
  - 编译通过，后端 build/index.js (73.3kb)
- **2026-01-01**: 实现用户统计（胜率、场次）功能（完整实现），包含:
  - 后端 types.ts 添加 UserStats/RoleStats 接口和 createInitialUserStats 函数
  - 后端 main.ts 添加 get_user_stats/record_game_result RPC 端点
  - 后端使用 Nakama storage 存储统计（werewolf_stats 集合）
  - 后端 match_handler.ts endGame 添加 nk 参数
  - 后端 recordGameStats 函数：游戏结束时记录所有玩家统计
  - 统计内容：总场次/胜负/胜率/阵营统计/角色统计/存活率/警长统计/时间戳
  - 前端 werewolf.ts 添加 UserStats/RoleStats 类型
  - 前端 useNakama.ts 添加 getUserStats 函数
  - 前端 UserStats.tsx (~350行)：CompactStats/StatsPanel/StatsModal/StatsButton 组件
  - 前端 Lobby.tsx 集成：顶部栏战绩按钮（连接后显示）、点击打开统计弹窗
  - 编译通过，后端 build/index.js (82.1kb)
- **2026-01-01**: 添加角色技能单元测试（完整实现），包含:
  - nakama/package.json 添加 test/test:watch 脚本和 @types/bun 依赖
  - 创建 src/__tests__/test-utils.ts (~280行) 测试工具模块
  - MockDispatcher/MockLogger 模拟 Nakama 运行时对象
  - createTestPlayer/createTestExtendedState/createTestGameState 测试状态工厂函数
  - setupTestGame 游戏配置辅助函数（支持自定义角色/阶段/人数）
  - checkWinCondition/canUseDeathSkill/processWolfKill/processWitchHeal/processWitchPoison/processSeerCheck 游戏逻辑辅助函数
  - 创建 src/__tests__/roles.test.ts (~400行) 角色技能测试
  - 测试覆盖：Werewolf (3)、Alpha Wolf (2)、Seer (4)、Witch (5)、Hunter (4)、Guard (2)、Idiot (2)、Win Conditions (6)、Cupid (2)、Death Skill Chains (3)
  - 共 33 个测试用例，全部通过
- **2026-01-01**: 添加投票逻辑单元测试（完整实现），包含:
  - 创建 src/__tests__/voting.test.ts (~500行) 投票逻辑测试
  - 测试辅助函数：countVotes（计票逻辑）、canPlayerVote（投票权限检查）、castVote（投票操作）、processIdiotReveal（白痴翻牌处理）
  - Vote Counting (5)：单目标投票、多数票决定、平票无人淘汰、弃权不计票、全弃权无人淘汰
  - Sheriff Vote Weight (4)：警长1.5票权重、警长票打破平局、非警长1票、无警长时所有票1票
  - Voting Eligibility (5)：存活玩家可投票、死亡玩家不能投票、翻牌白痴不能投票、未翻牌白痴可投票、eligible玩家统计
  - Vote Casting (7)：有效目标投票成功、死亡目标投票失败、不存在目标投票失败、弃权投票成功、死亡玩家投票失败、翻牌白痴投票失败、玩家可改票
  - Idiot Reveal on Vote (3)：未翻牌白痴被淘汰时翻牌存活、已翻牌白痴被淘汰死亡、非白痴不触发翻牌机制
  - Multi-Way Ties (2)：三方平局无人淘汰、一人票数最高则淘汰
  - Voting Edge Cases (5)：单人投票、自投有效、狼人可投票、投票含时间戳
  - Complex Voting Scenarios (3)：警长票打破2v2平局、多人弃权仍可决出结果、玩家中途死亡后的投票
  - 共 33 个测试用例，全部通过
  - 完整测试套件：66 个测试用例（33 角色 + 33 投票），全部通过
- **2026-01-01**: 添加游戏流程集成测试（完整实现），包含:
  - 扩展 src/__tests__/test-utils.ts 添加游戏流程辅助函数（~350行新增）：
    - simulateNightPhase：模拟完整夜晚阶段（狼人击杀、守卫守护、女巫用药）
    - countVotes：投票计票逻辑（支持警长1.5票权重）
    - simulateVoting：模拟投票阶段（白痴翻牌检测）
    - processLoverDeath：情侣殉情处理
    - transitionPhase：游戏阶段切换
    - getPendingShooters：获取待开枪的玩家
    - simulateGameTurn：模拟完整游戏回合（夜晚+白天）
  - 创建 src/__tests__/game-flow.test.ts (~600行) 游戏流程集成测试
  - Game Initialization (5)：玩家数量、角色分配、初始状态、女巫药水、座位号
  - Night Phase Flow (7)：狼人击杀、守卫守护、女巫救人、女巫毒人、双重技能、平安夜
  - Day Voting Flow (5)：多数票淘汰、平票无人出局、全弃权、警长票权重、白痴翻牌
  - Multi-Turn Game Flow (3)：村民胜利、狼人胜利、游戏继续
  - Special Scenarios (6)：情侣殉情、跨阵营情侣获胜、猎人开枪、猎人被毒不能开枪、狼王开枪、女巫药水消耗
  - Phase Transitions (4)：等待到夜晚、夜晚到白天、增加天数、投票清空
  - Edge Cases (6)：初始狼人数=村民数、无狼人、单挑局、同夜双杀、守卫自守、翻牌白痴不能投票
  - Complex Game Scenarios (3)：9人局村民胜利、狼人消耗胜利、女巫双杀结束游戏
  - 共 38 个测试用例，全部通过
  - 完整测试套件：104 个测试用例（33 角色 + 33 投票 + 38 流程），全部通过
- **2026-01-01**: 编写部署文档（完整实现），包含:
  - 创建 DEPLOY.md（~600行），完整的部署指南
  - 系统要求（最低/推荐配置、软件要求）
  - 本地开发环境设置（后端/前端启动步骤、端口说明、测试运行）
  - Docker 部署（docker-compose 使用、数据持久化、服务管理）
  - 生产环境部署：
    - Docker Compose 生产配置（docker-compose.prod.yml 示例）
    - Nginx 反向代理配置（SSL/WebSocket 支持）
    - 生产 Nakama 配置示例（安全设置）
    - 云平台部署架构（AWS/阿里云示例）
  - 环境变量配置（后端/前端配置项说明）
  - SSL/HTTPS 配置（Let's Encrypt 证书、自动续期）
  - 监控与日志（日志收集、Prometheus 指标、健康检查）
  - 常见问题解答（6个常见问题及解决方案）
  - 性能优化建议（数据库/Nakama/前端/网络）
  - 安全建议（5条安全最佳实践）
- **2026-01-01**: 实现好友邀请功能（完整实现），包含:
  - 后端 types.ts 添加邀请相关类型：
    - OpCode.INVITE_* (80-85) 邀请消息类型（SENT/RECEIVED/ACCEPTED/DECLINED/EXPIRED/CANCELLED）
    - InviteStatus 枚举（pending/accepted/declined/expired/cancelled）
    - GameInvite 接口（邀请数据结构）
    - INVITE_CONFIG 邀请配置（5分钟过期、最多10个待处理邀请）
  - 后端 main.ts 添加邀请 RPC 端点：
    - rpcSearchUsers：搜索用户（按用户名）
    - rpcSendInvite：发送邀请（验证房间状态、创建邀请记录、发送通知）
    - rpcGetInvites：获取邀请列表（sent/received，自动过期处理）
    - rpcRespondInvite：响应邀请（接受返回 matchId 和密码）
    - rpcCancelInvite：取消已发送邀请
    - readInvites/writeInvites 辅助函数：Nakama storage 读写
    - 使用 Nakama notifications 发送实时邀请通知
  - 前端 werewolf.ts 添加类型：
    - OpCode.INVITE_* 消息类型
    - InviteStatus 枚举
    - GameInvite 接口
    - SearchedUser 接口
  - 前端 useNakama.ts 添加方法：
    - searchUsers：搜索用户
    - sendInvite：发送邀请
    - getInvites：获取邀请列表
    - respondInvite：响应邀请
    - cancelInvite：取消邀请
  - 创建 InvitePanel.tsx（~550行），完整的邀请界面：
    - UserCard：用户搜索结果卡片（头像、在线状态、邀请按钮）
    - InviteCard：邀请卡片（房间信息、剩余时间、接受/拒绝/取消按钮）
    - InviteNotification：邀请通知弹窗（右上角浮动）
    - InviteButton：邀请好友按钮（用于房间列表）
    - InvitePanelModal：邀请面板弹窗
    - 三个标签页：收到的邀请、已发送、搜索用户
    - 倒计时显示、自动刷新、消息提示
  - 编译通过，后端 build/index.js (93.2kb)
- **2026-01-02**: 实现消息压缩优化（减少网络传输），包含:
  - 后端创建 message-compress.ts 压缩模块（~350行）：
    - 字段名映射（displayName→n、seatNumber→s 等 30+ 字段）
    - 枚举值压缩（Role/GamePhase/PlayerStatus/ConnectionStatus/NightSubPhase 转数字）
    - 移除默认值字段（false 的布尔值、0 的座位号等）
    - compressMessage/decompressMessage 通用压缩/解压函数
    - compressPlayerList 玩家列表数组格式优化
    - computeDelta/applyDelta 增量更新支持
    - CompressionMaps 导出供前端使用
  - 后端 match_handler.ts 修改：
    - 添加 ENABLE_COMPRESSION 开关
    - broadcastMessage/sendToPlayer 自动压缩消息
  - 前端创建 message-decompress.ts 解压模块（~320行）：
    - 字段名反向映射（30+ 字段）
    - 枚举值解压（数字转字符串，兼容前后端类型差异）
    - isCompressedMessage 自动检测是否压缩
    - safeDecompress 异常处理
    - applyDelta 增量更新支持
  - 前端 useNakama.ts handleMatchData 添加解压逻辑
  - 测试全部通过（104个），编译通过（后端 99.1kb）
  - 预估压缩效果：消息体积减少 30-50%
- **2026-01-02**: 实现防作弊系统（完整实现），包含:
  - 创建 anti-cheat.ts 防作弊模块（~550行）：
    - 速率限制系统：chat（5次/3秒）、action（3次/秒）、state（2次/秒）配置
    - 支持渐进式封禁（重复违规加长封禁时间）
    - checkRateLimit/clearRateLimitState 速率检查/清理函数
    - getOpCodeCategory 消息类型分类
  - 操作验证器：
    - validateVote：投票阶段、玩家状态、目标有效性验证
    - validateSkill：技能阶段、角色权限、目标有效性验证（狼人不能互刀、守卫不能连守等）
    - validateChat：消息长度、频道权限验证
  - 异常行为检测：
    - recordAction/getSuspicionScore：记录操作并计算嫌疑分数
    - 检测极速操作（<100ms）、重复操作模式、可疑行为
    - 嫌疑分数达到 80 分自动封禁
  - 会话管理：
    - registerSession/updateSessionActivity：会话注册和活动更新
    - checkSessionAnomalies：检测频繁重连等异常
  - 数据验证：
    - sanitizeMessageData：消息数据清理和验证
    - validateTimestamp：时间戳有效性验证
  - 综合检查：
    - checkAction：组合速率限制、异常检测、会话检查
    - cleanupPlayer：玩家离开时清理所有追踪数据
  - 集成到 match_handler.ts：
    - processMessage 添加防作弊检查（失败返回 MATCH_ERROR）
    - matchJoin 添加 registerSession 会话注册
    - matchLeave 添加 cleanupPlayer 清理逻辑
  - 创建 anti-cheat.test.ts 测试文件（~500行，51个测试用例）：
    - Rate Limiting (5)、OpCode Category (3)、Vote Validation (6)
    - Skill Validation (7)、Chat Validation (6)、Anomaly Detection (4)
    - Session Tracking (3)、Data Sanitization (6)、Timestamp Validation (4)
    - Combined Anti-Cheat (3)、Edge Cases (4)
  - 测试全部通过（155个 = 104个原有 + 51个新增），编译通过（后端 110.1kb）
- **2026-01-02**: 实现日志和监控系统（完整实现），包含:
  - 创建 logger.ts 日志模块（~320行）：
    - LogLevel 枚举（DEBUG/INFO/WARN/ERROR/FATAL 5级）
    - LogCategory 枚举（SYSTEM/MATCH/PLAYER/GAME/SKILL/VOTE/CHAT/SECURITY/PERFORMANCE/NETWORK 10类）
    - LogEntry 接口（时间戳、级别、分类、消息、matchId、playerId、数据、错误）
    - 可配置日志（minLevel/includeStack/prettyPrint/enableConsole）
    - JSON 格式输出，便于日志分析
    - createMatchLogger/createPlayerLogger 作用域日志工厂
    - startTimer 计时器辅助函数
    - logWithTiming/logWithTimingAsync 带计时的日志
    - BatchLogger 批量日志类（缓冲区、定时刷新）
  - 创建 metrics.ts 性能指标模块（~450行）：
    - MetricType 枚举（COUNTER/GAUGE/HISTOGRAM/SUMMARY）
    - MetricsStore 类：存储计数器、仪表、直方图、摘要
    - Prometheus 兼容格式导出（exportPrometheus）
    - 预定义 MetricNames（20+ 指标）：
      - 比赛指标：创建/活跃/完成/时长
      - 玩家指标：连接/加入/离开/重连
      - 游戏指标：胜利阵营/回合数/阶段时长
      - 技能/投票/消息指标
      - 错误/防作弊指标
      - 性能指标：循环耗时/广播耗时
    - createMatchMetrics 比赛级别指标工厂
    - timeFunction/timeFunctionAsync 函数计时辅助
    - getMetricsSummary 系统指标摘要
  - 创建 game-events.ts 游戏事件日志模块（~550行）：
    - GameEventType 枚举（30+ 事件类型）：
      - 比赛生命周期：创建/开始/结束/取消
      - 玩家事件：加入/离开/重连/准备/死亡
      - 阶段事件：阶段变化/夜晚开始/白天开始/投票
      - 技能事件：狼人击杀/预言家查验/女巫用药/守卫守护/猎人开枪
      - 投票/警长/特殊事件
      - 安全事件：速率限制/作弊检测/无效操作
    - GameEventData 接口（事件数据结构）
    - GameEventBuffer 事件缓冲区（批量处理）
    - createGameEventLogger 工厂函数（40+ 事件记录方法）
  - 集成到 match_handler.ts：
    - matchInit 记录比赛初始化和配置，增加 metrics
    - matchLoop 记录消息处理时间和循环耗时
    - matchTerminate 清理日志器和指标
    - endGame 记录游戏结束详情和统计
  - 测试全部通过（155个），编译通过（后端 125.4kb）
- **2026-01-02**: 创建 README.md 项目说明文档（完整实现），包含:
  - 项目概述和功能特性（核心玩法、技术特性、UI/UX）
  - 10 种角色及能力说明（Villager/Seer/Witch/Hunter/Guard/Idiot/Werewolf/Alpha Wolf/Cupid）
  - 技术栈说明（Nakama 3.21.1/React 18/Vite 5/Tailwind CSS/Zustand/Framer Motion）
  - 快速开始指南（3 步：后端 Docker、前端 Dev Server、访问地址）
  - 完整项目结构说明（后端 nakama/、前端 client/）
  - 游戏规则（胜利条件、游戏流程、角色配置表）
  - 开发命令（后端 build/typecheck/test，前端 dev/build/preview）
  - 155 个测试用例覆盖说明
  - 键盘快捷键表（11 个快捷键）
  - API 端点列表（8 个 RPC）
  - 部署文档链接
  - 环境变量配置（后端 3 个、前端 3 个）
  - 贡献指南和 MIT 许可证
- **2026-01-02**: 实现邮箱注册/登录功能（完整实现），包含:
  - 前端 useNakama.ts 添加邮箱认证方法:
    - authenticateWithEmail：邮箱登录（支持 remember 参数）
    - registerWithEmail：邮箱注册（用户名 + 邮箱 + 密码）
    - linkEmail：账户升级（游客账户绑定邮箱）
    - saveSession/loadSession/clearSession 会话持久化（localStorage）
    - 新增状态：isAuthenticated、authMethod、userEmail
  - 创建 AuthPanel.tsx 登录/注册 UI 组件（~550行）:
    - InputField 表单输入组件
    - Button 按钮组件（带 loading 状态）
    - LoginForm 登录表单（邮箱/密码/记住我）
    - RegisterForm 注册表单（邮箱/密码/确认密码/用户名）
    - GuestForm 游客快速开始（输入昵称）
    - AuthPanel 主组件（标签页切换：登录/注册/游客）
    - LinkEmailModal 账户升级弹窗
    - UserInfo 已登录用户信息显示
  - 更新 Lobby.tsx 集成认证流程:
    - 添加认证相关 props（isAuthenticated/authMethod/userEmail/onLogin/onRegister/onGuestLogin/onLinkEmail/onLogout）
    - useAuthFlow 标志判断显示认证面板或用户信息
    - 认证后显示 UserInfo + "进入游戏大厅"按钮
    - 未认证显示 AuthPanel
    - 游客账户支持绑定邮箱升级
  - 更新 App.tsx 传递认证相关回调到 Lobby
  - 会话恢复：应用启动时自动恢复已保存的会话
  - TypeScript 编译通过
- **2026-01-02**: 实现排行榜功能（完整实现），包含:
  - 后端 main.ts 已有 rpcGetLeaderboard RPC (~240行) 支持:
    - level/wins/winRate/winStreak 四种排行类型
    - 分页支持（limit/offset）
    - 当前用户排名（即使不在当前页）
    - 从 Nakama storage 读取用户统计
  - 前端 werewolf.ts 添加类型:
    - LeaderboardType 类型（'level' | 'wins' | 'winRate' | 'winStreak'）
    - LeaderboardEntry 接口（排名、用户信息、统计值）
    - LEADERBOARD_TYPE_NAMES/ICONS/DESCRIPTIONS 常量
  - 前端 useNakama.ts 添加 getLeaderboard 函数
  - 创建 LeaderboardPanel.tsx (~450行) 完整排行榜 UI:
    - LeaderboardTypeTab：4种排行类型切换标签
    - LeaderboardEntryRow：玩家条目（奖牌图标、等级称号、统计值）
    - MyRankCard：当前用户排名卡片
    - LeaderboardButton：触发按钮
    - CompactLeaderboard：紧凑版侧边栏显示
    - 分页、加载骨架屏、空状态
  - 更新 Lobby.tsx 集成排行榜按钮和弹窗
  - 更新 App.tsx 传递 onGetLeaderboard/currentUserId props
  - TypeScript 编译通过
- **2026-01-02**: 添加国际化支持（i18n）（完整实现），包含:
  - i18n/index.ts 配置文件：i18next + react-i18next + browser-languagedetector
  - i18n/locales/zh.ts 中文翻译（~500行）：覆盖通用/品牌/认证/大厅/房间列表/游戏/阶段/角色/阵营/投票/聊天/遗言/警长/结果/统计/成就/排行榜/邀请/观战/设置/快捷键/PWA/死亡原因/夜晚行动/过渡动画
  - i18n/locales/en.ts 英文翻译（~500行）：与中文完全对应
  - 创建 LanguageSelector.tsx（~250行）：
    - LanguageSelector：下拉式语言选择器
    - LanguageButton：紧凑版语言按钮
    - LanguageToggle：简单切换按钮
    - LanguageSettingsPanel：设置面板
  - 更新 main.tsx 导入 i18n 配置
  - 更新 Lobby.tsx 集成国际化：
    - 顶部栏添加 LanguageButton
    - 使用 useTranslation hook
    - 替换所有硬编码文本为 t() 函数调用
    - 游戏规则、阵营、角色描述支持多语言
  - 支持浏览器语言自动检测
  - localStorage 持久化语言选择
  - TypeScript 编译通过
- **2026-01-02**: 将 i18n 集成到 RoomList 和 ChatPanel 组件（完整实现），包含:
  - RoomList.tsx 添加 i18n 支持（~50处文本替换）：
    - RoomCard 房间状态文本（等待中/游戏中/已满）
    - QuickMatchCard 快速匹配按钮文本
    - EmptyRoomList 空列表提示
    - FilterBar 筛选器标签和按钮
    - PasswordEntryModal 密码输入弹窗
    - CreateRoomModal 创建房间表单（标题/人数/角色配置/密码/按钮）
    - 导航栏和页面标题
  - ChatPanel.tsx 添加 i18n 支持（~20处文本替换）：
    - 频道标签（公共/狼人/观战）
    - 输入框占位符（发送消息/禁言状态）
    - 发送按钮
    - 状态提示（夜晚禁言/狼人频道提示/观战模式提示）
    - 空消息列表提示
  - 所有组件添加 useTranslation hook
  - TypeScript 编译通过（npm run typecheck 验证）
- **2026-01-02**: 实现游戏回放功能（完整实现），包含:
  - 后端 replay.ts 回放存储模块（~400行）：
    - ReplayBuffer 类：游戏中收集事件的内存缓冲区
    - ReplayPlayer/ReplayConfig/ReplayMeta/ReplayEvent 类型定义
    - saveReplay：保存回放到 Nakama storage
    - getUserReplays：获取用户回放列表（最多50条）
    - getReplay/getReplayByOwner：获取单个回放详情
    - getKeyEvents/getReplayStats：回放统计和关键事件
  - 后端 main.ts 添加 RPC 端点：
    - get_replays：获取用户回放列表（分页支持）
    - get_replay：获取单个回放详情（含统计和关键事件）
  - 后端 match_handler.ts 集成：
    - startGame 初始化 ReplayBuffer（配置和玩家信息）
    - endGame 保存回放并清理缓冲区
    - 记录游戏开始、游戏结束等关键事件
  - 前端 werewolf.ts 添加回放类型：
    - GameEventType 枚举（30+事件类型）
    - ReplayPlayer/ReplayConfig/ReplayMeta/ReplayEvent 接口
    - GameReplay/ReplayListItem/ReplayStats 接口
  - 前端 useNakama.ts 添加回放 API：
    - getReplays：获取回放列表
    - getReplayById：获取回放详情
  - 创建 ReplayPanel.tsx 组件（~700行）：
    - ReplayListItemCard：回放列表项（日期、人数、胜负、角色）
    - StatsPanel：统计面板（击杀、出局、救人、技能使用）
    - PlayerList：玩家列表（按阵营分组显示角色）
    - EventTimeline：事件时间线（可点击跳转）
    - PlaybackControls：播放控制（播放/暂停、进度条、倍速）
    - ReplayDetailView：回放详情视图
    - ReplayButton/ReplayModal：触发按钮和弹窗
  - 编译通过（后端 189.8kb，前端 typecheck 通过）
- **2026-01-02**: 扩展 i18n 到 UserStats 组件（完整实现），包含:
  - 更新 zh.ts 添加更多统计翻译键：myStats、noGames、startPlaying、levelInfo、overallStats、survivalRate、sheriffGames、neverElected、maxWinStreak、firstGame、lastGame、games、winsCount、villagerFaction、werewolfFaction、loversFaction、sheriffWinRate
  - 更新 en.ts 添加对应英文翻译
  - UserStats.tsx 组件全面 i18n 化：
    - 添加 ROLE_TRANSLATION_KEYS 角色名称到翻译键映射
    - RoleStatItem 使用 useTranslation 获取角色名称
    - CompactStats 使用翻译键显示战绩信息
    - StatsPanel 使用翻译键显示所有标签（等级信息、总体统计、阵营统计、角色统计）
    - 日期格式根据语言自动切换（zh-CN/en-US）
    - StatsModal 标题使用翻译
    - StatsButton 文本使用翻译
  - TypeScript 编译通过
- **2026-01-02**: 扩展 i18n 到 SheriffPanel 组件（完整实现），包含:
  - 更新 zh.ts 添加警长系统翻译键：campaign.title/description、speech.title/progress/speaking/speakingNow、voting.title/selectCandidate/voted/waitingForOthers/votes、transfer.transferring/selectingSuccessor/description/confirm/destroy、election.autoElected/congratulations/gotPosition/waitingResult/noSheriff、currentFormat/weightShort/dead/seatFormat
  - 更新 en.ts 添加对应英文翻译
  - SheriffPanel.tsx 组件全面 i18n 化：
    - CandidateCard 组件使用 t() 翻译发言状态和票数
    - SheriffBadge 组件使用 t() 翻译座位格式和票数权重
    - CampaignPhase 组件使用 t() 翻译竞选标题、描述、按钮和候选人列表
    - SpeechPhase 组件使用 t() 翻译发言标题、进度和当前发言者
    - VotingPhase 组件使用 t() 翻译投票标题、提示和确认按钮
    - TransferPhase 组件使用 t() 翻译移交标题、描述和按钮
    - ElectionResult 组件使用 t() 翻译选举结果（平票/当选/等待）
    - SheriffStatusBar 组件使用 t() 翻译警长状态栏
  - TypeScript 编译通过
- **2026-01-02**: 扩展 i18n 到 AchievementPanel 组件（完整实现），包含:
  - 更新 zh.ts 添加成就系统完整翻译键：title/unlocked/locked/progress/completion/xpReward/noAchievements/loadError/achievementUnlocked/recentUnlock/achievementProgress
  - 添加 achievements.categories 11类分类翻译（beginner/games/wins/streak/roleMaster/skill/sheriff/special/survival/level/social）
  - 添加 achievements.rarity 5级稀有度翻译（common/uncommon/rare/epic/legendary）
  - 更新 en.ts 添加对应英文翻译
  - AchievementPanel.tsx 组件全面 i18n 化：
    - 添加 CATEGORY_TRANSLATION_KEYS 分类到翻译键映射
    - 添加 RARITY_TRANSLATION_KEYS 稀有度到翻译键映射
    - AchievementCard 组件使用 t() 翻译进度和解锁状态、日期根据语言切换格式
    - CategoryTab 组件使用 t() 翻译分类名称
    - AchievementStats 组件使用 t() 翻译统计标签
    - AchievementPanel 组件使用 t() 翻译空列表提示
    - AchievementModal 组件使用 t() 翻译标题和加载错误
    - AchievementButton 组件使用 t() 翻译按钮文本
    - AchievementUnlockNotification 组件使用 t() 翻译解锁提示
    - CompactAchievementDisplay 组件使用 t() 翻译进度标签和最近解锁
  - TypeScript 编译通过
- **2026-01-02**: 扩展 i18n 到 LevelDisplay 组件（完整实现），包含:
  - 更新 zh.ts 添加 level 命名空间翻译键：
    - levelUp（升级!）、maxLevel（满级）、xp（经验值）、xpToNext（距离下一级还需 {{xp}} XP）
    - tier（段位）、tierFormat（{{tier}}段位）、lvFormat（Lv.{{level}}）
    - xpRewards（经验值获取方式）
    - rewards.* 6项（gameCompleted/gameWon/survived/sheriffElected/firstWinOfDay/winStreakBonus）
    - winStreak（{{count}} 连胜）、currentRecord（历史最高）
  - 更新 en.ts 添加对应英文翻译
  - LevelDisplay.tsx 组件全面 i18n 化：
    - LevelProgress 组件使用 t() 翻译 MAX/XP 标签和进度提示
    - LevelCard 组件使用 t() 翻译段位格式、等级格式、经验值获取方式标题和6项奖励名称
    - LevelUpAnimation 组件使用 t() 翻译升级文字
    - WinStreakBadge 组件使用 t() 翻译连胜文字
  - TypeScript 编译通过
- **2026-01-02**: 扩展 i18n 到 AuthPanel 组件（完整实现），包含:
  - 更新 zh.ts auth 命名空间添加翻译键：
    - processing（处理中...）、loginNow（立即登录）、registerNow（立即注册）
    - startGame（开始游戏）、saveStats（想要保存战绩？）
    - guestNickname（游戏昵称（可选））、leaveEmptyRandom（留空将随机生成）
    - usernameChars（2-12个字符）、passwordChars（至少6个字符）
    - reenterPassword（再次输入密码）、setPassword（设置密码）
    - guestStart（游客快速开始）、bindEmailTitle/bindEmailSuccess/bindEmailDesc
    - tab.login/tab.register/tab.guest（标签页文本）
  - 更新 en.ts 添加对应英文翻译
  - AuthPanel.tsx 组件全面 i18n 化：
    - 添加 useTranslation 导入
    - Button 组件增加 loadingText 属性支持加载状态国际化
    - LoginForm 使用 t() 翻译邮箱/密码/记住登录/登录按钮/分隔符/游客开始/注册提示
    - RegisterForm 使用 t() 翻译用户名/邮箱/密码/确认密码/注册按钮/登录提示
    - GuestForm 使用 t() 翻译昵称/开始游戏按钮/注册提示
    - AuthPanel 主组件使用 t() 翻译三个标签页
    - LinkEmailModal 使用 t() 翻译标题/描述/成功提示/取消按钮/绑定按钮
    - UserInfo 使用 t() 翻译游客账号/绑定邮箱/退出登录
  - TypeScript 编译通过

## 当前阻塞问题

（暂无）

## 下一步建议

1. ~~创建 docker-compose.yml（Nakama + CockroachDB）~~ ✓
2. ~~创建 Nakama TypeScript 项目结构~~ ✓
3. ~~定义类型系统（角色、阶段、玩家状态）~~ ✓
4. ~~实现 match_handler.ts 基础框架~~ ✓
5. ~~实现角色分配逻辑~~ ✓
6. ~~实现夜晚阶段流程~~ ✓
7. ~~实现白天阶段和投票~~ ✓
8. ~~创建 React + Vite 前端项目结构~~ ✓
9. ~~实现 Nakama 客户端连接（useNakama hook）~~ ✓
10. ~~实现 PlayerSeat 组件（座位显示）~~ ✓
11. ~~实现 GameRoom 组件（圆桌布局）~~ ✓
12. ~~实现 ChatPanel 组件（聊天面板）~~ ✓
13. ~~实现猎人/狼王死亡带人技能~~ ✓
14. ~~实现前端死亡技能 UI（开枪选择界面）~~ ✓
15. ~~实现 VotingPanel 组件（独立投票界面）~~ ✓
16. ~~实现 RoleCard 组件（角色卡牌、翻牌动画）~~ ✓
17. ~~实现 NightOverlay 组件（夜晚技能选择界面）~~ ✓
18. ~~实现 ResultModal 组件（游戏结果弹窗）~~ ✓
19. ~~实现 DayTimer 组件（发言计时器）~~ ✓
20. ~~实现白痴角色（投票出局免死一次）~~ ✓
21. ~~实现情侣系统（丘比特角色）~~ ✓
22. ~~添加游戏音效系统~~ ✓
23. ~~实现警长竞选系统（前后端完整实现）~~ ✓
24. ~~实现遗言系统~~ ✓
25. ~~实现大厅页面和房间列表 UI~~ ✓
26. ~~实现死亡玩家观战聊天频道~~ ✓
27. ~~添加夜晚/白天过渡动画~~ ✓
28. ~~添加投票动画~~ ✓
29. ~~添加死亡特效~~ ✓
30. ~~实现房间设置（人数、角色配置）~~ ✓
31. ~~实现表情包系统~~ ✓
32. ~~移动端响应式适配~~ ✓
33. ~~实现快捷语音（预设语音消息）~~ ✓
34. ~~添加键盘快捷键支持~~ ✓
35. ~~添加 PWA 支持~~ ✓
36. ~~添加主题切换（暗黑/光明）~~ ✓
37. ~~实现观战模式~~ ✓
38. ~~实现私密房间（密码保护）~~ ✓
39. ~~添加用户统计（胜率、场次）~~ ✓
40. ~~添加角色技能单元测试~~ ✓
41. ~~添加投票逻辑单元测试~~ ✓
42. ~~添加游戏流程集成测试~~ ✓
43. ~~编写部署文档~~ ✓
44. ~~实现好友邀请功能~~ ✓
45. ~~性能优化（减少网络传输）~~ ✓
46. ~~添加防作弊措施~~ ✓
47. ~~添加日志和监控~~ ✓
48. ~~创建 README.md 项目说明文档~~ ✓
49. ~~实现用户注册/登录（邮箱认证）~~ ✓
50. ~~实现等级系统~~ ✓
51. ~~实现成就系统~~ ✓
52. ~~实现排行榜~~ ✓
53. ~~添加国际化支持（i18n）~~ ✓
54. ~~添加游戏回放功能~~ ✓
55. ~~将 i18n 集成到其他组件（RoomList, GameRoom, ChatPanel 等）~~ ✓
56. ~~扩展 i18n 到 UserStats 组件~~ ✓
57. ~~扩展 i18n 到 SheriffPanel 组件~~ ✓
58. ~~扩展 i18n 到 AchievementPanel 组件~~ ✓
59. ~~扩展 i18n 到 LevelDisplay 组件~~ ✓
60. ~~扩展 i18n 到 InvitePanel 组件~~ ✓
61. ~~扩展 i18n 到 LeaderboardPanel 组件~~ ✓
62. ~~扩展 i18n 到 ReplayPanel 组件~~ ✓
63. ~~扩展 i18n 到 AuthPanel 组件~~ ✓
64. 扩展 i18n 到 DayTimer 组件
65. 扩展 i18n 到 QuickPhrases 组件
66. 扩展 i18n 到 EmojiPicker 组件

---
*创建时间: 2025-12-30*
