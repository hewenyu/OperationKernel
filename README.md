# OperationKernel (OK Agent)

A Rust-based AI coding agent with Terminal User Interface (TUI).

## Current Phase: Phase 1 - Minimal TUI Echo Loop ✅

**Status:** Complete and working!

### What's Implemented

- ✅ **Ratatui + Crossterm TUI skeleton**
- ✅ **Split-screen layout** (chat history top, input bottom, status bar middle)
- ✅ **Multi-line text input** using tui-textarea
- ✅ **Event-driven architecture** with tokio async runtime
- ✅ **60 FPS rendering** (16ms tick interval)
- ✅ **Keyboard controls:**
  - Type multi-line messages
  - `Enter` to submit message
  - `Shift+Enter` to insert new line
  - `Ctrl+C` to quit gracefully
- ✅ **Echo functionality** - displays user input as "Echo: <message>"

### Project Structure

```
OperationKernel/
├── Cargo.toml           # Dependencies and build config
├── src/
│   ├── main.rs         # Entry point, event loop (109 lines)
│   ├── event.rs        # Event types (18 lines)
│   └── tui/
│       ├── mod.rs      # Module exports (4 lines)
│       ├── app.rs      # App state and rendering (160 lines)
│       └── input.rs    # Input widget wrapper (59 lines)
```

**Total:** 350 lines of Rust code

### How to Run

```bash
# Build the project
cargo build

# Run the application
cargo run
```

**Usage:**
1. Type your message (use `Shift+Enter` for multi-line)
2. Press `Enter` to submit
3. Watch it echo back!
4. Press `Ctrl+C` to quit

### Technical Highlights

- **Clean Architecture**: Separation of concerns (events, UI, app state)
- **Async-First**: Built on Tokio for future network operations
- **Type-Safe**: Zero unsafe code, leveraging Rust's type system
- **Efficient Rendering**: Only redraws on events, 60 FPS capable
- **Error Handling**: Proper error propagation with anyhow

### Dependencies

- `ratatui 0.28` - Terminal UI framework
- `crossterm 0.28` - Cross-platform terminal manipulation
- `tui-textarea 0.6` - Multi-line text input widget
- `tokio 1` - Async runtime
- `futures 0.3` - Async stream utilities
- `anyhow 1` - Error handling

### Next: Phase 2

Phase 2 will add:
- OpenAI API integration
- Real LLM responses (replacing echo)
- SSE stream parsing
- Token-by-token response streaming

### Success Criteria (Phase 1)

- [x] Compiles with zero errors ✅
- [x] Split screen layout ✅
- [x] Multi-line text input ✅
- [x] Ctrl+C quits gracefully ✅
- [x] 60 FPS rendering capability ✅

---

**Philosophy:** "先做了,再慢慢修复问题" (Build first, iterate later)
