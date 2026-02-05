# Zorvia TUI & CLI Theme Design

## Overview

Zorvia's theme system is inspired by GuestKit's proven design patterns, adapted for KubeVirt VM management workflows. The theme emphasizes clarity, visual hierarchy, and operational efficiency.

## Color Palette

### Primary Colors

**PURPLE (Primary Brand)** - Kubernetes/Virtualization Theme
- `PRIMARY`: RGB(136, 86, 222) - #8856DE (Vivid purple, inspired by Kubernetes)
- `DARK_PRIMARY`: RGB(106, 60, 180) - #6A3CB4 (Used for borders and secondary elements)
- `LIGHT_PRIMARY`: RGB(165, 115, 255) - #A573FF (Lighter accent, labels and highlights)

### Status Colors

- `SUCCESS`: RGB(50, 205, 50) - #32CD32 (LimeGreen - VMs running, operations succeeded)
- `WARNING`: RGB(255, 165, 0) - #FFA500 (Orange - VMs paused, warnings, degraded states)
- `ERROR`: RGB(220, 50, 47) - #DC322F (Red - VMs stopped, failed operations, critical errors)
- `INFO`: RGB(100, 150, 255) - #6496FF (Soft blue - informational content)

### VM State Colors

- `VM_RUNNING`: `SUCCESS` - Active and healthy VMs
- `VM_PENDING`: RGB(255, 200, 100) - #FFC864 (Yellow - VMs starting/pending)
- `VM_STOPPED`: RGB(150, 150, 150) - #969696 (Gray - VMs stopped)
- `VM_FAILED`: `ERROR` - Failed VMs
- `VM_MIGRATING`: RGB(100, 200, 255) - #64C8FF (Cyan - VMs in migration)
- `VM_PAUSED`: `WARNING` - Paused VMs

### Resource Colors

- `CPU_COLOR`: RGB(100, 200, 100) - #64C864 (Green - CPU usage/allocation)
- `MEMORY_COLOR`: RGB(100, 150, 255) - #6496FF (Blue - Memory usage/allocation)
- `DISK_COLOR`: RGB(220, 120, 255) - #DC78FF (Magenta - Disk usage/allocation)
- `NETWORK_COLOR`: RGB(100, 220, 220) - #64DCDC (Cyan - Network interfaces)

### Text & Background

- `TEXT_PRIMARY`: RGB(220, 220, 220) - #DCDCDC (Main text - soft white)
- `TEXT_SECONDARY`: RGB(180, 180, 180) - #B4B4B4 (Secondary text - dimmed)
- `TEXT_MUTED`: RGB(120, 120, 120) - #787878 (Muted text - help, hints)
- `BG_COLOR`: Color::Reset - Transparent (inherit terminal background)
- `BORDER_COLOR`: `DARK_PRIMARY` - Consistent borders

### Namespace/Cluster Colors

- `NAMESPACE_DEFAULT`: RGB(100, 150, 255) - #6496FF (Default namespace)
- `NAMESPACE_SYSTEM`: RGB(255, 100, 100) - #FF6464 (System namespaces - kube-system, etc.)
- `NAMESPACE_USER`: RGB(150, 255, 150) - #96FF96 (User namespaces)

## Typography & Modifiers

### Text Modifiers

- **BOLD**: Used for titles, VM names, resource values, active states
- **UNDERLINED**: Active tabs, search queries, highlighted selections
- **ITALIC**: Help text, disclaimers, status descriptions
- **DIM**: Disabled items, stopped VMs, secondary information

### Font Hierarchy

1. **Titles**: BOLD + PRIMARY color
2. **Labels**: LIGHT_PRIMARY + normal weight
3. **Values**: TEXT_PRIMARY + BOLD (for emphasis) or normal
4. **Help Text**: TEXT_MUTED + ITALIC
5. **Active Items**: BOLD + UNDERLINED + appropriate color

## Layout Architecture

### Main TUI Layout (Vertical Split)

