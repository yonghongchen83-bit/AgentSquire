# JSON Complexity Reduction — 格式对比

> 场景：用户说"帮我理解这段代码"，LLM 要创建两个概念 token、建立关系、保留一些内容。

---

## 🔴 当前方案 — 全 JSON 嵌套 (SquireResponse)

LLM 输出的**完整字符串**（这是网络传输的实际内容）：

```json
{
  "content": "我来分析这段代码。\n\n§^CODE_FLOW\n这段代码的核心流程是：\n1. 接收输入参数\n2. 验证格式\n3. 调用后端 API\n§^\n\n主要涉及 §!AUTH_MODULE 和 §!API_LAYER。\n\n§^KEY_ISSUE\n关键问题在于缺少错误处理。\n§^",
  "new_tokens": [
    {
      "id": "CODE_FLOW",
      "type": "concept",
      "short_desc": "代码流程分析",
      "full_desc": "这段代码的核心流程是：\n1. 接收输入参数\n2. 验证格式\n3. 调用后端 API"
    },
    {
      "id": "KEY_ISSUE",
      "type": "concept",
      "short_desc": "关键问题",
      "full_desc": "关键问题在于缺少错误处理"
    }
  ],
  "relationships": [
    {
      "subject": "CODE_FLOW",
      "predicate": "RespondsTo",
      "object": "USR_T1_001"
    },
    {
      "subject": "KEY_ISSUE",
      "predicate": "References",
      "object": "CODE_FLOW"
    }
  ],
  "preserve": [
    "AUTH_MODULE",
    "API_LAYER",
    "CODE_FLOW",
    "KEY_ISSUE"
  ],
  "ask_user": ""
}
```

**复杂度统计**：
- 总长度：~750 字符
- 嵌套层级：4 层 (root → new_tokens[0] → ranges)
- 数组数量：3 个 (new_tokens, relationships, preserve)
- 最容易出错的地方：`"full_desc"` 字段里的转义引号和换行符 \n，以及数组元素之间的逗号

**DeepSeek 常见错误**：
```
// 错误 1: 数组中漏了逗号
"new_tokens": [{"id": "CODE_FLOW" ...} {"id": "KEY_ISSUE" ...}]
//                                              ^ 缺逗号, JSON 解析失败

// 错误 2: full_desc 里的引号没转义
"full_desc": "核心是 "checkLogin" 函数"
                       ^ 裸引号, JSON 解析失败

// 错误 3: 花括号不匹配
"new_tokens": [{"id": "CODE_FLOW", "short_desc": "xxx"}
                                                         ^ 缺 ]} , JSON 解析失败

// 错误 4: 过早关闭根对象
{"content": "...", "new_tokens": [...]}
                                         ^ 之后又出来 "relationships": [...] 
```

---

## 🟢 方案 A: 纯 Sigil 协议（零 JSON）

**核心理念**：内容即协议。LLM 只输出文本 + 增强 sigil，后端解析器从 sigil 中提取所有元数据。

LLM 输出的**完整字符串**：

```text
§?你具体想理解哪部分？我先确认一下。?§

我来分析这段代码。

§^CODE_FLOW "代码流程分析"
这段代码的核心流程是：
1. 接收输入参数
2. 验证格式
3. 调用后端 API
§^

主要涉及 §!AUTH_MODULE 和 §!API_LAYER。

§^KEY_ISSUE "关键问题"
关键问题在于缺少错误处理。
§^

§@AUTH_MODULE,API_LAYER,CODE_FLOW,KEY_ISSUE§
```

**解析规则**（新增的 sigil）：

| Sigil | 含义 | 解析输出 |
|-------|------|---------|
| `§^TokenID "short_desc"` | 定义 token (short_desc 跟在 ID 后面) | `NewTokenSpec { id, short_desc, full_desc = 内容 }` |
| `§@ID1,ID2§` | 保留列表 | `preserve = [ID1, ID2]` |
| `§?text?§` | 询问用户 | `ask_user = "text"` |
| `§!ID` (已有) | 引用 token | 不变 |
| `§^TokenID ... §^` (已有) | token 定义 | full_desc = 内容 |

**关系推断**（无需 LLM 显式声明）：

| 规则 | 建议关系 |
|------|---------|
| `§!EXISTING_ID` 引用 | `ResponseToken RespondsTo ExistingToken` |
| `§^NEW_ID1 ... §!NEW_ID2 ... §^` 内引用 | `NEW_ID2 References NEW_ID1` |
| 新 token 存在 | `ResponseToken RespondsTo UserRequestToken` |

**复杂度统计**：
- 总长度：~400 字符（减少了 ~47%）
- 嵌套层级：**0 层**（纯文本）
- 数组数量：**0 个**
- ~~JSON 解析~~：**不需要！** 直接用 `protocol.rs` 的现有 sigil 解析器

**错误容错**：
```
// DeepSeek 漏了关闭 §^
§^CODE_FLOW "代码流程"
这段代码的核心流程...
§^ --- 忘记写了

// 结果：unclosed span → 丢弃该 token，其他内容正常显示
// 不会破坏整个响应！
```

