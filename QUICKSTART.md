# 快速开始 (Quick Start)

## 1️⃣ 首次运行

```bash
cd OperationKernel
cargo run
```

程序会自动创建配置文件：`~/.config/ok/config.toml`

## 2️⃣ 配置 Claude API

### 获取 API Key

1. 访问 https://console.anthropic.com/
2. 登录并创建 API Key
3. 复制你的 API Key（格式：`sk-ant-api03-...`）

### 编辑配置文件

```bash
vim ~/.config/ok/config.toml
```

**修改这一行：**
```toml
api_key = "YOUR_API_KEY_HERE"  # ← 替换为你的真实 API Key
```

**完整配置示例：**
```toml
default_station = "claude"

[[stations]]
id = "claude"
name = "Claude 3.5 Sonnet"
provider = "anthropic"
api_key = "sk-ant-api03-xxxxxxxxxxxxxxxxxxxxx"  # 你的 API Key
api_base = "https://api.anthropic.com"
model = "claude-3-5-sonnet-20241022"
max_tokens = 8192
temperature = 1.0
```

## 3️⃣ 再次运行

```bash
cargo run
```

## 4️⃣ 使用说明

| 操作 | 按键 |
|------|------|
| 发送消息 | `Enter` |
| 换行 | `Shift+Enter` |
| 退出 | `Ctrl+C` |

## 🎯 当前功能 (Phase 1)

✅ 终端 TUI 界面
✅ 多行文本输入
✅ 回显功能（测试用）
⏳ LLM 集成（Phase 2 开发中）

## 📚 更多文档

- **详细配置**: [CONFIG.md](./CONFIG.md)
- **项目说明**: [README.md](./README.md)

## ⚠️ 安全提示

**不要把包含 API Key 的配置文件提交到 Git！**

```bash
# 设置正确的文件权限
chmod 600 ~/.config/ok/config.toml
```

## 🚀 下一步

Phase 2 将实现真实的 Claude API 集成，敬请期待！

当前是 Echo 模式，输入的消息会原样返回（用于测试 TUI）。
