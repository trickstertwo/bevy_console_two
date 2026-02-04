# bevy_console Design Document

A minimal, extensible developer console for Bevy inspired by the [Source Engine ConVar system](https://wiki.alliedmods.net/ConVars_(SourceMod_Scripting)).

## Vision

Build a **long-lasting**, **minimal**, and **highly extensible** developer console that:
- Uses the least dependencies possible
- Employs best-in-class algorithms
- Supports configuration via RON
- Follows Bevy's ECS patterns natively

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    UI LAYER (feature-gated)                 │
│                                                             │
│   ┌─────────────────────┐    ┌─────────────────────────┐   │
│   │  feature: egui      │    │  Headless (no feature)  │   │
│   │  - Console window   │    │  - Events only          │   │
│   │  - Log display      │    │  - External tools hook  │   │
│   │  - Autocomplete     │    │  - Testing/CI           │   │
│   └─────────────────────┘    └─────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 CORE LAYER (always included)                │
│                   Zero optional dependencies                │
│                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐│
│  │   ConVar     │ │  ConCommand  │ │  ConsoleRegistry     ││
│  │ - Typed vals │ │ - Handlers   │ │  - Trie O(k) lookup  ││
│  │ - Constraints│ │ - Flags      │ │  - Prefix iteration  ││
│  │ - Flags      │ │              │ │                      ││
│  └──────────────┘ └──────────────┘ └──────────────────────┘│
│                                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐│
│  │CommandHandlers│ │  Tokenizer  │ │   Fuzzy Matcher      ││
│  │ - Separated  │ │ - Quoted str │ │  - Subsequence       ││
│  │   from meta  │ │ - Semicolons │ │  - Scoring           ││
│  └──────────────┘ └──────────────┘ └──────────────────────┘│
│                                                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  Events: Input, Output, ConVarChanged, Toggle, Clear   ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              PERSISTENCE LAYER (feature: persist)           │
│                                                             │
│   - Load/save ARCHIVE convars to RON                        │
│   - exec <file> command                                     │
│   - host_writeconfig command                                │
└─────────────────────────────────────────────────────────────┘
```

### Feature Matrix

| Feature | Dependencies Added | Use Case |
|---------|-------------------|----------|
| (none) | bevy (minimal) | Headless, external UI, testing |
| `egui` (default) | bevy_egui, tracing | Quick setup, dev tools |
| `persist` | ron, serde | Save/load config files |

---

## Core Concepts

### 1. ConVar (Console Variable)

A named variable with typed value, constraints, and flags:

```rust
// Registration
registry.register_var(
    ConVar::new("sv_gravity", 800.0f32)
        .description("World gravity")
        .flags(ConVarFlags::ARCHIVE)
        .min(0.0)
        .max(10000.0)
);

// Access
let gravity: f32 = registry.get("sv_gravity").unwrap_or(800.0);

// Modify
registry.set("sv_gravity", 1000.0f32);
```

**Flags** (inspired by Source Engine FCVAR_*):

| Flag | Description |
|------|-------------|
| `ARCHIVE` | Persists to config file |
| `CHEAT` | Requires sv_cheats to modify |
| `READ_ONLY` | Cannot be modified at runtime |
| `HIDDEN` | Hidden from autocomplete/listing |
| `NOTIFY` | Triggers notification on change |
| `DEV_ONLY` | Development only |

### 2. ConCommand (Console Command)

A named command that executes a handler function:

```rust
register_cmd(&mut registry, &mut handlers,
    ConCommand::new("quit", |_args, world| {
        world.send_event(AppExit::default());
    })
    .description("Exit the game")
    .flags(ConVarFlags::NONE)
);
```

### 3. ConsoleRegistry + CommandHandlers

**Key Architecture Decision**: Command handlers are stored separately from command metadata to avoid borrow conflicts.

```rust
// ConsoleRegistry stores metadata (name, description, flags)
// CommandHandlers stores the actual handler closures

// This allows handlers to access ConsoleRegistry during execution:
fn my_command(args: &CommandArgs, world: &mut World) {
    let registry = world.resource::<ConsoleRegistry>();
    // Can read registry here without borrow conflict!
}
```

### 4. Command Execution Pipeline

Three-stage pipeline to handle borrow checker constraints:

```
┌────────────────────┐     ┌─────────────────────┐     ┌──────────────────┐
│ parse_console_input│ ──▶ │execute_pending_cmds │ ──▶ │send_pending_output│
│                    │     │    (exclusive)      │     │                  │
│ - Read input events│     │ - Take handler      │     │ - Send output    │
│ - Tokenize         │     │ - Execute           │     │   events         │
│ - Queue commands   │     │ - Put handler back  │     │ - Send change    │
└────────────────────┘     └─────────────────────┘     │   events         │
                                                       └──────────────────┘
```

### 5. Tokenizer

Simple space-separated tokenizer with quoted string support:

```
sv_gravity 800              → ["sv_gravity", "800"]
echo "hello world"          → ["echo", "hello world"]
cmd1; cmd2; cmd3            → splits into 3 commands
echo test // comment        → ["echo", "test"]
```

### 6. Fuzzy Matcher

Zero-dependency subsequence matcher for autocomplete:

```rust
// "sgr" matches "sv_gravity" at positions [0, 3, 4]
// Scoring: consecutive bonus (+10), word start bonus (+5), prefix bonus (+20)
subsequence_match("sgr", "sv_gravity") // Some(MatchResult { score: 28, indices: [0,3,4] })
```

---

## Built-in Commands

| Command | Description |
|---------|-------------|
| `help [cmd]` | Show help or list all commands |
| `find <term>` | Search by name or description |
| `cvarlist [prefix]` | List all convars |
| `differences` | Show non-default values |
| `echo <text>` | Print text to console |
| `clear` | Clear console output |
| `toggle <cvar>` | Toggle boolean/integer convar |
| `reset <cvar>` | Reset to default value |

---

## Plugin Setup

```rust
use bevy::prelude::*;
use bevy_console::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ConsolePlugin)
        .add_systems(Startup, setup_console)
        .run();
}