---

## 🟢 方案 B: 扁平最小 JSON

**核心理念**：保留一层 JSON 外壳，但把嵌套数组全部拍平为分隔符字符串。

LLM 输出的**完整字符串**：

```json
{
  "content": "我来分析这段代码。\n\n§^CODE_FLOW\n这段代码的核心流程是：\n1. 接收输入参数\n2. 验证格式\n3. 调用后端 API\n§^\n\n主要涉及 §!AUTH_MODULE 和 §!API_LAYER。\n\n§^KEY_ISSUE\n关键问题在于缺少错误处理。\n§^",
  "new_tokens": "CODE_FLOW|concept|代码流程分析|这段代码的核心流程是：...\nKEY_ISSUE|concept|关键问题|关键问题在于缺少错误处理",
  "relationships": "CODE_FLOW|RespondsTo|USR_T1_001\nKEY_ISSUE|References|CODE_FLOW",
  "preserve": "AUTH_MODULE,API_LAYER,CODE_FLOW,KEY_ISSUE",
  "ask_user": ""
}
```

**复杂度统计**：
- 总长度：~550 字符（减少了 ~27%）
- 嵌套层级：**1 层**（只有根对象）
- 数组数量：**0 个**（全变成字符串）
- 关键改进：`new_tokens` 从数组对象 → 行分隔的字符串，每行是 `|` 分隔的字段

**为什么更容易**：
```
// 错误 1: full_desc 里的引号 → 没问题，因为这是字符串里的字符
// "这段代码的"核心"是 checkLogin"  ← 在字符串里不需要转义！因为 JSON 字符串天然支持引号

// 错误 2: 忘记一个 token → 不影响其他 token
new_tokens = "CODE_FLOW|concept|代码\nKEY_ISSUE|concept|关键问题"
              ^ 第二个 | 被换行打断了？这条单独解析失败，不影响 KEY_ISSUE

// 错误 3: 行分隔 → 某行坏了跳过该行
```

**解析代码极简**：
```rust
// 解析 new_tokens 字符串
for line in parsed.new_tokens.split('\n') {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 3 { continue; }  // 跳过损坏行
    let spec = NewTokenSpec {
        id: parts[0].trim().to_string(),
        token_type: parts.get(1).unwrap_or(&"concept").to_string(),
        short_desc: parts.get(2).unwrap_or(&"").to_string(),
        full_desc: parts.get(3).map(|s| s.to_string()),
        ..Default::default()
    };
    store.upsert_token(spec);
}

// 解析 relationships
for line in parsed.relationships.split('\n') {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 3 { continue; }
    store.add_relationship(Relationship {
        subject: parts[0].trim().to_string(),
        predicate: parts[1].trim().to_string(),
        object: parts[2].trim().to_string(),
    });
}

// 解析 preserve
let preserve: Vec<String> = parsed.preserve
    .split(',')
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .collect();
```

---

## 🟢 方案 C: 区段文本协议（零 JSON）

**核心理念**：纯文本格式，用区段分隔符划分。每个区段独立解析，容错极强。

LLM 输出的**完整字符串**：

```text
---CONTENT---
我来分析这段代码。

§^CODE_FLOW
这段代码的核心流程是：
1. 接收输入参数
2. 验证格式
3. 调用后端 API
§^

主要涉及 §!AUTH_MODULE 和 §!API_LAYER。

§^KEY_ISSUE
关键问题在于缺少错误处理。
§^
---NEW_TOKENS---
CODE_FLOW | concept | 代码流程分析 | 这段代码的核心流程是...
KEY_ISSUE | concept | 关键问题 | 关键问题在于缺少错误处理
---RELATIONSHIPS---
CODE_FLOW | RespondsTo | USR_T1_001
KEY_ISSUE | References | CODE_FLOW
---PRESERVE---
AUTH_MODULE, API_LAYER, CODE_FLOW, KEY_ISSUE
```

**复杂度统计**：
- 总长度：~500 字符
- 嵌套层级：**0 层**（纯文本）
- 数组数量：**0 个**
- 解析方式：按 `---SECTION---` 划分，每段按行/分隔符解析

**容错能力极强**：
```
// DeepSeek 在 ---NEW_TOKENS--- 区段中间插了一段废话：
---NEW_TOKENS---
CODE_FLOW | concept | 代码流程分析 | ...
顺便说一下这个 token 是这样的....
KEY_ISSUE | concept | 关键问题 | ...
                                             ^ 多了一行 "顺便说一下..."
// 结果：只有该行跳过，前后 token 正常解析

// DeepSeek 忘了 ---RELATIONSHIPS--- 区段：
// 结果：relationships 默认为空，不影响 content 和 tokens
```

