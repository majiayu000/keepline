# Projects View Design Document

## Overview

Add a new "Projects" tab to display Claude Code sessions grouped by project/directory, providing a bird's-eye view of all active projects and their status.

## User Story

As a developer using multiple Claude Code sessions across different projects, I want to see all my projects at a glance, understand which ones are active, and quickly identify what each project is currently working on.

## Design: Project Cards Grid + Stats Overview

### Layout Structure

```
┌─────────────────────────────────────────────────────────┐
│ [Sessions] [Analytics] [Projects]                       │ ← Tab Navigation
├─────────────────────────────────────────────────────────┤
│  📊 12 Projects │ 🟢 5 Active │ 💤 7 Idle              │ ← Project Stats Bar
├─────────────────────────────────────────────────────────┤
│ ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐│
│ │ tasker    │ │ agent-kb  │ │ jsonrepair│ │ cupcake   ││
│ │ 🟢2 🟡1 🔴1│ │ 🟢1 🟡2   │ │ 🟢1    🔴3│ │    🔴2    ││
│ │           │ │           │ │           │ │           ││
│ │ 看下最新  │ │ 自动化飞轮│ │contributor│ │ (no task) ││
│ │ 分支...   │ │ 实现      │ │ 分析      │ │           ││
│ │ ───────── │ │ ───────── │ │ ───────── │ │ ───────── ││
│ │ 2min ago  │ │ 38min ago │ │ now       │ │ 21h ago   ││
│ └───────────┘ └───────────┘ └───────────┘ └───────────┘│
│                                                         │
│ ┌───────────┐ ┌───────────┐ ...                        │
│ │ hackclub  │ │ websockets│                            │
│ │ ...       │ │ ...       │                            │
│ └───────────┘ └───────────┘                            │
└─────────────────────────────────────────────────────────┘
```

## Components

### 1. ProjectStatsBar

Displays aggregate project statistics at the top.

**Props:**
```typescript
interface ProjectStatsBarProps {
  totalProjects: number
  activeProjects: number  // Has at least one running/waiting session
  idleProjects: number    // Only idle/lost sessions
}
```

**Display:**
- Total project count
- Active projects count (with running/waiting sessions)
- Idle projects count

### 2. ProjectCard

Individual card representing one project/directory.

**Props:**
```typescript
interface ProjectCardProps {
  project: ProjectInfo
  onClick?: (projectPath: string) => void
}

interface ProjectInfo {
  path: string              // Full directory path
  name: string              // Extracted project name (last segment)
  sessions: Session[]       // All sessions in this directory
  stats: {
    running: number
    waiting: number
    idle: number
    lost: number
    completed: number
    total: number
  }
  currentTask?: string      // Title/prompt of active session
  lastActiveAt: string      // Most recent activity timestamp
}
```

**Visual Elements:**
- Project name (bold, top)
- Status indicators: 🟢 running 🟡 idle 🔴 lost (with counts)
- Current task snippet (truncated to ~30 chars)
- Last activity time (relative, e.g., "2min ago")

### 3. ProjectsGrid

Container for all project cards with responsive grid layout.

**Props:**
```typescript
interface ProjectsGridProps {
  projects: ProjectInfo[]
  onProjectClick?: (projectPath: string) => void
}
```

**Layout:**
- CSS Grid: `repeat(auto-fill, minmax(200px, 1fr))`
- Gap: 1rem
- Cards sorted by activity (most recent first)

## Data Aggregation

### useProjects Hook

New hook to aggregate session data by directory.

```typescript
interface UseProjectsReturn {
  projects: ProjectInfo[]
  stats: {
    total: number
    active: number
    idle: number
  }
  loading: boolean
}

function useProjects(sessions: Session[]): UseProjectsReturn {
  // Group sessions by directory
  // Calculate stats for each project
  // Sort by lastActiveAt descending
  // Return aggregated data
}
```

### Aggregation Logic

```typescript
function aggregateProjects(sessions: Session[]): ProjectInfo[] {
  const projectMap = new Map<string, Session[]>()

  // Group by directory
  for (const session of sessions) {
    const existing = projectMap.get(session.directory) || []
    projectMap.set(session.directory, [...existing, session])
  }

  // Transform to ProjectInfo
  return Array.from(projectMap.entries()).map(([path, sessions]) => ({
    path,
    name: extractProjectName(path),
    sessions,
    stats: calculateStats(sessions),
    currentTask: findCurrentTask(sessions),
    lastActiveAt: findLastActive(sessions),
  }))
}

function extractProjectName(path: string): string {
  // "/Users/xxx/code/tasker" → "tasker"
  return path.split('/').filter(Boolean).pop() || path
}
```

## UI Integration

### Tab Navigation Update

Add "Projects" to existing TabNav:

```typescript
const TABS: Tab[] = [
  { id: 'sessions', label: 'Sessions', icon: '>' },
  { id: 'analytics', label: 'Analytics', icon: '$' },
  { id: 'projects', label: 'Projects', icon: '#' },  // NEW
]

export type TabId = 'sessions' | 'analytics' | 'projects'
```

### App.tsx Changes

```tsx
{/* Projects Tab */}
{activeTab === 'projects' && !loading && (
  <>
    <ProjectStatsBar stats={projectStats} />
    <ProjectsGrid
      projects={projects}
      onProjectClick={handleProjectClick}
    />
  </>
)}
```

## Styling

### ProjectCard CSS

```css
.card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 1rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.card:hover {
  border-color: var(--color-accent);
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.projectName {
  font-weight: 600;
  font-size: 1rem;
  margin-bottom: 0.5rem;
  color: var(--color-text);
}

.statusRow {
  display: flex;
  gap: 0.75rem;
  font-size: 0.85rem;
  margin-bottom: 0.5rem;
}

.currentTask {
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lastActive {
  font-size: 0.75rem;
  color: var(--color-text-secondary);
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}
```

## File Structure

```
src/web/client/src/
├── components/
│   ├── ProjectCard/
│   │   ├── ProjectCard.tsx
│   │   ├── ProjectCard.module.css
│   │   └── index.ts
│   ├── ProjectsGrid/
│   │   ├── ProjectsGrid.tsx
│   │   ├── ProjectsGrid.module.css
│   │   └── index.ts
│   └── ProjectStatsBar/
│       ├── ProjectStatsBar.tsx
│       ├── ProjectStatsBar.module.css
│       └── index.ts
├── hooks/
│   └── useProjects.ts
└── types/
    └── project.ts
```

## Future Enhancements

1. **Click to filter** - Click project card to filter Sessions tab by that directory
2. **Expand details** - Click to show all sessions in that project
3. **Project settings** - Custom project names, icons, notes
4. **Progress tracking** - Track completed vs total tasks per project
5. **Time tracking** - Total time spent on each project

## Implementation Estimate

- **Components**: 2-3 hours
- **Hook & Logic**: 1 hour
- **Styling**: 1 hour
- **Testing**: 30 min

**Total: ~4-5 hours**

## Priority

Medium - Nice to have for multi-project workflow management.