fn setup_console(
    mut registry: ResMut<ConsoleRegistry>,
    mut handlers: ResMut<CommandHandlers>,
) {
    // Register a convar
    registry.register_var(
        ConVar::new("sv_gravity", 800.0f32)
            .description("World gravity")
            .flags(ConVarFlags::ARCHIVE)
    );

    // Register a command (use helper function)
    register_cmd(&mut registry, &mut handlers,
        ConCommand::new("noclip", |_args, world| {
            // Toggle noclip...
        })
        .description("Toggle noclip mode")
        .flags(ConVarFlags::CHEAT)
    );
}
```

### Reacting to ConVar Changes

```rust
fn on_gravity_change(mut events: MessageReader<ConVarChangedEvent>) {
    for event in events.read() {
        if &*event.name == "sv_gravity" {
            info!("Gravity changed: {} -> {}", event.old_value, event.new_value);
        }
    }
}
```

### Programmatic Console Input

```rust
fn trigger_command(mut events: MessageWriter<ConsoleInputEvent>) {
    events.write(ConsoleInputEvent::new("sv_cheats 1; noclip"));
}
```

---

## File Structure

```
bevy_console/
├── Cargo.toml
├── DESIGN.md
├── CLAUDE.md
├── src/
│   ├── lib.rs              # Plugin, exports, built-in commands
│   │
│   ├── core/               # Zero optional dependencies
│   │   ├── mod.rs
│   │   ├── convar.rs       # ConVar<T>, ConVarFlags, ConVarValue
│   │   ├── concommand.rs   # ConCommand, ConCommandMeta, CommandArgs
│   │   ├── registry.rs     # ConsoleRegistry, CommandHandlers, ConEntry
│   │   ├── trie.rs         # Trie for O(k) lookup
│   │   ├── matcher.rs      # Fuzzy subsequence matcher
│   │   ├── tokenizer.rs    # Command tokenizer
│   │   └── events.rs       # All event types
│   │
│   ├── persist/            # feature: persist - RON config, aliases
│   │   └── mod.rs          # ConsoleConfigFile, CommandAliases, load/save
│   │
│   ├── config.rs           # feature: egui - ConsoleConfig, ConsoleTheme
│   ├── logging.rs          # feature: egui - Log capture via tracing
│   └── ui/                 # feature: egui - egui UI rendering
│       ├── mod.rs
│       └── completions.rs
```

---

## Roadmap

### Milestone 1: Core Foundation ✅ COMPLETE

- [x] ConVar with typed values, constraints, flags
- [x] ConCommand with handlers
- [x] ConsoleRegistry with trie-based lookup
- [x] CommandHandlers (separated handler storage)
- [x] Fuzzy subsequence matcher
- [x] Tokenizer with quoted strings, semicolons
- [x] Events (Input, Output, Changed, Toggle, Clear)
- [x] Built-in commands (help, cvarlist, find, echo, clear, toggle, reset, differences)
- [x] ConsolePlugin
- [x] Integration tests

### Milestone 2: Persistence (feature: `persist`) ✅ COMPLETE

- [x] RON config format
- [x] Load ARCHIVE convars on startup
- [x] `exec <file>` command
- [x] `host_writeconfig` command
- [x] `alias` / `unalias` commands

### Milestone 3: egui Polish (feature: `egui`) ✅ COMPLETE

- [x] History navigation (up/down arrows)
- [x] Autocomplete: Tab or ArrowRight to accept
- [x] Log level filtering (checkboxes for each level)
- [x] Copy log line on click

### Milestone 4: Documentation & Release

- [x] Comprehensive rustdoc
- [x] Example: minimal (headless)
- [x] Example: basic (egui with convars)
- [x] Example: game (realistic use case)
- [x] README with quick start
- [ ] Publish to crates.io
- [x] Best in class implementation - lean code no duplicates, very good code - no ai hallucination shit, verified working implementations and
- [x] Best in class testing suite - no duplicate or weird test, proper testing -> all codepaths are tested, which should be tested

---

## Explicitly Out of Scope

These features are intentionally not planned:

| Feature                          | Reason                                               |
|----------------------------------|------------------------------------------------------|
| `bevy_ui` backend                | Massive effort, egui is sufficient for dev tools     |
| `convar!` / `concommand!` macros | Builder pattern is ergonomic enough                  |
| `ConVarHandle` (cached access)   | Premature optimization                               |
| `CommandParser` trait            | Simple tokenizer covers 95% of use cases             |
| Full expression parser           | Removed - added too much complexity and dependencies |

If you need these features, contributions are welcome, but they're not on the roadmap.

---

## Design Principles

1. **Minimal by default** - Core works with zero optional dependencies
2. **Opt-in complexity** - Advanced features behind feature flags
3. **Bevy-native** - ECS patterns, proper scheduling
4. **Predictable performance** - O(k) lookup, O(k+m) autocomplete
5. **Source Engine familiar** - ConVar/ConCommand API style

---

## Key Architectural Decisions

### Why CommandHandlers is Separate

Command handlers need `&mut World` to do useful work (access resources, send events). But if handlers were stored in `ConsoleRegistry` (a World resource), we'd have a borrow conflict:

```rust
// This doesn't work:
let registry = world.resource::<ConsoleRegistry>();
let cmd = registry.get_command("help");
cmd.execute(world); // Error: can't borrow world mutably while registry is borrowed
```

Solution: Store handlers in a separate `CommandHandlers` resource. During execution, we temporarily take the handler out, execute it, then put it back.

### Why Three-Stage Pipeline

1. **parse_console_input** - Regular system, reads events, tokenizes, queues
2. **execute_pending_commands** - Exclusive system with `&mut World`, can take/put handlers
3. **send_pending_outputs** - Regular system, sends output events

This allows maximum flexibility for command handlers while working within Rust's borrow rules.

### Why No Macros

The design originally proposed `convar!` and `concommand!` macros. We dropped them because:

1. Builder pattern is already ergonomic
2. Macros add complexity and debugging difficulty
3. IDE support is worse for macro-generated code
4. No compelling advantage for the added complexity

---

## References

- [Source Engine ConVar](https://developer.valvesoftware.com/wiki/ConVar)
- [AlliedModders ConVar Documentation](https://wiki.alliedmods.net/ConVars_(SourceMod_Scripting))
- [Developer Console Control](https://developer.valvesoftware.com/wiki/Developer_Console_Control)
