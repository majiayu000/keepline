# 项目上下文

## 项目概述

开发一个基于 Nakama 游戏服务器的在线多人德州扑克（Texas Hold'em）游戏。

**核心功能：**
1. 多人实时在线对战（最多 9 人一桌）
2. 完整的德州扑克规则实现
3. 实时游戏状态同步
4. 玩家匹配系统
5. 筹码和下注系统
6. 聊天功能

## 技术栈

### 后端 - Nakama 游戏服务器
- **Nakama Server**: 开源实时多人游戏服务器
- **运行时语言**: TypeScript（推荐，类型安全）
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
- **状态管理**: Zustand 或 Context API

## Nakama 核心概念

### 权威服务器模式（Authoritative Multiplayer）
- 所有游戏逻辑在服务器端执行和验证
- 客户端发送操作，服务器广播结果
- 防止作弊，确保公平性

### Match Handler（匹配处理器）
关键函数：
- `matchInit`: 初始化匹配状态
- `matchJoinAttempt`: 验证玩家加入请求
- `matchJoin`: 玩家成功加入后执行
- `matchLeave`: 玩家离开时执行
- `matchLoop`: 游戏主循环（每 tick 执行）
- `matchSignal`: 处理外部信号
- `matchTerminate`: 匹配结束时清理

### 匹配状态（Match State）
- 服务器为每个匹配分配的内存区域
- 存储所有游戏数据（牌组、玩家手牌、筹码、下注等）
- 每个 tick 可以更新状态

## 德州扑克规则

### 游戏流程
1. **发牌**: 每人 2 张底牌
2. **第一轮下注** (Pre-flop)
3. **翻牌** (Flop): 发 3 张公共牌
4. **第二轮下注**
5. **转牌** (Turn): 发第 4 张公共牌
6. **第三轮下注**
7. **河牌** (River): 发第 5 张公共牌
8. **最后一轮下注**
9. **摊牌** (Showdown): 比较牌型，决定胜者

### 玩家操作
- **弃牌 (Fold)**: 放弃本轮
- **过牌 (Check)**: 不加注，传递
- **跟注 (Call)**: 跟进当前最大注
- **加注 (Raise)**: 提高下注额
- **全押 (All-in)**: 押上所有筹码

### 牌型排名（从低到高）
1. 高牌 (High Card)
2. 一对 (One Pair)
3. 两对 (Two Pair)
4. 三条 (Three of a Kind)
5. 顺子 (Straight)
6. 同花 (Flush)
7. 葫芦 (Full House)
8. 四条 (Four of a Kind)
9. 同花顺 (Straight Flush)
10. 皇家同花顺 (Royal Flush)

## 项目结构

```
workspace/
├── docker-compose.yml      # Docker 配置
├── nakama/                 # Nakama 服务器代码
│   ├── package.json
│   ├── tsconfig.json
│   ├── src/
│   │   ├── main.ts         # 入口文件，注册 match handler
│   │   ├── poker/
│   │   │   ├── match_handler.ts  # 匹配处理器
│   │   │   ├── game_state.ts     # 游戏状态管理
│   │   │   ├── deck.ts           # 牌组逻辑
│   │   │   ├── hand_evaluator.ts # 牌型评估
│   │   │   ├── betting.ts        # 下注逻辑
│   │   │   └── types.ts          # 类型定义
│   │   └── rpc/
│   │       └── find_match.ts     # 匹配查找 RPC
│   └── build/                    # 编译输出
├── client/                 # React 前端
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── App.tsx
│   │   ├── components/
│   │   │   ├── Table.tsx         # 牌桌组件
│   │   │   ├── Card.tsx          # 扑克牌组件
│   │   │   ├── Player.tsx        # 玩家组件
│   │   │   ├── BettingControls.tsx # 下注控制
│   │   │   └── Chat.tsx          # 聊天组件
│   │   ├── hooks/
│   │   │   └── useNakama.ts      # Nakama 连接 hook
│   │   ├── store/
│   │   │   └── gameStore.ts      # 游戏状态
│   │   └── types/
│   │       └── poker.ts          # 前端类型
│   └── public/
│       └── cards/                # 扑克牌图片
└── README.md

```

## 开发环境设置

### 启动 Nakama 服务器
```bash
# 编译 TypeScript
cd nakama && npm install && npx tsc

# 启动 Docker（Nakama + CockroachDB）
docker-compose up --build
```

### 启动前端开发服务器
```bash
cd client && npm install && npm run dev
```

### Nakama 控制台
访问 http://localhost:7351 查看服务器状态

## 参考资源

