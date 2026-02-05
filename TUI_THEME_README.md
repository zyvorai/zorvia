# 🎨 Zorvia TUI & CLI Theme

> A comprehensive theme system for Zorvia, inspired by GuestKit's design patterns

## ✨ Features

- 🎨 **Purple Kubernetes-inspired color palette** with 20+ semantic colors
- 🖥️ **TUI framework integration** using Ratatui 0.28
- 🌈 **CLI color helpers** for terminal output
- ⚙️ **User-configurable** via TOML configuration file
- ♿ **Accessible** with symbol + color + text for all status indicators
- 🧪 **Fully tested** with unit tests and visual color tests
- 📚 **Comprehensive documentation** (1000+ lines)

## 🚀 Quick Start

### View the Colors

Run the color test script to see all theme colors in your terminal:

```bash
./scripts/test_theme.sh
```

You'll see:
- ✓ All color swatches with hex codes
- ✓ VM status symbols (●, ◐, ○, ✗, ⟳, ⏸) with colors
- ✓ Sample VM list with themed output
- ✓ Resource usage bars
- ✓ Border examples

### Run Tests

```bash
# All TUI module tests
cargo test --lib tui

# Specific module tests
cargo test --lib tui::theme
cargo test --lib tui::config
cargo test --lib tui::colors
```

**Result**: ✅ 9 tests passed

### Use in Your Code

#### TUI Example (Ratatui)

```rust
use zorvia::tui::Theme;
use ratatui::widgets::{Block, Borders, List, ListItem};

fn render_vm_list(vms: &[VM]) {
    let theme = Theme::default();

    // Create themed list items
    let items: Vec<ListItem> = vms
        .iter()
        .map(|vm| {
            let (symbol, _) = theme.vm_status_symbol(&vm.status);
            let content = format!("{} {} ({})",
                symbol,
                vm.name,
                vm.namespace
            );
            ListItem::new(content).style(theme.text_style())
        })
        .collect();

    // Create themed block
    let block = Block::default()
        .title("Virtual Machines")
        .title_style(theme.title_style())
        .borders(Borders::ALL)
        .border_style(theme.border_style());

    let list = List::new(items).block(block);
}
```

#### CLI Example (Colored Output)

```rust
use zorvia::tui::colors::cli;
use zorvia::tui::colors::{vm_status_symbol, resource_bar};

fn display_vm_info(vm: &VM) {
    println!("{}", cli::header("VM Information"));
    println!("Name:      {}", cli::vm_name(&vm.name));
    println!("Namespace: {}", cli::namespace(&vm.namespace));
    println!("Status:    {} {}",
        vm_status_symbol(&vm.status),
        cli::vm_status(&vm.status)
    );

    // Resource usage
    let cpu_bar = resource_bar(vm.cpu_usage, 20);
    println!("CPU:       [{}] {}%", cpu_bar, vm.cpu_usage);

    // Success message
    println!("{}", cli::success("VM information retrieved"));
}
```

#### Configuration Example

```rust
use zorvia::tui::TuiConfig;

fn load_user_config() -> anyhow::Result<TuiConfig> {
    // Load from ~/.config/zorvia/tui.toml
    let config = TuiConfig::load()?;

    // Check user preferences
    if config.ui.show_splash {
        show_splash(config.ui.splash_duration_ms);
    }

    if config.behavior.confirm_delete {
        // Show confirmation before deleting
    }

    if config.keybindings.vim_mode {
        // Enable hjkl navigation
    }

    Ok(config)
}
```

## 📁 Project Structure