```
┌─────────────────────────────────────────────────────────┐
│ Header (3 lines)                                        │
│   - Logo + Title + Current Context (namespace/cluster)  │
├─────────────────────────────────────────────────────────┤
│ Stats Bar (2 lines) - Optional, configurable           │
│   - Total VMs | Running | Stopped | CPU% | Memory%     │
├─────────────────────────────────────────────────────────┤
│ Navigation Tabs (3 lines)                              │
│   Dashboard | VMs | Templates | Events | Monitoring     │
├─────────────────────────────────────────────────────────┤
│                                                         │
│ Content Area (Variable Height)                         │
│   - Main view content (tables, charts, details)        │
│                                                         │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ Footer (1 line)                                        │
│   - Keyboard shortcuts | Search info | Status          │
└─────────────────────────────────────────────────────────┘
```

### Constraint System

- Header: `Length(3)`
- Stats Bar: `Length(2)` (conditional rendering)
- Navigation Tabs: `Length(3)`
- Content Area: `Min(10)` with `Percentage(100)` fill
- Footer: `Length(1)`

## TUI Views

### 1. Dashboard View
**Purpose**: Overview of VM infrastructure

**Components**:
- VM status distribution (gauge chart)
- Resource utilization (CPU, Memory, Disk - bar charts)
- Recent events timeline
- Health summary

**Colors**:
- Status gauges use VM state colors
- Resource bars use resource-specific colors
- Health indicators: SUCCESS/WARNING/ERROR

### 2. VMs View
**Purpose**: List and manage VMs

**Components**:
- Table with columns: Name, Namespace, Status, CPU, Memory, Node, Age
- Status indicators with color-coded symbols
- Quick actions footer

**Status Symbols**:
- ● Running (SUCCESS)
- ◐ Pending (VM_PENDING)
- ○ Stopped (VM_STOPPED)
- ✗ Failed (ERROR)
- ⟳ Migrating (VM_MIGRATING)
- ⏸ Paused (WARNING)

**Colors**:
- VM names: TEXT_PRIMARY + BOLD
- Namespaces: NAMESPACE colors based on type
- Status: VM state colors
- Resources: Resource-specific colors

### 3. VM Detail View
**Purpose**: Detailed information about a single VM

**Components**:
- VM metadata block
- Resource allocation (gauges)
- Disks table
- Network interfaces table
- Cloud-init/user-data display
- Events log

**Layout**: 2-column layout (60/40 split)
- Left: Metadata, resources, events
- Right: Configuration, volumes, networking

### 4. Templates View
**Purpose**: Browse and inspect VM templates

**Components**:
- Template list with descriptions
- Template preview (selected template details)
- Create from template quick action

**Colors**:
- Template names: PRIMARY + BOLD
- OS indicators: Color-coded badges
  - Ubuntu: RGB(233, 84, 32) (orange)
  - Fedora: RGB(51, 105, 173) (blue)
  - CentOS: RGB(156, 66, 157) (purple)
  - Debian: RGB(215, 10, 83) (red)
  - Windows: RGB(0, 120, 212) (blue)
  - RHEL: RGB(238, 0, 0) (red)

### 5. Events View
**Purpose**: Real-time event stream from Kubernetes

**Components**:
- Scrollable event list
- Event type filters
- Search functionality

**Event Types & Colors**:
- Normal events: INFO
- Warning events: WARNING
- Error events: ERROR

### 6. Monitoring View
**Purpose**: Real-time metrics and monitoring

**Components**:
- CPU usage sparklines (per VM)
- Memory usage sparklines
- Network I/O graphs
- Disk I/O graphs

**Colors**: Resource-specific colors with gradients

### 7. Create/Edit View
**Purpose**: Interactive VM creation/editing

**Components**:
- Form-based input (using ratatui input widgets)
- Real-time validation feedback
- Preview pane (shows generated YAML)

**Validation Colors**:
- Valid fields: SUCCESS
- Invalid fields: ERROR + error message
- Optional fields: TEXT_MUTED

## CLI Output Formats

### Human-Readable (Default)

**Table Format**:
```
NAME         NAMESPACE  STATUS   CPU  MEMORY  NODE          AGE
my-ubuntu    default    Running  2    4Gi     worker-node1  2d
web-server   prod       Running  8    16Gi    worker-node2  5h
```

**Colors**:
- Headers: PRIMARY + BOLD
- VM names: TEXT_PRIMARY + BOLD
- Status: VM state colors
- Resources: Resource colors
- Age: TEXT_SECONDARY

### Structured Formats

- **JSON**: Syntax-highlighted if terminal supports it
- **YAML**: Syntax-highlighted if terminal supports it
- **Table**: ASCII table with borders