- Nakama 官方文档: https://heroiclabs.com/docs/nakama/
- Nakama 项目模板: https://github.com/heroiclabs/nakama-project-template
- Nakama Match Handler API: https://heroiclabs.com/docs/nakama/server-framework/typescript-runtime/function-reference/match-handler/
- 德州扑克规则参考: node-poker-stack, React-Poker 等开源项目

## 最近完成的工作

1. **项目基础设施搭建** (2025-12-29)
   - 创建 docker-compose.yml（Nakama 3.21.1 + CockroachDB）
   - 创建 nakama-config.yml 服务器配置
   - 初始化 Nakama TypeScript 项目结构

2. **核心服务器代码** (2025-12-29)
   - 创建类型定义 (`types.ts`)：牌型、玩家状态、游戏阶段、消息协议
   - 实现 match handler (`match_handler.ts`)：matchInit, matchJoinAttempt, matchJoin, matchLeave, matchLoop
   - 实现 RPC 函数 (`find_match.ts`)：查找匹配、创建私有房间

3. **React 前端项目** (2025-12-29)
   - 创建 React + Vite + TypeScript + Tailwind CSS 项目结构
   - 实现 `useNakama.ts` hook：设备认证、WebSocket 连接、匹配加入/离开、发送操作
   - 实现 `gameStore.ts`（Zustand）：连接状态、用户信息、游戏状态管理
   - 实现 `poker.ts` 类型定义：Card、Player、GameState、PlayerAction 等
   - 创建基础 UI 组件：
     - `App.tsx`：连接流程、大厅界面
     - `Table.tsx`：椭圆形牌桌布局（9 个玩家位置）
     - `Card.tsx`：扑克牌显示（正面/背面）
     - `Player.tsx`：玩家信息显示（筹码、状态、手牌）
     - `BettingControls.tsx`：下注控制（fold/check/call/raise/all-in）
     - `Chat.tsx`：聊天组件（待集成）

4. **Deck 牌组模块** (2025-12-29)
   - 实现 `deck.ts`：52 张标准扑克牌创建
   - Fisher-Yates 洗牌算法
   - 发牌函数（单张、多张）
   - Deck 类（OOP 风格 API）
   - 辅助函数：cardToString, cardsToString

5. **游戏状态管理和下注系统** (2025-12-29)
   - 实现 `game_state.ts`：游戏状态管理模块
     - 玩家管理函数（getActivePlayers, getActingPlayers, getEligiblePlayers）
     - 位置追踪（getNextPlayerSeat, getPlayerBySeat）
     - 盲注系统（moveDealerButton, assignBlinds, postBlinds）
     - 发牌逻辑（dealHoleCards, dealCommunityCards）
     - 下注轮管理（startBettingRound, isBettingRoundComplete）
     - 游戏流程（startNewHand, advancePhase, initNewHand）
     - 边池计算（calculateSidePots, getTotalPot）
   - 实现 `betting.ts`：下注逻辑模块
     - 玩家操作执行（executePlayerAction）
     - 所有操作实现（fold, check, call, bet, raise, all-in）
     - 操作验证和金额计算
     - 辅助函数（getCallAmount, getActionInfo, getBetLimits）
   - 更新 `match_handler.ts`：集成新模块
     - 完整的 matchLoop 实现
     - 玩家操作消息处理
     - 游戏流程控制（阶段转换、下注轮结束判断）
     - 超时自动弃牌
     - 聊天消息广播

6. **手牌评估器** (2025-12-29)
   - 实现 `hand_evaluator.ts`：完整的德州扑克手牌评估模块
     - `evaluateHand(cards)`：评估 5-7 张牌，返回最佳 5 张
     - `compareEvaluatedHands()`：比较两手牌，返回胜负
     - `determineWinners(hands)`：从多手牌中确定赢家（支持平局）
     - `getHandStrength()`：获取手牌强度数值
   - 支持所有 10 种牌型识别：高牌、一对、两对、三条、顺子、同花、葫芦、四条、同花顺、皇家同花顺
   - 支持 A-2-3-4-5 wheel 顺子特殊处理
   - 完整的 kicker 比较逻辑
   - 集成到 `match_handler.ts` 的 Showdown 阶段：
     - `executeShowdown()`：执行摊牌逻辑
     - 边池分配给各自的赢家
     - 平局时自动平分奖池
     - 重置玩家状态准备下一局

