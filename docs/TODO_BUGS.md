# Bug Fix TODO List

## Issue Analysis

### Bug 1: Analytics 页面数据变成0
**现象**: Analytics 页面的内容过一会会变成0，然后再也不变了

**根本原因**:
- `App.tsx` 第 178-181 行，Analytics 使用 `filteredSessions` 而不是全部 `sessions`
- 当用户在 Sessions 页搜索过滤后，`filteredSessions` 变少或为空
- 切换到 Analytics 时，继承了这个过滤状态

**代码位置**:
```tsx
// App.tsx:178-181
{activeTab === 'analytics' && !loading && filteredSessions.length > 0 && (
  <>
    <CostPanel sessions={filteredSessions} />      // 问题: 使用 filteredSessions
    <AnalyticsPanel sessions={filteredSessions} /> // 问题: 使用 filteredSessions
  </>
)}
```

**解决方案**: Analytics 应该使用全部 `sessions`，不受搜索过滤影响

---

### Bug 2: Projects 页面点击后只显示一个项目
**现象**: 点击 Project 卡片后会切换到 Sessions 页面，再切回 Projects 页面只显示一个项目

**根本原因**:
- `handleProjectClick` (App.tsx:117-122) 设置 `searchQuery` 为 projectPath
- `useProjects` (App.tsx:55) 使用 `filteredSessions`
- 切回 Projects 时，searchQuery 仍然保持，导致只显示匹配的项目

**代码位置**:
```tsx
// App.tsx:55
const { projects, stats: projectStats } = useProjects(filteredSessions) // 问题

// App.tsx:117-122
const handleProjectClick = useCallback((projectPath: string) => {
  setActiveTab('sessions')
  setSearchQuery(projectPath)  // 设置了搜索过滤
  showToast(`Filtered to: ${projectPath.split('/').pop()}`, 'info')
}, [setSearchQuery, showToast])
```

**解决方案**: Projects 页面应该使用全部 `sessions`，不受搜索过滤影响

---

## Fix Plan

### Task 1: Fix Analytics to use all sessions
- [ ] 修改 App.tsx，让 CostPanel 和 AnalyticsPanel 使用 `sessions` 而不是 `filteredSessions`

### Task 2: Fix Projects to use all sessions
- [ ] 修改 App.tsx，让 useProjects 使用 `sessions` 而不是 `filteredSessions`

### Task 3: Test the fixes
- [ ] 确保 Sessions 页面的搜索过滤仍然正常工作
- [ ] 确保 Analytics 页面不受搜索影响，始终显示全部数据
- [ ] 确保 Projects 页面不受搜索影响，始终显示所有项目
- [ ] 确保点击 Project 后切换到 Sessions 页面，搜索框正确过滤
- [ ] 确保切回 Projects 页面后显示所有项目

---

## Summary

| Bug | 原因 | 解决方案 |
|-----|------|----------|
| Analytics 变0 | 使用 filteredSessions | 改用 sessions |
| Projects 只显示1个 | 使用 filteredSessions | 改用 sessions |
