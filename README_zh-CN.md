# Trucli

Trucli 是实验性的 Windows 命令行工具，用于协调官方 aTrust 客户端与
Mihomo 校园资源分流规则。

**v0.1.0-alpha.1 · Windows 10/11 x86-64 · Experimental / Alpha**

[English（规范文档）](README.md) · [发布页](https://github.com/styayur/trucli/releases)
· [安全说明](SECURITY.md)

## 已实现与限制

- 在 `%LOCALAPPDATA%\Trucli` 初始化用户目录及可编辑配置模板。
- 校验学校无关的配置：域名、域名后缀、IPv4/IPv6 CIDR、检测提示、可选 DNS 预期。
- 从用户已有 Mihomo YAML 生成独立文件，把校园 `DIRECT` 规则放在原规则之前；
  保留其他配置语义及原规则顺序，基于原文件目录解析 provider 路径，并将输入和缓存快照复制到 Trucli 目录。
  不保留注释和排版，不覆盖原文件。
- 找到用户信任的 Mihomo 后运行 `-t` 校验；缺少引擎时明确标为仅结构校验。
- 只读采集 Windows 进程、安装记录、网卡、IPv4/IPv6 路由、DNS、网关、接口跃点、
  系统代理和运行状态证据，可导出经过白名单处理的摘要。
- 显式启动回环监听及当前用户系统代理，停止时有条件恢复原状态，并校验子进程身份。

不支持 TUN、自动登录 aTrust、验证身份认证、资源连通性探测、订阅界面或后台监护。
只影响遵循当前用户 Windows 系统代理的应用。发现进程或生成 `DIRECT` 规则并不代表
VPN 已连接。尚未验证真实校园/aTrust 环境兼容性。发布的可执行文件未签名。

## 快速开始

通过学校授权渠道安装官方 aTrust 并在官方客户端完成认证。
从 [MetaCubeX 官方发布页](https://github.com/MetaCubeX/mihomo/releases) 自行获取并核验
Windows Mihomo 代理引擎；代理引擎源码位于 [`Meta` 分支](https://github.com/MetaCubeX/mihomo/tree/Meta)。
准备自己的有效 Mihomo 配置。Trucli 不下载或打包这两个客户端。

下载 [alpha 预发布包](https://github.com/styayur/trucli/releases/tag/v0.1.0-alpha.1)，
使用 `Get-FileHash -Algorithm SHA256` 与同次发布的 `SHA256SUMS.txt` 核对后解压：

```powershell
.\trucli.exe --version
.\trucli.exe init
notepad "$env:LOCALAPPDATA\Trucli\profiles\example.yaml"
.\trucli.exe profile list
.\trucli.exe profile validate example
.\trucli.exe status --profile example
.\trucli.exe doctor --profile example --config .\mihomo.yaml
.\trucli.exe --mihomo 'C:\Tools\mihomo.exe' config build --profile example --input .\mihomo.yaml
# 将以下占位符替换为 build 实际输出的文件路径：
.\trucli.exe --mihomo 'C:\Tools\mihomo.exe' config validate --config '<generated.yaml>'
.\trucli.exe --mihomo 'C:\Tools\mihomo.exe' start --mode system-proxy --config '<generated.yaml>'
.\trucli.exe stop
```

示例中的 `.invalid` 域名故意不能通过校验，必须用学校授权文档提供的域名替换。
CIDR 默认为空，不假设任何私网段属于校园网；填写规范网络前缀，拒绝主机位及 `/0`。
保持 `mode: system-proxy` 和 `require_campus_rules: true`。
`dns_expectations` 只用于检查，不会更改 DNS。

基础 YAML 使用 `rule` 模式（或省略 mode），设置非零 `mixed-port` 或 HTTP `port`。
启动要求 `allow-lan: false`、`bind-address: 127.0.0.1`，关闭 TUN 和内置 DNS 服务，
省略控制器、自定义 listeners、tunnels、透明代理端口、authentication、全局
`interface-name`、NTP 及 UI 字段。这些不支持的配置会被拒绝，不会被静默改写。
可选 SOCKS 端口也必须可用。校园 DNS 需要由已有授权网络环境正确提供。

如果绝对 PATH 目录中已有 `mihomo.exe`，可省略 `--mihomo`；也可显式指定可信 `.exe`。
文件型 provider 更新后需重新生成；HTTP provider 刷新只写 Trucli 缓存。
外部引擎校验可能访问配置中的 provider URL 并写入引擎数据目录；`status` 与 `doctor`
不会运行引擎。为保护密钥，外部校验输出不会公开，失败时应在本地直接运行引擎排查。
任何命令均支持 `--help`。`--home <目录>` 可隔离状态。编辑生成文件后需重新生成，
因为启动会核对记录的 SHA-256。退出码 0 表示成功（包括明确标注的仅结构校验），
1 表示操作或校验错误，2 表示命令语法错误。停止时发现后续代理修改会保留新设置、
停止自己启动的子进程，并返回 1。

## 路由、检测与安全

校园域名和 CIDR 的 `DIRECT` 规则在原 catch-all 规则之前，其余流量继续使用原代理组
及规则。局域网访问遵循显式配置，不自动添加网段。Trucli 不创建 VPN 网关、路由、
DNS 服务器或资源授权；仅在已有 OS VPN 路由及授权有效时才可能正确访问校园资源。

进程和接口检测使用可配置的子串提示。校园路由证据要求匹配到处于 Up 状态的提示接口，
其上存在覆盖指定校园 CIDR 的非默认路由。最多只能报告“可能建立连接”，不会声称
已经认证。仅域名配置无法提供 CIDR 路由证据。路由观察上限为 4096 条。
Fake-IP 重叠检查包含显式 IPv4/IPv6 池和启用 Fake-IP 时的默认 IPv4 池；重叠会阻止
生成。`doctor --profile example --config <文件>` 可只读检查。

诊断不修改网络，不上传数据。导出文件只包含计数、标志和跃点，不含任意配置值、
个人路径、资源域名、地址、凭据或订阅令牌。分享前仍应自行检查：

```powershell
.\trucli.exe doctor --profile example --export .\trucli-report.json
```

启动会保存 WinINet LAN 代理快照，以挂起状态创建子进程并记录 PID、创建时间及执行
路径，然后恢复运行；确认监听端口属于该进程后才设置代理。停止时仅在当前代理仍与
Trucli 应用的状态完全一致或已经恢复时处理恢复，并在同一原生进程句柄上校验身份后
终止自己启动的进程。其他应用或用户的后续修改优先。不会修改 WinHTTP、路由、DNS、
防火墙或 aTrust 状态；没有 TUN 回退。

本地运行快照可能包含私有 PAC URL，生成 YAML 可能保留原代理密钥。
目录继承 Windows 账户目录权限，请保护它们，不要上传。相同用户恶意篡改以及第三方
并发代理写入不在安全边界内，跨应用的读取、比较和写入不能原子化。
中断启动或引擎意外退出后，在相同 `--home` 下运行 `stop`；恢复失败会保留状态便于重试。
没有自动崩溃监护，也不保证完整网络回滚。在进程创建与身份记录之间发生硬崩溃可能
留下挂起进程，需本地核验，不应按进程名批量终止。损坏的状态应保留排查。

## 构建、发布与后续工作

需要 Rust stable、MSVC C++ 工具和 Windows SDK。打包另需 Python 3.11+。

```powershell
cargo fmt --all -- --check
cargo check --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo install cargo-audit --version 0.22.2 --locked
cargo audit --deny warnings
cargo build --release --locked --target x86_64-pc-windows-msvc
python scripts/package.py
```

测试涵盖配置/CIDR 错误、规则顺序和合并、Fake-IP 重叠、脱敏、所有帮助页面、错误码、
初始化及模拟代理恢复。CI 不修改真实代理或路由。本地只读烟雾测试检查原生诊断和代理读取；
真实代理修改及校园会话仍待手动验证。[CI 实际结果](https://github.com/styayur/trucli/actions/workflows/ci.yml)
可在 Actions 查看。

标签工作流重新检查、构建并以预发布形式发布 `trucli-windows-x86_64.zip` 和
`SHA256SUMS.txt`。压缩包包含未签名程序、简明用法、许可证、依赖清单和依赖许可文本；
对应标签源码可从 GitHub 下载。不包含 aTrust 或 Mihomo 二进制。
后续根据授权场景实测证据完善兼容说明、DNS 诊断和崩溃恢复。
TUN 需要独立的特权生命周期设计与测试，目前没有兼容性承诺。

## 许可与来源

Copyright (C) 2026 styayur。Trucli 使用 **AGPL-3.0-only**，不提供担保。
参见 [LICENSE](LICENSE)、[第三方声明](THIRD_PARTY_NOTICES.md) 和
[完整依赖清单](DEPENDENCIES.json)。依据
[应用许可政策](https://github.com/styayur/styayur/blob/main/LICENSE_POLICY.md)
作出本项目选择，不影响其他仓库。第三方依赖保持原许可；独立安装的 Mihomo 与专有
aTrust 不受 Trucli 许可证重新授权。LabVPN 仅用于架构参考，没有复制代码、素材或引入 fdu-connect。