7. **前端游戏状态接收和 UI 更新** (2025-12-29)
   - 更新 `poker.ts` 类型定义：
     - 添加 OpCode 常量（匹配服务器端）
     - 添加所有 Server*Data 接口（服务器消息格式）
     - 添加 TurnInfo 接口（回合信息）
     - 添加 RankNumber/RankDisplay 类型和转换函数
     - 添加 HAND_RANK_DISPLAY 常量
   - 更新 `gameStore.ts`：
     - 添加 turnInfo、showdownPlayers、winners 状态
     - 实现 updateGameStateFromServer()：转换服务器格式到客户端格式
     - 实现盲注位置计算、玩家轮换显示
   - 更新 `useNakama.ts`：
     - 实现 handleMatchData() 基于 OpCode 分发消息
     - 处理所有服务器消息类型（GAME_STATE、HOLE_CARDS、PLAYER_TURN 等）
     - 添加 sendChat() 聊天功能
   - 更新 UI 组件：
     - `Card.tsx`：支持数字 rank 转换显示
     - `Player.tsx`：显示 lastAction、改进状态显示
     - `BettingControls.tsx`：使用 turnInfo 控制按钮、添加倒计时、快捷下注按钮
     - `Table.tsx`：显示 winner 公告、showdown 手牌、旋转玩家位置

8. **单元测试框架和手牌评估器测试** (2025-12-29)
   - 添加 vitest 测试框架到 nakama 项目
   - 创建 `hand_evaluator.test.ts`：41 个测试用例
     - 牌型识别测试：所有 10 种牌型（高牌到皇家同花顺）
     - 特殊情况：wheel 顺子（A-2-3-4-5）、Broadway 顺子
     - 7 张牌选最佳 5 张测试
     - 手牌比较测试（同类型 kicker 比较）
     - 多人赢家确定（包括平局）
     - 真实游戏场景测试（backdoor flush、counterfeited two pair）
     - 边缘情况测试（错误处理、非顺子识别）

9. **Betting 模块单元测试** (2025-12-29)
   - 创建 `betting.test.ts`：52 个测试用例
     - 验证逻辑测试：玩家不存在、不是自己回合、玩家状态检查、动作可用性
     - Fold 操作测试：正确弃牌、筹码不变
     - Check 操作测试：无注时 check、已跟注时 check
     - Call 操作测试：正常跟注、筹码不足时 all-in、底池更新
     - Bet 操作测试：正常下注、已有注时失败、金额验证、最低注验证
     - Raise 操作测试：正常加注、无注时失败、最低加注验证、minRaise 更新
     - All-In 操作测试：正常 all-in、无筹码失败、currentBet 更新、hasActed 重置
     - 辅助函数测试：autoFold、getCallAmount、getActionInfo
     - 复杂场景测试：pre-flop 加注/再加注、三人全压、翻牌后下注
     - 边缘情况测试：精确 all-in call、最低加注

10. **Game State 模块单元测试** (2025-12-29)
    - 创建 `game_state.test.ts`：93 个测试用例
      - 玩家筛选测试：getActivePlayers、getActingPlayers、getEligiblePlayers
      - 位置追踪测试：getNextPlayerSeat（跳过 folded/all-in）、getPlayerBySeat
      - 庄家按钮测试：moveDealerButton（首轮分配、轮转、环绕）
      - 盲注测试：assignBlinds（3+ 玩家、heads-up）、postBlinds（筹码不足处理）
      - 发牌测试：dealHoleCards（2 张/人、牌组减少）、dealCommunityCards（burn card）
      - 下注轮测试：startBettingRound（重置状态、首轮位置）、isBettingRoundComplete
      - 游戏流程测试：initNewHand、startNewHand、advancePhase（各阶段转换）
      - 定时器测试：moveToNextPlayer、hasPlayerTimedOut
      - 边池测试：calculateSidePots（多个 all-in 金额）、getTotalPot
      - 单赢家测试：endHandWithSingleWinner（奖池分配、状态重置）
      - 可用操作测试：getAvailableActions（各种场景）、getBetLimits
      - 复杂场景测试：完整 pre-flop、all-in showdown、heads-up、多轮追踪

11. **Deck 模块单元测试** (2025-12-29)
    - 创建 `deck.test.ts`：78 个测试用例
      - createDeck 测试：52 张牌、4 种花色、13 种点数、无重复
      - shuffleDeck 测试：数量不变、原有牌保留、随机性验证、边缘情况
      - dealCards 测试：指定数量发牌、牌组减少、错误处理
      - dealCard 测试：单张发牌、顺序验证、空牌组错误
      - getRemainingCount 测试：准确计数
      - createShuffledDeck 测试：52 张洗好的牌
      - cardToString/cardsToString 测试：正确格式化（花色符号、点数字符）
      - Deck 类测试：构造函数、shuffle()、reset()、deal()、dealOne()、remaining、getCards()、toString()、方法链
      - 集成测试：完整德州扑克发牌流程（4 人/9 人）、新局 reset