```
zorvia/
├── src/tui/
│   ├── mod.rs              # Module exports
│   ├── theme.rs            # Theme system (400+ lines)
│   │   ├── Theme struct with 20+ color constants
│   │   ├── Style helper methods
│   │   ├── VM status styling
│   │   ├── Resource usage coloring
│   │   └── OS-specific colors
│   ├── config.rs           # Configuration (250+ lines)
│   │   ├── TuiConfig, UiConfig, BehaviorConfig
│   │   ├── TOML serialization
│   │   └── Load/save functionality
│   └── colors.rs           # CLI helpers (150+ lines)
│       ├── Terminal color functions
│       ├── VM status symbols
│       └── Resource bars
├── scripts/
│   └── test_theme.sh       # Visual color test (80 lines)
├── examples/
│   └── tui.toml            # Sample configuration (60 lines)
├── THEME_DESIGN.md         # Design specification (600+ lines)
├── THEME_IMPLEMENTATION.md # Implementation guide (400+ lines)
└── THEME_SUMMARY.md        # Summary overview (250+ lines)
```

**Total**: ~2000+ lines of code and documentation

## 🎨 Color Palette

### Primary (Purple Theme)
```
PRIMARY:       #8856DE  ████  Main brand color
DARK_PRIMARY:  #6A3CB4  ████  Borders, secondary
LIGHT_PRIMARY: #A573FF  ████  Labels, highlights
```

### Status
```
SUCCESS:  #32CD32  ████  VMs running, success
WARNING:  #FFA500  ████  Warnings, paused VMs
ERROR:    #DC322F  ████  Failed VMs, errors
INFO:     #6496FF  ████  Informational content
```

### VM States
```
● Running    #32CD32  ████  Active VMs
◐ Pending    #FFC864  ████  Starting VMs
○ Stopped    #969696  ████  Stopped VMs
✗ Failed     #DC322F  ████  Failed VMs
⟳ Migrating  #64C8FF  ████  Migrating VMs
⏸ Paused     #FFA500  ████  Paused VMs
```

### Resources
```
CPU:      #64C864  ████  Green
MEMORY:   #6496FF  ████  Blue
DISK:     #DC78FF  ████  Magenta
NETWORK:  #64DCDC  ████  Cyan
```

### Namespaces
```
Default:  #6496FF  ████  default namespace
System:   #FF6464  ████  kube-* namespaces
User:     #96FF96  ████  user namespaces
```

### OS (Templates)
```
Ubuntu:   #E95420  ████  Orange
Fedora:   #3369AD  ████  Blue
CentOS:   #9C429D  ████  Purple
Debian:   #D70A53  ████  Red
Windows:  #0078D4  ████  Blue
RHEL:     #EE0000  ████  Red
```

## ⚙️ Configuration

Create `~/.config/zorvia/tui.toml`:

```toml
[theme]
name = "default"

[ui]
show_splash = true              # Show splash screen
splash_duration_ms = 800        # Splash duration
show_stats_bar = true           # Show stats bar
default_view = "dashboard"      # Starting view
auto_refresh_interval = 5       # Auto-refresh (seconds)
table_style = "rounded"         # Table style

[behavior]
confirm_delete = true           # Confirm before delete
confirm_stop = false            # Confirm before stop
confirm_restart = false         # Confirm before restart
search_case_sensitive = false   # Search case sensitivity
search_regex = false            # Regex search mode
scroll_amount = 10              # Lines per page
max_list_items = 1000           # Max list items

[keybindings]
vim_mode = true                 # Enable hjkl navigation
quick_jump = true               # Enable Ctrl+P quick jump
```

See `examples/tui.toml` for full documentation.

## 🧪 Testing

### Compilation
```bash
cargo check
# ✅ Finished successfully
```

### Unit Tests
```bash
cargo test --lib tui
# ✅ 9 tests passed
#    - theme::tests (4 tests)
#    - config::tests (3 tests)
#    - colors::tests (2 tests)
```

### Visual Test
```bash
./scripts/test_theme.sh
# ✅ Displays all colors in terminal
```

## 📖 Documentation

| File | Lines | Description |
|------|-------|-------------|
| `THEME_DESIGN.md` | 600+ | Complete design specification |
| `THEME_IMPLEMENTATION.md` | 400+ | Implementation guide & API docs |
| `THEME_SUMMARY.md` | 250+ | High-level overview |
| `TUI_THEME_README.md` | This file | Quick start guide |

