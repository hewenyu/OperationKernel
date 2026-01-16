# 配置指南 (Configuration Guide)

## 配置文件位置

第一次运行 `ok` 时，会自动在以下位置创建配置文件：

```
~/.config/ok/config.toml
```

## 配置文件格式

配置文件使用 TOML 格式，支持多个 LLM "站点"（stations）。

### 基础配置示例

```toml
default_station = "claude"

[[stations]]
id = "claude"
name = "Claude 3.5 Sonnet"
provider = "anthropic"
api_key = "YOUR_API_KEY_HERE"
api_base = "https://api.anthropic.com"
model = "claude-3-5-sonnet-20241022"
max_tokens = 8192
temperature = 1.0
```

## 字段说明

### 全局配置

- **`default_station`** (必填): 默认使用的站点 ID

### 站点配置 (`[[stations]]`)

每个站点代表一个 LLM 配置，你可以配置多个站点。

#### 必填字段

- **`id`**: 站点唯一标识符（自定义，用于命令行选择）
- **`name`**: 站点显示名称
- **`provider`**: 提供商类型
  - `"anthropic"` - Claude API
  - `"openai"` - OpenAI API
  - `"gemini"` - Google Gemini API
- **`api_key`**: API 密钥
- **`model`**: 模型名称

#### 可选字段

- **`api_base`**: 自定义 API 端点（例如代理或自托管服务）
- **`max_tokens`**: 最大生成 token 数（默认 8192）
- **`temperature`**: 温度参数 0.0-1.0（默认 1.0）

## Claude API 配置

### 获取 API Key

1. 访问 [Anthropic Console](https://console.anthropic.com/)
2. 登录或注册账户
3. 进入 "API Keys" 页面
4. 创建新的 API Key
5. 复制 API Key 到配置文件的 `api_key` 字段

### Claude 模型列表

```toml
# Claude 3.5 Sonnet (推荐 - 最强性能)
model = "claude-3-5-sonnet-20241022"

# Claude 3.5 Haiku (快速响应)
model = "claude-3-5-haiku-20241022"

# Claude 3 Opus (旧版最强)
model = "claude-3-opus-20240229"
```

### 完整 Claude 配置示例

```toml
default_station = "claude"

[[stations]]
id = "claude"
name = "Claude 3.5 Sonnet"
provider = "anthropic"
api_key = "sk-ant-api03-xxxxxxxxxxxxxxxxxxxxx"  # 替换为你的真实 API Key
api_base = "https://api.anthropic.com"
model = "claude-3-5-sonnet-20241022"
max_tokens = 8192
temperature = 1.0
```

## 自定义 API 端点

如果你使用代理或自托管的 Claude 兼容服务：

```toml
[[stations]]
id = "claude-proxy"
name = "Claude via Proxy"
provider = "anthropic"
api_key = "your-api-key"
api_base = "https://your-proxy.com/v1"  # 自定义端点
model = "claude-3-5-sonnet-20241022"
```

## 多站点配置示例

你可以配置多个站点，用于不同场景：

```toml
default_station = "claude-fast"

# 快速响应站点
[[stations]]
id = "claude-fast"
name = "Claude Haiku (Fast)"
provider = "anthropic"
api_key = "sk-ant-api03-xxxxx"
model = "claude-3-5-haiku-20241022"
max_tokens = 4096
temperature = 0.7

# 高质量站点
[[stations]]
id = "claude-quality"
name = "Claude Sonnet (Quality)"
provider = "anthropic"
api_key = "sk-ant-api03-xxxxx"
model = "claude-3-5-sonnet-20241022"
max_tokens = 8192
temperature = 1.0
```

运行时切换站点：
```bash
ok --station claude-quality
```

## 验证配置

编辑配置文件后，运行 `ok` 会自动加载配置。如果配置有误，会显示错误信息。

## 安全注意事项

⚠️ **重要提示**:

1. **不要把 API Key 提交到 Git 仓库！**
2. 配置文件包含敏感信息，权限应设为 `600`：
   ```bash
   chmod 600 ~/.config/ok/config.toml
   ```
3. 定期轮换 API Key
4. 不要与他人共享包含 API Key 的配置文件

## 下一步

配置完成后，运行：

```bash
cargo run
```

开始使用 OperationKernel！

---

**当前状态**: Phase 1 完成，Phase 2 (LLM 集成) 准备中