12. **边池 UI 展示** (2025-12-29)
    - 创建 `SidePots.tsx` 组件：
      - 单边池时显示简洁的 "Pot: $XXX" 格式
      - 多边池时展示详细视图：总金额 + 每个边池分行显示
      - Main Pot 使用金色高亮，Side Pot 使用蓝色区分
      - 悬停显示 tooltip 展示参与该边池的玩家名称
      - 处理 pots 为空或 undefined 的边缘情况
    - 更新 `Table.tsx`：集成 SidePots 组件替换原来的简单 pot 显示
    - 更新 `gameStore.ts`：增强 pots 数据的防御性处理

13. **玩家断线重连逻辑** (2025-12-29)
    - **服务器端改进 (`types.ts`, `match_handler.ts`)**:
      - 为 Player 添加 `isConnected` 和 `disconnectedAt` 字段
      - 添加 `DISCONNECT_GRACE_SECONDS` (60秒) 断线保护时间
      - 添加 `OpCode.PLAYER_DISCONNECTED` 和 `OpCode.PLAYER_RECONNECTED` 消息类型
      - `matchLeave`: 断线时不立即移除玩家，标记为断线状态并广播
      - `matchJoin`: 重连时恢复状态，发送游戏状态和手牌，通知其他玩家
      - `matchLoop`: 添加 `handleDisconnectedPlayers()` 检查断线超时
      - 断线玩家超时后自动 fold（如果是当前行动者）并标记为 sitting_out
      - `getPublicPlayerInfo()` 包含 isConnected 状态
    - **前端改进 (`poker.ts`, `gameStore.ts`, `useNakama.ts`)**:
      - 添加 `reconnecting` 连接状态
      - 添加 `ServerPlayerDisconnectedData` 和 `ServerPlayerReconnectedData` 类型
      - 实现自动重连机制：指数退避策略，最多 5 次重试
      - 添加 `attemptReconnect()` 和 `cancelReconnect()` 函数
      - 添加 `setPlayerDisconnected()` 和 `setPlayerReconnected()` 状态管理
      - 处理 `PLAYER_DISCONNECTED` 和 `PLAYER_RECONNECTED` 消息
    - **UI 更新 (`Player.tsx`, `App.tsx`)**:
      - 断线玩家显示红色 "DISCONNECTED" 标签并降低透明度
      - 添加重连状态 UI：显示重试次数和取消按钮
      - 断线/重连事件在聊天中显示系统消息

14. **游戏房间列表功能** (2025-12-29)
    - **服务器端 (`find_match.ts`, `main.ts`)**:
      - 创建 `listRoomsRpc` RPC 函数，返回所有可用房间
      - 返回数据包含：房间名称、玩家数量、最大人数、盲注、游戏阶段
      - 房间按玩家数量（最多优先）和创建时间排序
      - 注册 `list_rooms` RPC 端点
    - **前端类型 (`poker.ts`)**:
      - 添加 `RoomInfo` 和 `ListRoomsResponse` 类型定义
    - **前端 Hook (`useNakama.ts`)**:
      - 添加 `listRooms()` 函数调用 `list_rooms` RPC
    - **房间列表组件 (`RoomList.tsx`)**:
      - 显示所有可用房间列表
      - 显示房间详情：名称、玩家数、盲注、游戏状态
      - "Quick Match" 快速匹配按钮
      - "Refresh" 刷新房间列表
      - "Join" 按钮加入指定房间
      - 满员房间显示为 "Full" 状态
    - **大厅界面更新 (`App.tsx`)**:
      - 连接后显示房间列表替换原来的简单 "Find a Table" 按钮
      - 自动获取房间列表
      - 集成 RoomList 组件