## Interactive Elements

### Search Interface

**Visual Design**:
- Search prompt: `🔍 Search:` in INFO color
- Query text: UNDERLINED
- Mode toggles:
  - `Ctrl+I`: Case-insensitive (indicator in LIGHT_PRIMARY)
  - `Ctrl+R`: Regex mode (indicator in WARNING)
- Match counter: `[3/15]` in TEXT_MUTED

### Keyboard Shortcuts

**Navigation**:
- `↑/k`: Move up
- `↓/j`: Move down
- `←/h`: Previous tab/panel
- `→/l`: Next tab/panel
- `Tab`: Cycle panels
- `g/Home`: Go to top
- `G/End`: Go to bottom
- `Ctrl+D`: Page down
- `Ctrl+U`: Page up

**Actions**:
- `Enter`: Select/View details
- `c`: Create VM
- `e`: Edit VM
- `d`: Delete VM
- `s`: Start VM
- `S`: Stop VM
- `r`: Restart VM
- `/`: Search
- `n`: Next search result
- `N`: Previous search result
- `f`: Filter
- `?`: Help overlay
- `q/Esc`: Quit/Cancel

**Shortcuts Display** (in footer):
- Command keys: PRIMARY + BOLD
- Separators: `|` in TEXT_MUTED
- Descriptions: TEXT_SECONDARY

Example: `[c]Create  [e]Edit  [d]Delete  [/]Search  [?]Help  [q]Quit`

### Modal Dialogs

**Confirmation Dialogs**:
```
┌─────────────────────────────────────┐
│ Confirm Delete                      │
├─────────────────────────────────────┤
│                                     │
│ Delete VM "my-ubuntu"?              │
│                                     │
│ This action cannot be undone.       │
│                                     │
│     [Yes]      [No]                 │
└─────────────────────────────────────┘
```

**Colors**:
- Border: PRIMARY
- Title: PRIMARY + BOLD
- Content: TEXT_PRIMARY
- Warning text: WARNING
- Active button: SUCCESS + BOLD + UNDERLINED
- Inactive button: TEXT_SECONDARY

### Progress Indicators

**Spinner** (for async operations):
```
⠋ Creating VM "my-ubuntu"...
⠙ Starting VM "web-server"...
✓ VM "db-server" created successfully
✗ Failed to delete VM "test-vm"
```

**Colors**:
- Spinner: INFO
- Success: SUCCESS
- Error: ERROR
- Progress text: TEXT_SECONDARY

**Progress Bar**:
```
Creating VM: [████████████----] 75%
```

**Colors**:
- Completed: SUCCESS
- Remaining: TEXT_MUTED
- Percentage: TEXT_PRIMARY + BOLD

### Help Overlay

**Design**:
- Centered overlay (80% width, 90% height)
- Sections: Navigation, Actions, Filters, Search, Misc
- Color-coded command categories

**Layout**:
```
┌────────────────── Help ──────────────────┐
│                                          │
│ Navigation                               │
│   ↑/k    Move up                         │
│   ↓/j    Move down                       │
│   ...                                    │
│                                          │
│ Actions                                  │
│   c      Create VM                       │
│   e      Edit VM                         │
│   ...                                    │
│                                          │
│ Press ? or Esc to close                  │
└──────────────────────────────────────────┘
```

**Colors**:
- Border: PRIMARY
- Section titles: LIGHT_PRIMARY + BOLD + UNDERLINED
- Command keys: PRIMARY + BOLD
- Descriptions: TEXT_PRIMARY

## Status Indicators

### VM Status Badge

Visual representation with symbol + text:
```
● Running    ◐ Pending     ○ Stopped
✗ Failed     ⟳ Migrating   ⏸ Paused
```

### Health Indicators

**Gauge-Based**:
- 0-50%: ERROR + ▁▂▃▄▅
- 51-75%: WARNING + ▄▅▆▇
- 76-100%: SUCCESS + ▆▇█

### Resource Usage Visualization

**CPU/Memory Bars**:
```
CPU:    [████████░░] 80%  (8/10 cores)
Memory: [██████░░░░] 60%  (12Gi/20Gi)
```

**Colors**:
- 0-70%: SUCCESS
- 71-90%: WARNING
- 91-100%: ERROR

## Configuration System

### Theme Configuration File