## 🎯 API Reference

### Theme Methods

```rust
let theme = Theme::default();

// Common styles
theme.title_style()       // Purple + Bold
theme.label_style()       // Light purple
theme.value_style()       // White + Bold
theme.border_style()      // Dark purple
theme.text_style()        // Primary text color
theme.muted_style()       // Dimmed text

// Status styles
theme.success_style()     // Green
theme.warning_style()     // Orange
theme.error_style()       // Red
theme.info_style()        // Blue

// Resource styles
theme.cpu_style()         // Green
theme.memory_style()      // Blue
theme.disk_style()        // Magenta
theme.network_style()     // Cyan

// Dynamic styles
theme.vm_status_style("running")           // Based on status
theme.vm_status_symbol("running")          // (symbol, style)
theme.namespace_style("default")           // Based on namespace
theme.resource_usage_style(75.0)           // Based on percentage
```

### CLI Color Helpers

```rust
use zorvia::tui::colors::cli;

// Text formatting
cli::header("Title")           // Purple + Bold
cli::label("Label:")           // White
cli::value("Value")            // White + Bold
cli::muted("(optional)")       // Dimmed

// VM specific
cli::vm_name("web-server")     // Bright white + Bold
cli::namespace("default")      // Blue (namespace-aware)
cli::vm_status("running")      // Green (status-aware)

// Messages
cli::success("Operation succeeded")  // ✓ Green
cli::error("Operation failed")       // ✗ Red
cli::warning("Resource warning")     // ⚠ Orange
cli::info("Information")             // ℹ Blue

// Resources
cli::resource("4", "cpu")      // Green
cli::resource("8Gi", "memory") // Blue

// Helpers
vm_status_symbol("running")    // ● (colored)
resource_bar(75.0, 20)         // [████████░░] (colored)
```

### Configuration API

```rust
use zorvia::tui::TuiConfig;

// Load configuration
let config = TuiConfig::load()?;                    // From default path
let config = TuiConfig::load_from_path(&path)?;     // From custom path

// Access settings
config.theme.name                   // "default"
config.ui.show_splash               // true
config.ui.default_view              // "dashboard"
config.behavior.confirm_delete      // true
config.keybindings.vim_mode         // true

// Save configuration
config.save()?;                     // To default path
config.save_to_path(&path)?;        // To custom path

// Get default config path
let path = TuiConfig::default_path()?;  // ~/.config/zorvia/tui.toml
```

## 🔮 Future Enhancements

### Phase 1: TUI Views ⬜
- Dashboard view with stats and charts
- VMs list view with table
- VM detail view
- Templates view
- Events view

### Phase 2: Interactivity ⬜
- Keyboard navigation (vim + arrows)
- Search functionality
- Modal dialogs
- Real-time updates
- Filter and sort

### Phase 3: Advanced ⬜
- Monitoring with graphs
- Multiple themes (dark, light)
- Custom theme editor
- Keybinding customization
- ASCII-only mode

## 🙏 Credits

**Inspired By:**
- **GuestKit** - Color system, config architecture, view patterns
- **Kubernetes** - Purple brand color
- **k9s** - Keyboard navigation, real-time updates
- **lazydocker** - Status indicators
- **Ratatui** - TUI framework

## 📄 License

Apache-2.0 (same as Zorvia)

## 📊 Statistics

- **Code**: 800+ lines (theme.rs + config.rs + colors.rs)
- **Documentation**: 1000+ lines (design + implementation + summary)
- **Tests**: 9 unit tests + 1 visual test
- **Dependencies**: 8 new (ratatui, crossterm, colored, owo-colors, toml, dirs, miette, rustyline)
- **Files**: 9 new files created
- **Status**: ✅ Complete and production-ready

---

**Ready to build beautiful TUIs for Zorvia! 🚀**

For more details:
- See `THEME_DESIGN.md` for the complete design specification
- See `THEME_IMPLEMENTATION.md` for implementation examples and best practices
- See `THEME_SUMMARY.md` for a high-level overview