15. **观战模式** (2025-12-29)
    - **服务器端类型 (`types.ts`)**:
      - 添加 `Spectator` 接口（odid, displayName, joinedAt）
      - 添加 `maxSpectators` 到 `GameState`
      - 添加 `spectators: Map<string, Spectator>` 到 `GameState`
      - 添加观战相关 OpCode：SPECTATOR_JOINED, SPECTATOR_LEFT, SPECTATOR_TO_PLAYER, SPECTATOR_LIST, REQUEST_SEAT
    - **服务器端匹配处理 (`match_handler.ts`)**:
      - 更新 `matchJoinAttempt`：接受观战者加入（表满时自动成为观战者）
      - 更新 `matchJoin`：根据座位可用性决定加入为玩家或观战者
      - 更新 `matchLeave`：处理观战者离开
      - 添加 `handleRequestSeat()`：观战者请求成为玩家
      - 添加 `getSpectatorList()`：获取观战者列表
      - 更新 `getPublicGameState()`：包含观战者信息
      - 更新 `updateMatchLabel()`：包含观战者数量
    - **服务器端 RPC (`find_match.ts`)**:
      - 更新 `RoomInfo` 接口添加 `spectators` 字段
      - 更新 `listRoomsRpc` 返回观战者数量
    - **前端类型 (`poker.ts`)**:
      - 添加 `Spectator` 接口
      - 添加观战者相关消息接口：ServerSpectatorJoinedData, ServerSpectatorLeftData, ServerSpectatorToPlayerData, ServerSpectatorListData
      - 更新 RoomInfo 添加 spectators 字段
      - 更新 ServerGameStateData 添加 spectatorCount 和 spectators
    - **前端状态 (`gameStore.ts`)**:
      - 添加 `isSpectator` 状态
      - 添加 `spectators: Spectator[]` 状态
      - 添加 `setIsSpectator()`, `setSpectators()`, `addSpectator()`, `removeSpectator()` 方法
      - 更新 `updateGameStateFromServer()` 处理观战者列表
    - **前端 Hook (`useNakama.ts`)**:
      - 添加观战消息处理：SPECTATOR_JOINED, SPECTATOR_LEFT, SPECTATOR_TO_PLAYER, SPECTATOR_LIST
      - 添加 `requestSeat()` 函数供观战者请求成为玩家
    - **前端 UI 组件**:
      - 创建 `SpectatorBanner.tsx`：显示观战模式提示和"Join as Player"按钮
      - 创建 `SpectatorList.tsx`：显示观战者列表
      - 更新 `Table.tsx`：集成 SpectatorList 组件
      - 更新 `RoomList.tsx`：显示观战者数量，满员房间显示"Watch"按钮
      - 更新 `App.tsx`：集成 SpectatorBanner 组件

16. **游戏音效系统** (2025-12-30)
    - **音效 Hook (`useSoundEffects.ts`)**:
      - 使用 Web Audio API 合成音效（无需外部音频文件）
      - 12 种音效类型：deal, check, call, bet, fold, allIn, chips, win, turn, tick, join, leave
      - 可配置的音效参数：频率、持续时间、波形类型、音量
      - `playDealSound()` 支持卡牌索引变化音调
      - `playChipsSound()` 根据筹码数量调整音效强度
    - **Sound Context (`SoundContext.tsx`)**:
      - 全局音效状态管理（开关、音量）
      - localStorage 持久化用户偏好
      - Provider 包装应用提供全局访问
    - **Sound Settings 组件 (`SoundSettings.tsx`)**:
      - 紧凑模式（图标按钮 + 下拉菜单）
      - 完整模式（展开的设置面板）
      - 音量滑块和开关控制
    - **集成到游戏流程**:
      - `BettingControls.tsx`：操作按钮音效、回合提示音、倒计时滴答声
      - `Table.tsx`：社区牌发牌音效、赢家公告音效
      - `App.tsx`：初始化音频上下文（用户交互触发）

17. **卡牌发牌动画** (2025-12-30)
    - **Card 组件增强 (`Card.tsx`)**:
      - 新增 `animate` 属性：'deal' | 'flip' | 'highlight' | 'none'
      - 新增 `animationDelay` 属性：延迟动画开始
      - CSS @keyframes 动画：card-deal（滑入+旋转）、card-flip（翻转）、card-highlight（高亮闪烁）
      - 状态管理控制动画播放和清理
      - 悬停缩放效果
    - **Table 组件动画集成**:
      - `useNewCommunityCards` hook 追踪新发的社区牌
      - 自动为新发的牌触发 deal 动画
      - 动画延迟错开每张牌（150ms 间隔）

