# Codex 内部发布：Code Signing 问题排查与修复建议（macOS）

## 一、执行摘要

当前魔改版可以继续开发，但**不建议在默认启用 `computer-use` 插件状态下直接对公司内部大规模发布**。

核心阻塞：

1. `computer-use` 子进程在 macOS 上被系统直接杀掉（不是 OpenAI 账号登录问题）。
2. 崩溃根因是 Code Signing / Launch Constraint Violation。
3. 该问题在本机是长期重复发生，不是偶发。

此外还有两类发布前应收敛的问题：

- 历史出现过一次 `codex-cli` 本体 CODESIGNING 崩溃。
- 当前配置里 `notify` 指向已不存在的旧版本 computer-use 路径，持续报 warning。

---

## 二、魔改背景（基于 `Modifications.md`）

这个仓库是内部 fork，主要差异：

1. 认证：改为内部 SSO + cookie（不再依赖 OpenAI 登录语义触发）。
2. 分发命名：内部安装进程名是 `codex-cli`（上游 Rust 二进制仍是 `codex`）。
3. 配置目录：固定 `~/.codex`（小写）。
4. provider 仍参与运行时路由（不是废弃概念）。
5. 新增内部脚本：`xhs.sh`、`install-codex-cli.sh`。

结论：业务侧魔改合理，但发布时必须补齐 macOS 签名信任链治理。

---

## 三、问题列举（按优先级）

### P0（发布阻塞）：`computer-use` 无法启动

#### 现象

- 启动后出现：
  - `MCP client for 'computer-use' failed to start ... Broken pipe`
  - `MCP startup incomplete (failed: computer-use)`

#### 本机证据

- `~/Library/Logs/DiagnosticReports/SkyComputerUseClient-2026-05-03-113857.ips`
- `~/Library/Logs/DiagnosticReports/SkyComputerUseClient-2026-05-03-114348.ips`
- `~/Library/Logs/DiagnosticReports/SkyComputerUseClient-2026-05-03-114355.ips`

以上报告均显示：

- `signal: SIGKILL (Code Signature Invalid)`
- `namespace: CODESIGNING`
- `indicator: Launch Constraint Violation`

同时本机校验：

- `codesign --verify --deep --strict` 对以下对象均报 `invalid signature`：
  - `Codex Computer Use.app`
  - `SkyComputerUseClient`
  - `SkyComputerUseService`
- `spctl --assess` 返回 code signing subsystem error。

#### 结论

这不是登录问题，是系统级签名/启动约束校验失败导致子进程秒退，随后 MCP 握手阶段出现 Broken pipe。

---

### P0（持续性）：问题已长期存在

本机统计：

- `SkyComputerUseClient` 崩溃报告：current 3 + retired 46 = **49**

历史样本（retired）例如：

- `SkyComputerUseClient-2026-05-02-172718.ips`（build 758）

同样显示 `Code Signature Invalid`。

结论：不是新回归，而是长期未闭环问题。

---

### P1（历史高风险）：`codex-cli` 本体曾崩溃于 CODESIGNING

本机证据：

- `~/Library/Logs/DiagnosticReports/Retired/codex-cli-2026-04-28-122135.ips`

关键信息：

- `SIGKILL (Code Signature Invalid)`
- `namespace: CODESIGNING`
- `indicator: Invalid Page`
- 路径：`/usr/local/bin/codex-cli`

说明发布链路历史上存在本体签名一致性问题（即便当前文件已可验证）。

---

### P2（发布规范风险）：当前 `codex-cli` 签名形态不统一

本机当前：

- `/usr/local/bin/codex-cli` 为 ad-hoc 签名（`TeamIdentifier=not set`）

这不一定立即导致崩溃，但不符合“企业可重复分发、可审计”的理想标准。

---

### P3（中低风险）：`notify` 配置路径已失效

`~/.codex/config.toml` 里：

- `notify` 指向 `computer-use/1.0.758/.../SkyComputerUseClient`

而本机当前为 1.0.770，日志里出现：

- `after_agent hook failed ... No such file or directory`

---

### P4（低风险）：插件 manifest 规范告警