**Location**: `~/.config/zorvia/tui.toml`

```toml
[theme]
# Theme name: "default", "dark", "light", "kubernetes"
name = "default"

# Custom color overrides (hex format)
# primary = "#8856DE"
# success = "#32CD32"
# error = "#DC322F"

[ui]
# Show splash screen on startup
show_splash = true
splash_duration_ms = 800

# Show stats bar
show_stats_bar = true

# Default view on startup
default_view = "dashboard"  # dashboard, vms, templates, events, monitoring

# Auto-refresh interval (seconds, 0 to disable)
auto_refresh_interval = 5

# Table formatting
table_style = "rounded"  # rounded, sharp, double, simple

[behavior]
# Confirmation prompts
confirm_delete = true
confirm_stop = false
confirm_restart = false

# Search defaults
search_case_sensitive = false
search_regex = false

# Scroll behavior
scroll_amount = 10  # lines per page up/down

# Maximum items in lists
max_list_items = 1000

[keybindings]
# Vim mode
vim_mode = true

# Quick jump menu (Ctrl+P)
quick_jump = true

# Custom keybindings (future feature)
# [keybindings.custom]
# "ctrl+t" = "toggle_theme"
```

## Dependencies

### Core TUI

```toml
[dependencies]
# Terminal UI
ratatui = "0.28"
crossterm = "0.28"

# CLI colors and formatting
colored = "3.1"
owo-colors = "4.0"

# Error handling with styled output
miette = { version = "7.0", features = ["fancy"] }

# Configuration
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# Terminal input (for REPL-like features)
rustyline = "17.0"

# Progress bars
indicatif = "0.17"
```

## Visual Design Principles

### 1. Clarity Over Complexity
- Use color purposefully, not decoratively
- Consistent status colors across all views
- Clear visual hierarchy through typography

### 2. Operational Efficiency
- Keyboard-first navigation
- Quick access to common actions
- Real-time feedback for all operations

### 3. Information Density
- Balance information density with readability
- Use sparklines and compact charts for trends
- Progressive disclosure (summary → details)

### 4. Consistency
- Reuse color constants across all components
- Standard border styles and spacing
- Unified modifier patterns

### 5. Accessibility
- High contrast colors (WCAG AA compliant)
- Support for reduced motion
- Clear focus indicators
- Symbol + color + text for status (not color alone)

## Implementation Strategy

### Phase 1: Core Theme System
1. Create `src/tui/theme.rs` with color constants
2. Create `src/tui/config.rs` for configuration
3. Implement basic layout in `src/tui/ui.rs`

### Phase 2: Views Implementation
1. Dashboard view
2. VMs list view
3. VM detail view
4. Templates view
5. Events view
6. Monitoring view (optional, future)

### Phase 3: Interactivity
1. Keyboard navigation
2. Search functionality
3. Modal dialogs
4. Help overlay
5. Real-time updates

### Phase 4: Polish
1. Splash screen
2. Animations and transitions
3. Custom themes support
4. Keybinding customization

## Testing Theme

### Color Testing Script

Create `scripts/test_theme.sh`:
```bash
#!/bin/bash
# Test theme colors in terminal

echo -e "\033[38;2;136;86;222m● PRIMARY\033[0m"
echo -e "\033[38;2;50;205;50m● SUCCESS\033[0m"
echo -e "\033[38;2;255;165;0m● WARNING\033[0m"
echo -e "\033[38;2;220;50;47m● ERROR\033[0m"
echo -e "\033[38;2;100;150;255m● INFO\033[0m"
```

### Visual Regression Testing
- Screenshot-based testing with `insta` crate
- Compare rendered TUI frames
- Ensure consistent rendering across terminals

## Inspiration & References

- **GuestKit**: Color palette system, view architecture, help overlay design
- **Kubernetes Dashboard**: Resource visualization patterns
- **k9s**: Keyboard navigation, real-time updates
- **lazydocker**: Status indicators, interactive TUI patterns
- **Ratatui examples**: Component implementations, layout patterns

## Future Enhancements

1. **Theme Marketplace**: Share custom themes
2. **Dark/Light Auto-switching**: Based on terminal settings
3. **Colorblind Modes**: Alternative palettes
4. **ASCII-only Mode**: For terminals without Unicode support
5. **Custom Widgets**: VM topology graph, migration flow diagram