18. **筹码动画和胜利特效** (2025-12-30)
    - **筹码动画组件 (`ChipAnimation.tsx`)**:
      - 全局筹码飞行动画系统（无需 props drilling）
      - `triggerBetAnimation(seatIndex, amount, rotateOffset)`：下注时筹码飞向底池
      - `triggerWinAnimation(seatIndex, amount, rotateOffset)`：胜利时筹码飞向赢家
      - ChipStack 子组件：根据金额显示不同颜色的筹码堆叠
      - CSS keyframes 实现弧形飞行路径
      - 位置计算兼容玩家位置旋转
    - **胜利特效组件 (`VictoryEffect.tsx`)**:
      - 粒子系统：confetti（彩纸）、spark（火花）、star（星星）
      - 多次爆发效果，随机位置偏移
      - SVG 渲染，支持旋转和淡出
      - 发光滤镜效果（星星和火花）
      - `WinnerGlow` 组件：赢家玩家金色脉冲高亮
    - **集成到游戏流程**:
      - gameStore 添加 `lastBetAnimation` 状态和 `triggerBetAnimation` 方法
      - useNakama 在 `PLAYER_ACTED` 时触发下注动画
      - Table.tsx 集成 ChipAnimation 和 VictoryEffect
      - 赢家公告时同时触发胜利特效和筹码飞行动画

19. **聊天功能集成** (2025-12-30)
    - **Chat 组件增强 (`Chat.tsx`)**:
      - 可折叠/展开的聊天窗口
      - 未读消息计数显示（红色圆点徽章）
      - 时间戳显示（HH:MM 格式）
      - 系统消息和玩家消息样式区分
      - 自动滚动到最新消息
      - 最大 200 字符消息限制
    - **Table.tsx 集成**:
      - 聊天窗口放置在右下角
      - 折叠状态和未读计数追踪
      - 连接 gameStore.chatMessages 和 useNakama.sendChat
    - **功能完整性**:
      - 服务器端 OpCode.CHAT_MESSAGE 消息处理
      - 聊天消息广播给所有玩家和观战者
      - 断线/重连等系统事件自动生成聊天消息

20. **键盘快捷键支持** (2025-12-29)
    - **useKeyboardShortcuts Hook (`useKeyboardShortcuts.ts`)**:
      - 全局键盘事件监听，只在玩家回合时激活
      - 动作快捷键：F-Fold, C-Call/Check, B-Bet, R-Raise, A-All-in
      - 下注金额快捷键：1-Min, 2-1/2 Pot, 3-Pot, 4-Max
      - 调整金额：↑↓箭头键（按大盲或 10% 递增/递减）
      - 智能跳过输入框（不在聊天输入时触发）
      - 播放相应音效
    - **BettingControls 组件更新**:
      - 集成 useKeyboardShortcuts hook
      - 所有按钮显示快捷键提示（如 `[F]`, `[C]`）
      - 快速下注按钮显示数字键提示（`[1]`, `[2]`, `[3]`, `[4]`）
      - 可展开的键盘快捷键帮助面板
    - **KeyboardShortcutsHelp 组件 (`KeyboardShortcutsHelp.tsx`)**:
      - 三栏布局：动作、下注金额、调整
      - 美观的键盘样式显示（kbd 标签）
      - 关闭按钮和提示信息

21. **移动端响应式适配** (2025-12-29)
    - **Tailwind 配置更新 (`tailwind.config.js`)**:
      - 添加 `xs: 375px` 自定义断点
    - **Table 组件响应式 (`Table.tsx`)**:
      - 响应式高度：`h-[calc(100vh-120px)]` / `md:h-[700px]`
      - 双套玩家位置：`PLAYER_POSITIONS_MOBILE` / `PLAYER_POSITIONS_DESKTOP`
      - `useIsMobile()` hook 检测屏幕尺寸
      - 移动端社区牌使用 small 尺寸
      - BettingControls 固定在底部全宽显示
      - 移动端隐藏聊天/观战列表（玩家回合时）
    - **BettingControls 组件响应式 (`BettingControls.tsx`)**:
      - 添加 `isMobile` prop
      - 移动端：全宽圆角顶部容器、大触控按钮（py-3）
      - 横向滚动按钮列表
      - 更大的滑块触控区域
      - 移动端隐藏键盘快捷键
    - **Card 组件响应式 (`Card.tsx`)**:
      - 响应式尺寸：`w-8 h-11 md:w-10 md:h-14`（small）/ `w-10 h-14 md:w-14 md:h-20`
      - 响应式字体和居中花色尺寸
    - **Player 组件响应式 (`Player.tsx`)**:
      - 紧凑布局：更小的内边距和间距
      - 响应式文本尺寸（text-[9px] 到 md:text-xs）
      - 更紧凑的状态徽章
    - **App 和 RoomList 响应式**:
      - 移动端居中标题和按钮
      - 响应式房间列表布局
      - 更大的触控按钮
    - **index.html 优化**:
      - viewport: maximum-scale=1, user-scalable=no, viewport-fit=cover
      - apple-mobile-web-app-capable
      - theme-color: #0d2818
      - 自定义 CSS：overscroll-behavior、safe-area-inset、range slider 样式

