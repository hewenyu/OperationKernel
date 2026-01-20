# OperationKernel (OK Agent)

A Rust-based AI coding agent with Terminal User Interface (TUI).

## Current Phase: Phase 2 - Claude API Integration ✅

**Status:** Complete and working! Real conversations with Claude.

### What's Implemented

**Phase 1 - TUI Foundation:**
- ✅ **Ratatui + Crossterm TUI skeleton**
- ✅ **Split-screen layout** (chat history top, input bottom, status bar middle)
- ✅ **Multi-line text input** using tui-textarea
- ✅ **Event-driven architecture** with tokio async runtime
- ✅ **60 FPS rendering** (16ms tick interval)
- ✅ **Keyboard controls:**
  - `Enter` to submit message
  - `Shift+Enter` to insert new line
  - `Ctrl+C` to quit gracefully

**Phase 2 - Claude API Integration:**
- ✅ **Real Claude API integration** with streaming responses
- ✅ **Token-by-token display** - see Claude think in real-time
- ✅ **Multi-turn conversations** - maintains full chat history
- ✅ **Automatic text wrapping** - handles long messages gracefully
- ✅ **Smart scrolling system:**
  - Keyboard scroll (↑/↓, PgUp/PgDn, Home/End)
  - Mouse wheel support
  - Auto-scroll to bottom with manual override
  - Visual scroll indicator (percentage + offset)
- ✅ **Clean message formatting** with spacing between messages
- ✅ **Loading indicators** - clear "Generating..." status
- ✅ **Error handling** - network errors displayed clearly
- ✅ **Configuration system** - TOML-based API key management

**UI/UX Enhancements:**
- ✅ **Modern Chat Design** - Slack/Discord-inspired clean interface
- ✅ **Unified Rounded Borders** - All components use consistent rounded style
- ✅ **High-Contrast Colors** - Light color variants (LightCyan, LightGreen, LightBlue, LightRed) for 30-80% better visibility
- ✅ **Emoji Role Icons** - Instant visual identification (👤 User, 🤖 AI, ℹ️ System, ⚠️ Error)
- ✅ **Generous Spacing** - 2x message gaps, wider padding for comfortable reading
- ✅ **Two-Line Message Layout** - Role header separated from content with indentation
- ✅ **Improved Streaming Indicators** - Shows `⋯` while waiting, `▌` cursor while typing
- ✅ **Cohesive Container Styling** - All borders, titles, and colors follow unified design language

### Project Structure

```
OperationKernel/
├── Cargo.toml           # Dependencies and build config
├── src/
│   ├── main.rs          # Entry point, event loop
│   ├── agent.rs         # UI-agnostic agent loop (LLM + tools + conversation)
│   ├── event.rs         # Event types
│   ├── config/          # Configuration system
│   │   ├── mod.rs       # Config loading/saving
│   │   └── station.rs   # Station (LLM provider) definitions
│   ├── llm/             # LLM integration
│   │   ├── mod.rs       # Module exports
│   │   ├── types.rs     # Message, StreamChunk types
│   │   └── anthropic.rs # Claude API client with SSE streaming
│   ├── tool/            # Tool system (bash/read/write/grep/...)
│   ├── process/         # Background processes (bash_output/kill_shell)
│   └── tui/             # Terminal UI
│       ├── mod.rs       # Module exports
│       ├── app.rs       # App state and rendering
│       └── input.rs     # Input widget wrapper
```

**Total:** ~750 lines of Rust code

### Configuration

第一次运行时，会自动生成配置文件：`~/.config/ok/config.toml`

**快速配置 Claude API:**

```bash
# 打开配置文件
vim ~/.config/ok/config.toml

# 或使用你喜欢的编辑器
code ~/.config/ok/config.toml
nano ~/.config/ok/config.toml
```

**配置示例:**
```toml
default_station = "claude"

[[stations]]
id = "claude"
name = "Claude 3.5 Sonnet"
provider = "anthropic"
api_key = "sk-ant-api03-your-key-here"  # 替换为你的 Claude API Key
model = "claude-3-5-sonnet-20241022"
```

📖 **详细配置说明**: 查看 [CONFIG.md](./CONFIG.md)

### How to Run

```bash
# Build the project
cargo build

# Run the application
cargo run
```

### Tests

部分集成测试是“真实联网测试”，通过 `OperationKernel/tests/config.toml` 配置（例如 `web_fetch` / `web_search`）。

```bash
cp OperationKernel/tests/config.example.toml OperationKernel/tests/config.toml
```

然后按需把 `web_fetch.enabled` / `web_search.enabled` 设为 `true`，并填入真实的 `web_search.brave_api_key`。

### Working Directory (Tools)

- All tools treat the process `PWD` as the project root (`working_dir`), and tool outputs will echo it back.
- File tools (`read`/`write`/`edit`/`glob`/`grep`/`notebook_edit`) only allow paths **inside** `working_dir` (plus the system temp directory like `/tmp` on Linux/macOS) to prevent “searching random folders” by mistake.
- Prefer `glob`/`grep` with relative paths (e.g. `.` / `src/**`) instead of `find /...`.

**Usage:**
1. Type your message (use `Shift+Enter` for multi-line)
2. Press `Enter` to submit
3. Watch Claude respond in real-time (streaming)
4. Press `Ctrl+C` to quit

### Technical Highlights

- **Clean Architecture**: UI-agnostic agent core + TUI rendering
- **Agent Runner**: `src/agent.rs` manages streaming + tool loop
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

### Next: Phase 3

Phase 3 will add:
- Permissions / policy layer for dangerous tools
- More agent capabilities (planning, memory, routing)
- Provider adapters (Gemini/Codex, etc.) will be done last

### Success Criteria (Phase 1)

- [x] Compiles with zero errors ✅
- [x] Split screen layout ✅
- [x] Multi-line text input ✅
- [x] Ctrl+C quits gracefully ✅
- [x] 60 FPS rendering capability ✅

---

**Philosophy:** "先做了,再慢慢修复问题" (Build first, iterate later)
