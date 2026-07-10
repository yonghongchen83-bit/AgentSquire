# Bookmark 协议 — 零配对、零引号、零逗号

## 设计原理

所有结构都遵循 **bookmark 原则**：只有"打开"，没有"关闭"。

- `§^name§^` — 开就完了，隐式结束
- `§#keyword` 区段标题 — 写出来就开始了，下个 `§#` 标题或文末自动结束
- 每行数据 — 独立存在，行坏了不影响其他行

**零配对要求**：不需要 `"`、`{`、`[`、`(`、`</tag>` 这些东西。

**区段分隔**：使用 `§#` 前缀作为区段标记。

- `§#` 不会被内容和现有 sigil 系统混淆：
  - `§!` = 引用已有 token（内容中）
  - `§^` = span 定义（内容中）
  - `§#` = 元数据区段（新增，内容中不会自然出现）
- 不需要空行分隔，跨越多行空行也无所谓

---

## 对比：同一场景的格式

> 场景：用户说"帮我理解这段代码"

### 🔴 当前 JSON（大量配对）

```json
{
  "content": "我来分析这段代码。\n\n§^CODE_FLOW\n核心流程...\n§^\n\n主要涉及 §!AUTH_MODULE。\n\n§^KEY_ISSUE\n缺少错误处理。\n§^",
  "new_tokens": [
    {
      "id": "CODE_FLOW",
      "type": "concept",
      "short_desc": "代码流程"
    }
  ],
  "relationships": [
    {
      "subject": "CODE_FLOW",
      "predicate": "RespondsTo",
      "object": "USR_T1_001"
    }
  ],
  "preserve": ["CODE_FLOW"],
  "ask_user": ""
}
```

配对统计：`{`×2 `}`×2 `[`×2 `]`×2 `"`×18 `,`×7

### 🟢 Bookmark 协议（零配对）

```
我来分析这段代码。

§^CODE_FLOW
核心流程是：
1. 接收输入参数
2. 验证格式
3. 调用后端 API
§^

主要涉及 §!AUTH_MODULE 和 §!API_LAYER。

§^KEY_ISSUE
关键问题在于缺少错误处理。
§^

§#new_tokens
CODE_FLOW | concept | 代码流程
KEY_ISSUE | concept | 关键问题

§#relationships
CODE_FLOW | RespondsTo | USR_T1_001
KEY_ISSUE | References | CODE_FLOW

§#preserve
AUTH_MODULE
API_LAYER
CODE_FLOW
KEY_ISSUE
```

配对统计：**0 个！**
需要 `|` 作为字段分隔符，但 `|` 不需要配对。

---

## 格式规范

```
[内容文本 — 自然段落，可以用 §! 和 §^...§^ 符号]
[可以有任意空行和格式]

§#new_tokens
token_id | type | short_desc | full_desc(可选)
token_id | type | short_desc
...

§#relationships
subject | predicate | object
...

§#preserve
token_id
...

§#ask_user
提问文本（出现这个区段时，内容区段被忽略，ask_user 后面的内容是问题文本）
```

### 规则

1. **内容** = 第一个 `§#` 区段标题行之前的所有文本（含 sigil）
2. **区段**以 `§#keyword` 独占一行开始，以下一个 `§#` 行或文末结束
3. 区段内的空行被忽略（数据行之间可以有间距）
4. 区段顺序随意，不出现的区段默认为空
5. `§#` 是纯元数据前缀 — 与内容中已有的 `§!`（引用）、`§^`（span）不冲突

---

## 解析伪代码