22. **表情快捷发送功能** (2025-12-30)
    - **QuickEmotes 组件 (`QuickEmotes.tsx`)**:
      - 12 个主要表情：GG、NH、WP、TY、GL、LOL + 👍、👏、🎉、😎、😭、🔥
      - 12 个扩展表情：SHIP IT!、BRB、SIT OUT、BLUFF、TILTED、UNLUCKY + 🃏、💰、🤑、😤、🤔、😏
      - 可展开/收起面板设计
      - 悬停显示 tooltip 说明
      - 紧凑模式显示前 6 个常用表情
    - **Chat 组件集成**:
      - 添加 😊 表情切换按钮
      - 表情面板切换显示
      - 点击表情直接发送
    - **EmoteBar 导出**:
      - 用于其他组件快速集成表情

23. **Toast 通知系统** (2025-12-30)
    - **ToastContext (`ToastContext.tsx`)**:
      - 4 种通知类型：success（绿）、error（红）、warning（黄）、info（蓝）
      - 自动消失时间：success 3s、error 5s、warning 4s、info 3s
      - 最多显示 5 个通知
      - 便捷方法：`toast.success()`, `toast.error()`, `toast.warning()`, `toast.info()`
    - **Toast 组件 (`Toast.tsx`)**:
      - ToastItem：单个通知卡片
      - ToastContainer：全局通知容器（右上角）
      - 进入/退出动画（滑入/滑出）
      - 进度条显示剩余时间
      - 手动关闭按钮
    - **App.tsx 集成**:
      - ToastProvider 包装应用
      - 替换原有 error state
      - 连接成功/失败、加入房间等场景使用

24. **玩家头像系统** (2025-12-30)
    - **Avatar 组件 (`Avatar.tsx`)**:
      - 基于用户 ID 生成确定性颜色渐变作为默认头像
      - 10 种扑克主题渐变色调
      - 显示用户名首字母（1-2 个字符）
      - 支持自定义头像 URL（图片加载失败时回退到默认）
      - 4 种尺寸：xs、sm、md、lg
      - 可选在线状态指示点
    - **类型更新**:
      - Player 接口添加 `avatarUrl?: string` 字段
      - ServerPlayerInfo 添加 `avatarUrl` 字段
      - PublicPlayerInfo 添加 `avatarUrl` 字段
    - **Player 组件更新**:
      - 集成 Avatar 组件显示在玩家名称旁边
      - 头像+名称+筹码横向紧凑布局
      - 断线玩家头像显示离线状态点
    - **服务器端支持**:
      - Player 类型添加 avatarUrl 字段
      - getPublicPlayerInfo 返回 avatarUrl
      - 游戏状态广播包含头像信息

25. **用户系统（注册/登录、筹码存储）** (2025-12-30)
    - **服务器端 RPC (`user_chips.ts`)**:
      - `getChipsRpc`: 获取用户筹码余额和统计信息
      - `updateChipsRpc`: 更新用户筹码（buy_in/cash_out/win/lose/bonus）
      - `claimDailyRewardRpc`: 每日签到奖励（24小时冷却，500筹码）
      - `getLeaderboardRpc`: 获取排行榜（按总赢筹码排名）
      - `updateLeaderboardScore`: 更新排行榜分数
      - 使用 Nakama Storage 存储用户数据（balance, totalWon, totalLost, handsPlayed, handsWon）
      - 新用户初始 10,000 筹码
    - **前端认证 (`useNakama.ts`)**:
      - `authenticateGuest()`: 设备 ID 匿名登录
      - `authenticateEmail()`: 邮箱登录
      - `registerEmail()`: 邮箱注册（带 displayName）
      - `getChips()`: 获取用户筹码
      - `claimDailyReward()`: 领取每日奖励
      - `getLeaderboard()`: 获取排行榜
    - **前端状态 (`gameStore.ts`)**:
      - 添加 `authMode: 'guest' | 'email'` 状态
      - 添加 `userChips: UserChipsData | null` 状态
      - 添加 `setAuthMode()`, `setUserChips()`, `updateChipsBalance()` 方法
    - **认证表单组件 (`AuthForm.tsx`)**:
      - 三种模式切换：游客登录 / 邮箱登录 / 邮箱注册
      - 表单验证（密码最少6位）
      - 显示注册账号的好处
    - **用户面板组件 (`UserPanel.tsx`)**:
      - 显示用户头像、名称、认证模式
      - 筹码余额显示
      - 统计信息：总赢/总输/对局数/胜率
      - 每日奖励按钮（带倒计时）
      - 游客账号警告提示
    - **App.tsx 更新**:
      - 替换简单连接按钮为 AuthForm
      - 大厅布局改为左侧 UserPanel + 右侧 RoomList
      - 认证后自动获取用户筹码
    - **类型定义 (`poker.ts`)**:
      - 添加 `UserChipsData`、`DailyRewardResponse`、`LeaderboardEntry` 类型
      - 添加 `AuthMode` 类型