日志里有 `defaultPrompt` 长度/数量超限被忽略，不是崩溃根因，但建议清理。

---

## 四、崩溃报告摘要（可用于评审会）

### A. SkyComputerUseClient（近期）

- 时间：2026-05-03 11:38:57 / 11:43:48 / 11:43:55 (+0800)
- 进程：`SkyComputerUseClient`
- 父进程：`codex-cli`
- 信号：`SIGKILL (Code Signature Invalid)`
- 终止域：`CODESIGNING`
- 终止指示：`Launch Constraint Violation`

### B. codex-cli（历史）

- 时间：2026-04-28 12:21:35 (+0800)
- 进程：`/usr/local/bin/codex-cli`
- 信号：`SIGKILL (Code Signature Invalid)`
- 终止域：`CODESIGNING`
- 终止指示：`Invalid Page`

---

## 五、现状分析：为什么日志看到的是 Broken pipe

表面错误是 MCP Broken pipe，真实链路是：

1. Session 初始化时会启动启用的 MCP servers。
2. `computer-use` 走 stdio，拉起 `SkyComputerUseClient mcp`。
3. macOS 在进程启动阶段进行签名与约束检查。
4. 校验失败，系统 SIGKILL 子进程。
5. 上层发 initialize 请求时对端已死，表现为 Broken pipe。

---

## 六、修复建议（以“可发布”为目标）

## A. 立即止血（本周）

1. 发布版默认禁用 `computer-use@openai-bundled`。
2. 删除/修复 `notify` 中旧版本硬编码路径。
3. 在 UI 文案中将该类错误提示为“可能是 macOS 签名约束问题”，避免误导为账号问题。

## B. 短期（1~2 周）

1. 建立可重复的 macOS 签名流水线：固定证书、固定签名参数、固定产物校验。
2. 加入发布门禁：
   - `codesign --verify --deep --strict`
   - `spctl --assess --type execute`
3. 启动前健康检查 computer-use，失败自动降级禁用该 server，不阻塞主功能。

## C. 中期（发布前必须完成）

1. 主程序/插件/辅助程序签名策略统一。
2. bundled 插件入库前先验签，不把失败留给用户端。
3. 灰度发布：先小范围，观察 crash rate / startup failure rate 再放量。

---

## 七、macOS 签名体系小白指南（面向内部开发）

### 1）Code Signing 是什么？

给可执行文件加“身份 + 防篡改封条”。系统启动时会验证签名链与文件内容一致性。

### 2）ad-hoc vs Developer ID

- ad-hoc：本地哈希签名，无开发者身份（`TeamIdentifier` 为空）。
- Developer ID：有苹果信任链，适合企业分发。

建议：企业发布优先使用 Developer ID 体系。

### 3）Notarization（公证）

将产物提交 Apple 进行自动安全扫描并回传票据。对 Gatekeeper 通过率、用户体验很重要。

### 4）Hardened Runtime

更严格运行保护，现代 macOS 分发常见要求。

### 5）Entitlements

可执行权限声明（能力白名单）。配置错误会导致功能不可用或启动失败。

### 6）Launch Constraints（本次关键）

可执行可声明“允许哪些父进程/责任进程启动我”。不满足时系统直接杀进程。

### 7）为何会出现 `Code Signature Invalid`

常见触发：

- 签名后文件被改动（哪怕 1 字节）。
- 打包/解包破坏 bundle 完整性。
- 父子进程签名信任链不满足约束。
- 产物在发布链路中被替换或不一致。

---

## 八、建议的发布验收清单（可直接流程化）

1. 主程序签名与 `spctl` 评估通过。
2. 所有内置 helper/插件二进制签名与 `spctl` 评估通过。
3. 首次启动 smoke test：MCP 初始化无 fatal。
4. 若启用 computer-use：必须通过真实机器端到端启动测试。
5. 配置中不得存在版本硬编码失效路径。
6. 灰度阶段崩溃报告与启动失败率满足阈值后再全量。

---

## 九、当前建议决策

若要尽快提供稳定可用版本给内部用户：

- **推荐路径**：先发布“默认禁用 computer-use”的稳定版。
- 待签名链治理完成并验证通过后，再单独灰度开放 computer-use。

