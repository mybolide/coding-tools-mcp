# 疑难解决方案

> 用户遇到问题时可以按本文档排查。每条记录包含现象、根因、处理步骤和验收方式。
>
> 返回索引：[README](../README.md) · [项目上下文](./project-context.md)

---

## macOS 反复弹出「想访问文稿」授权窗，点「允许」无效

### 现象

macOS 上持续弹出：

> 「Coding Tools MCP」想访问「文稿」文件夹中的文件。

用户点「允许」后，几秒内又弹，无限循环；严重时窗口卡死，点不掉也关不掉。

实测频率可达 24 小时内 1458 次授权请求，其中单个小时 953 次（约每 40 秒一次）。

### 根因

两层原因叠加，缺一不可。

**1. 发布包没有有效的 bundle 签名（主因）**

未签名的 App 点「允许」后，TCC 存下授权，但下次校验时找不到可匹配的
code requirement，判定「身份不符」，于是重新弹窗。用户怎么点都没用。

日志里表现为：

```
Failed to match existing code requirement for subject com.codingtools.mcp.desktop
and service kTCCServiceSystemPolicyDocumentsFolder
```

**2. 常驻子进程的工作目录落在受保护目录内**

`frpc` / `cloudflared` 等隧道进程会继承 App 作为 TCC 的 responsible process。
如果它们的工作目录（cwd）停在 `~/Documents` 下，就会持续轮询受保护目录，
把「一次授权」变成「无限授权」。

> **注意**：弹窗写的是 App 名，但真正读写文件、需要终止的往往是子进程。
> 必须看 TCC 日志里的 `pid` / `binary_path` 字段，不要只按 App 名判断。

### 排查步骤

#### 第一步：确认是谁在访问

```bash
# 找出可疑进程（父进程为 1 的孤儿进程尤其可疑）
ps -axo pid,ppid,etime,command | grep -iE "node|python|devcontrol|frpc|cloudflared" | grep -v grep

# 查 TCC 日志，看实际发起请求的进程和失败原因
/usr/bin/log show --last 30m --predicate 'subsystem == "com.apple.TCC"' --style compact \
  | grep -iE "AUTHREQ_PROMPTING|Failed to match"
```

> **必须用 `/usr/bin/log` 全路径。** shell 里 `log` 很可能被别名占用，会报
> "too many arguments"。

日志关键字段：

```
AUTHREQ_PROMPTING: service=kTCCServiceSystemPolicyDocumentsFolder,
  subject=Sub:{<bundle-id>}
  Resp:{TCCDProcess: identifier=..., pid=<PID>,
        responsible_path=<App 路径>, binary_path=<实际可执行文件>}
```

- `subject` = 弹窗显示的那个 App 身份
- `pid` / `binary_path` = **真正在读写文件、需要被终止的进程**

> 若看到 `Failed to match existing code requirement`，说明 App 没有有效签名，
> 直接看第四步。

#### 第二步：终止触发弹窗的进程

```bash
kill -TERM <PID1> <PID2>
sleep 3
ps -p <PID1>,<PID2> -o pid,stat,command   # 确认已退出
```

> **只杀日志里实际出现的 PID。不要无差别 `pkill node`**，会误杀编辑器、
> Claude Code、其他项目的 MCP 服务。

#### 第三步：清掉卡死的弹窗窗口

```bash
# 看前台 App 是不是 UserNotificationCenter
lsappinfo info -only name $(lsappinfo front)

# 列出可见窗口（无窗口 = 卡死的残影）
osascript -e 'tell application "System Events" to tell process "UserNotificationCenter" to get name of every window'

# 杀掉，它会按需自动重启，不影响系统
kill -9 <UserNotificationCenter 的 PID>
```

> `screencapture` 报 `could not create image from display` 通常是缺少
> 「屏幕录制」权限，不是显示器故障。改用 `lsappinfo` + AppleScript 判断即可。

#### 第四步：修复签名（根因）

先确认问题：

```bash
APP="/Applications/Coding Tools MCP.app"
codesign -v --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2>&1 | grep -E "Identifier|Signature|Info.plist|Sealed"
spctl -a -vv "$APP"
```

出现以下任一输出，说明没有有效 bundle 签名：

- `code object is not signed at all`
- `rejected, source=no usable signature`
- `Info.plist=not bound`
- `Sealed Resources=none`

**用户侧临时修复**（无需 Apple 账号）：

```bash
APP="/Applications/Coding Tools MCP.app"
xattr -dr com.apple.quarantine "$APP"              # 去掉下载隔离标记
codesign --force --deep --sign - "$APP"            # 本地 ad-hoc 签名
tccutil reset SystemPolicyDocumentsFolder com.codingtools.mcp.desktop
```

验收：

```bash
codesign -v --verbose=2 "$APP"
# 必须输出 "valid on disk" 和 "satisfies its Designated Requirement"
```

**开发侧正式修复**（v0.2.4 已完成）：

在 `src-tauri/tauri.conf.json` 启用签名，并让 CI 把签名校验作为构建门禁：

```json
"macOS": {
  "entitlements": "Entitlements.plist",
  "infoPlist": "Info.plist",
  "signingIdentity": "-",
  "hardenedRuntime": false
}
```

同时把 `frpc` / `cloudflared` 的 cwd 从工作区移到应用配置目录，并在应用退出时
终止全部隧道子进程（见 `src-tauri/src/tunnel/`）。

#### 第五步：验证

```bash
# 等 1 分钟后再查，应该为 0
/usr/bin/log show --last 2m \
  --predicate 'subsystem == "com.apple.TCC" AND eventMessage CONTAINS "kTCCServiceSystemPolicyDocumentsFolder"' \
  --style compact | grep -c AUTHREQ_PROMPTING

# 相关进程应为空
ps -axo pid,command | grep -iE "frpc|cloudflared" | grep -v grep
```

### 注意事项

1. 修复后**下次启动 App 会正常弹一次**，点「允许」即持久生效，不再反复弹。
2. ad-hoc 签名的身份是二进制指纹（cdhash），**每次重新构建或覆盖安装都会变**，
   升级后需重新授权一次。想彻底稳定，需要自签证书（仅本机）或
   Developer ID + 公证（对外分发）。
3. macOS 只保护「桌面 / 文稿 / 下载」三个目录。把工作区移到 `~/dev` 这类
   非受保护目录，可以完全绕开该问题，不需要任何签名或授权。
4. 这属于产品缺陷而非用户操作问题：发布包没签名，**所有用户都会中招**，
   应修在构建流程里，而不是让每个用户手动处理。

### 相关代码

| 位置 | 作用 |
| --- | --- |
| `src-tauri/tauri.conf.json` | macOS 签名配置 |
| `src-tauri/Info.plist` | 文件夹用途说明（`NSDocumentsFolderUsageDescription` 等） |
| `src-tauri/Entitlements.plist` | 权限声明 |
| `src-tauri/src/tunnel/frp/client.rs` | frpc 工作目录 |
| `src-tauri/src/tunnel/cloudflare.rs` | cloudflared 工作目录 |
| `src-tauri/src/tunnel/access.rs` | 退出清理与启动兜底清理 |
| `.github/workflows/macos-release.yml` | CI 签名门禁 |