```rust
const SECTION_KEYS: &[&str] = &["new_tokens", "relationships", "preserve", "ask_user"];

fn is_section_header(line: &str) -> Option<&'static str> {
    let line = line.trim();
    for key in SECTION_KEYS {
        if line == &format!("§#{}", key) {
            return Some(key);
        }
    }
    None
}

fn parse_bookmark_protocol(text: &str) -> SquireResponse {
    let mut resp = SquireResponse::default();
    let mut content_lines: Vec<&str> = Vec::new();
    let mut current_section: Option<&str> = None;

    for line in text.lines() {
        if let Some(key) = is_section_header(line) {
            current_section = Some(key);
            continue;
        }

        match current_section {
            None => {
                // Still in content — before first section marker
                content_lines.push(line);
            }
            Some("new_tokens") => {
                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }
                let parts: Vec<&str> = trimmed.split('|').collect();
                if parts.len() >= 3 {
                    resp.new_tokens.push(NewTokenSpec {
                        id: parts[0].trim().to_string(),
                        token_type: parts[1].trim().to_string(),
                        short_desc: parts[2].trim().to_string(),
                        full_desc: parts.get(3).map(|s| s.trim().to_string()),
                        ..Default::default()
                    });
                }
            }
            Some("relationships") => {
                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }
                let parts: Vec<&str> = trimmed.split('|').collect();
                if parts.len() >= 3 {
                    resp.relationships.push(Relationship {
                        subject: parts[0].trim().to_string(),
                        predicate: parts[1].trim().to_string(),
                        object: parts[2].trim().to_string(),
                    });
                }
            }
            Some("preserve") => {
                let id = line.trim();
                if !id.is_empty() {
                    resp.preserve.push(id.to_string());
                }
            }
            Some("ask_user") => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    resp.ask_user.push_str(trimmed);
                    resp.ask_user.push('\n');
                }
            }
            _ => {}
        }
    }

    resp.content = content_lines.join("\n").trim().to_string();
    resp.ask_user = resp.ask_user.trim().to_string();
    resp
}
```

---

## 和当前代码的集成

### 改动范围

```diff
// === tools.rs — 不再需要 built_in_tool_definitions ===
// （如果走纯内容协议，Squire 工具可以保留）

// === adapter.rs — finalize_turn ===
- let cleaned = clean_deepseek_json(assistant_content.trim());
- let parsed: SquireResponse = match serde_json::from_str(&cleaned) { ... };
+ let parsed = parse_bookmark_protocol(&assistant_content);

// 后续流程完全不变，parsed 仍然是 SquireResponse：
// parsed.content → expand_for_display
// parsed.new_tokens → upsert_token
// parsed.relationships → add_relationship
// parsed.preserve → set_preserve_list
// parsed.ask_user → AskUser

// === utils.rs — 删掉 safe-json-repair 依赖 ===
- pub fn clean_deepseek_json(raw: &str) -> String { ... }

// === adapter.rs — 删掉校验（解析器已天然容错）===
- validate_squire_response(&parsed, ...)
```

### 向后兼容

如果某个模型输出了旧版 JSON，可以检测并走回退：

```rust
fn detect_and_parse(text: &str) -> SquireResponse {
    if text.trim().starts_with('{') {
        // 旧版 JSON 回退
        let cleaned = clean_deepseek_json(text);
        serde_json::from_str(&cleaned).unwrap_or_default()
    } else {
        // 新版 bookmark 协议
        parse_bookmark_protocol(text)
    }
}
```

---

## 为什么这个格式 DeepSeek 搞不错

| 元素                          | DeepSeek 记忆负担                             | 为什么             |
| ----------------------------- | --------------------------------------------- | ------------------ |
| `"key"`                     | 🔴 高 — 记不住配对的引号                     | **不出现**   |
| `,` 分隔                    | 🔴 高 — 不知道哪个元素是最后一个             | **不出现**   |
| `{` `}`                   | 🔴 中 — 忘记关闭                             | **不出现**   |
| `[` `]`                   | 🔴 中 — 忘记关闭                             | **不出现**   |
| `§#new_tokens`（区段标题） | 🟢 极低 — 写一行就行，关不关不用管           | **新增**     |
| `§!ID`                     | 🟢 极低 — 单个符号                           | **保留**     |
| `§^ID ... §^`             | 🟢 极低 — 上下文自然                         | **保留**     |
| 换行                          | 🟢 自然                                       | **保留**     |
| `\|` 分隔                    | 🟢 很低 — 不需要配对                         | **新增**     |
| `§#` vs `§^` 混淆风险   | 🟢 无 —`#` 和 `^` 视觉差异大，且用途不同 | **设计保证** |

和人类写草稿的习惯一模一样。没有语法可以"错"。