**解析代码**：
```rust
fn parse_sections(text: &str) -> SquireResponse {
    let mut resp = SquireResponse::default();
    let mut current_section = String::new();
    
    for line in text.lines() {
        if line.starts_with("---") {
            current_section = line.trim_matches('-').to_uppercase();
            continue;
        }
        match current_section.as_str() {
            "CONTENT" => resp.content.push_str(line),
            "NEW_TOKENS" => {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 3 {
                    resp.new_tokens.push(NewTokenSpec { ... });
                } // 否则跳过该行
            }
            "RELATIONSHIPS" => {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 3 {
                    resp.relationships.push(Relationship { ... });
                }
            }
            "PRESERVE" => {
                for id in line.split(',') {
                    resp.preserve.push(id.trim().to_string());
                }
            }
            _ => {}
        }
    }
    resp
}
```

---

## 方案对比总表

| 指标 | 🔴 当前全JSON | 🟢 A.纯Sigil | 🟢 B.扁平JSON | 🟢 C.区段文本 |
|------|:----------:|:----------:|:----------:|:----------:|
| **LLM 输出大小** | ~750 字符 | **~400 字符** | ~550 字符 | ~500 字符 |
| **嵌套层级** | 4 层 | **0 层** | **1 层** | **0 层** |
| **数组数量** | 3 个 | **0 个** | **0 个** | **0 个** |
| **需要 JSON 解析** | ✅ 需要 | ❌ 不需要 | ✅ 简单（1层） | ❌ 不需要 |
| **新增代码量** | — | ~80 行 (sigil 增强) | ~60 行 (分隔符解析) | ~80 行 (区段解析) |
| **部署风险** | 基准 | 中（改 sigil 语法）| **低**（兼容现有流程）| 低（全新格式）|
| **关系表达能力** | 完整 | 需推断规则 | 完整 | 完整 |
| **ranges 支持** | ✅ | ⚠️ 需设计 | ⚠️ 需扩充分隔符 | ⚠️ 需扩充分隔符 |
| **错误容错** | ❌ 一错全丢 | ✅ 单 sigil 错误不影响 | ✅ 单行错误不影响 | ✅ 单区段错误不影响 |
| **渐进采用** | — | ⚠️ 需要改 system prompt | ✅ 可兼容旧格式 | ✅ 可兼容旧格式 |

---

## 我的推荐

> **短期优先 → 方案 B（扁平最小 JSON）**

理由：
1. **改动最小** — `SquireResponse` 结构体改 3 个字段类型，`adapter.rs::finalize_turn` 改解析方式，无需改 sigil 解析器
2. **风险最低** — 仍然是 JSON，system prompt 小改即可，新旧格式可并存
3. **解决核心问题** — `new_tokens: [...]` 这种数组 > 3 个元素时就频繁出错的场景被消除了
4. **可以逐步加强** — 先实现 B，再考虑是否往 A 方向走

> **长期方向 → 方案 C（区段文本）或方案 A（纯 Sigil）**

等验证了方案 B 确实减少了合规失败，再评估是否彻底摆脱 JSON。

---

## 方案 B 的具体改动范围

### Rust 改动

```rust
// === src-tauri/src/agent/squire/types.rs ===
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SquireResponse {
    pub ask_user: String,
    pub content: String,
    pub preserve: String,           // Vec<String> → String (逗号分隔)
    pub new_tokens: String,         // Vec<NewTokenSpec> → String (行/|分隔)
    pub relationships: String,      // Vec<Relationship> → String (行/|分隔)
}

// === src-tauri/src/agent/squire/adapter.rs ===
// 删掉 clean_deepseek_json 调用
// 删掉 validate_squire_response 调用（字符串解析不产生结构性错误）
// 改成调用 parse_new_tokens(), parse_relationships(), parse_preserve()

// === src-tauri/src/commands/utils.rs ===
// 删掉 safe-json-repair 依赖（或保留作回退）
```

### System Prompt 改动

```diff
- Always return exactly one JSON object with this structure:
- {
-   "ask_user": "",
-   "content": "",
-   "preserve": [],
-   "new_tokens": [],
-   "relationships": []
- }

+ Return a JSON object with flat string fields:
+ {
+   "content": "your response with §!/§^ sigils",
+   "new_tokens": "ID|type|short_desc|full_desc\nID|type|short_desc",
+   "relationships": "subj|pred|obj\nsubj|pred|obj",
+   "preserve": "ID1,ID2,ID3",
+   "ask_user": ""
+ }
+ 
+ For new_tokens: one token per line, fields separated by |
+ For relationships: one relationship per line, fields separated by |
+ For preserve: comma-separated token IDs
```

### 向后兼容

如果某个 LLM（如 GPT-4）能稳定输出完整 JSON，解析器可以同时支持两种格式：
```rust
fn parse_new_tokens(input: &str) -> Vec<NewTokenSpec> {
    // 先尝试解析为新格式（行+分隔符）
    if input.contains('|') {
        return parse_pipe_format(input);
    }
    // 回退：尝试解析为旧 JSON 数组格式
    serde_json::from_str(input).unwrap_or_default()
}
```