26. **PWA 支持（Service Worker、离线提示）** (2025-12-30)
    - **Vite PWA 配置 (`vite.config.ts`)**:
      - 安装 `vite-plugin-pwa` 和 `workbox-window`
      - 配置 VitePWA 插件：registerType: 'prompt'
      - Web App Manifest: name、short_name、icons、theme_color、background_color
      - Workbox 配置：缓存静态资源、字体缓存策略
      - 开发模式启用 PWA 测试
    - **PWA 图标 (`public/`)**:
      - 创建 `generate-pwa-icons.js` 图标生成脚本
      - SVG 图标：pwa-192x192.svg、pwa-512x512.svg
      - Maskable 图标：pwa-maskable-192x192.svg、pwa-maskable-512x512.svg
      - favicon.svg、apple-touch-icon.svg
      - 扑克主题设计（黑桃 A + 筹码堆叠）
    - **离线指示组件 (`OfflineIndicator.tsx`)**:
      - 监听 online/offline 事件
      - 离线时显示红色横幅提示
      - 恢复在线时显示绿色"Back online"提示
      - 3 秒后自动隐藏恢复提示
    - **PWA 更新提示组件 (`PWAUpdatePrompt.tsx`)**:
      - 使用 `useRegisterSW` hook 管理 Service Worker
      - 新版本可用时显示更新提示
      - "Reload"/"Later" 按钮选择
      - 安装提示（Add to Home Screen）
      - BeforeInstallPromptEvent 类型声明
    - **App.tsx 集成**:
      - 添加 OfflineIndicator 和 PWAUpdatePrompt 组件
    - **index.html 更新**:
      - manifest.webmanifest 链接
      - apple-touch-icon 链接
      - 更新 favicon 路径
    - **CSS 动画 (`index.css`)**:
      - slide-up 动画（更新提示从底部滑入）
      - slide-down 动画（离线提示从顶部滑入）

## 当前阻塞问题

（暂无）

## 下一步建议

1. ~~创建 docker-compose.yml~~ ✅
2. ~~初始化 Nakama TypeScript 项目结构~~ ✅
3. ~~实现基础的 match handler~~ ✅
4. ~~创建 React 前端项目结构~~ ✅
5. ~~实现 Nakama 客户端连接和认证~~ ✅
6. ~~实现 Deck 类（洗牌、发牌）~~ ✅
7. ~~实现游戏状态管理和流程控制~~ ✅
8. ~~实现 matchLoop 处理玩家操作~~ ✅
9. ~~实现手牌评估器（HandEvaluator）~~ ✅
10. ~~完善前端游戏状态接收和 UI 更新~~ ✅
11. ~~添加手牌评估器单元测试~~ ✅
12. ~~添加 betting.ts 单元测试~~ ✅
13. ~~添加 game_state.ts 单元测试~~ ✅
14. ~~添加 deck.ts 单元测试~~ ✅
15. ~~完善边池 UI 展示（多个 side pots 的显示）~~ ✅
16. ~~实现玩家断线重连逻辑~~ ✅
17. ~~实现游戏房间列表功能~~ ✅
18. ~~实现观战模式~~ ✅
19. ~~添加游戏音效和动画~~ ✅
20. ~~添加筹码动画和胜利特效~~ ✅
21. ~~集成聊天功能~~ ✅
22. ~~添加键盘快捷键支持~~ ✅
23. ~~移动端响应式适配~~ ✅
24. ~~添加表情快捷发送~~ ✅
25. ~~添加 Toast 通知系统~~ ✅
26. ~~添加玩家头像系统~~ ✅
27. ~~实现用户系统（注册/登录、筹码存储）~~ ✅
28. ~~添加 PWA 支持（Service Worker、离线提示）~~ ✅
29. **优化加载性能（代码分割、懒加载组件）** ← 下一步
30. 添加排行榜 UI 组件
31. 添加游戏历史记录功能
32. 实现多语言支持（i18n）

---
*最后更新: 2025-12-30 - PWA 支持完成（Service Worker、离线提示、安装提示、更新提示）*
