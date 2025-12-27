# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Terminal app selection for session recovery workflow
- CLI `--terminal-app` option to specify terminal for recover command
- Enhanced terminal compatibility for session restoration

### Changed
- Improved session recovery terminal selection UI

### Fixed
- OAuth token credential handling (support multiple credential names)
- Project merging when same name exists in different directories

## [1.0.1] - 2025-12-27

### Added
- Hide Dock Icon option for menubar-only mode
- Auto-resize window based on content
- Toast notification component

### Fixed
- Rust compiler warnings (removed unused icon generator functions)
- Deprecated Tauri API (menu_on_left_click → show_menu_on_left_click)
- Dock visibility using Tauri's native activation policy API
- Settings row layout to prevent text truncation
- Dashboard button shows "Under Development" toast

## [1.0.0] - 2025-12-16

### Added
- **Session Recovery**: Monitor and recover Claude Code sessions with terminal selection
- **Web UI Dashboard**: React-based web interface for viewing sessions and analytics
- **Real-time Updates**: WebSocket integration for live session monitoring
- **Projects View**: Group sessions by project directory
- **Analytics Dashboard**: Usage tracking and quota/rate limit display
- **Tab Navigation**: Separate Sessions and Analytics views in web UI
- **Usage Analytics**: Integration with ccusage for accurate token counting and cost calculation
- **Multi-theme Support**: 5 theme options for web UI customization
- **Lazy Loading**: Efficient loading of session details in expanded views
- **API Performance**: Optimized with batching and pagination

### Changed
- Improved quota error messages for better clarity
- Enhanced quota progress bars visibility
- Updated README with real screenshots and eye-catching layout
- Refactored API performance with rate limiting improvements

### Fixed
- WebSocket connection leak prevention during Vite HMR
- Singleton WebSocket manager to prevent connection leaks on re-renders
- Infinite re-render loops in lazy loading components
- Token counting to include prompt cache tokens
- Analytics and Projects views to use all sessions instead of filtered subset
- StatsBar and Toolbar visibility on Analytics tab

### Removed
- Unused AnalyticsPanel and CostPanel components
- WebSocket debug logging

### Technical Details
- **Runtime**: Bun
- **CLI Framework**: Commander.js
- **Web Backend**: Hono
- **Web Frontend**: React 18 + Vite + TypeScript
- **Database**: SQLite
- **Terminal UI**: Ink (React for CLI)
- **Version**: 1.0.0

## Project Milestones

### Phase 1: Core Enhancements
- Foundation of session monitoring and recovery
- Initial CLI implementation

### Phase 2: Analytics & Notifications
- Analytics dashboard
- Notification system implementation

### Phase 3: Real-time Updates & Sub-agent Tracking
- WebSocket integration for live updates
- Sub-agent session tracking capability

### Phase 4-6: Frontend Optimization & Refinements
- Comprehensive React performance optimization
- Code quality improvements
- Enhanced user experience

---

For more information, see [README.md](./README.md) and [CLAUDE.md](./CLAUDE.md).
