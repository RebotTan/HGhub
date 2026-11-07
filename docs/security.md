# 客户端内容与网络安全

## 本次加固的行为

- 文章正文通过固定版本 DOMPurify 清洗，并且将 `DOMPurify.sanitize` 作为进入 `dangerouslySetInnerHTML` 前的最终步骤（幂等处理代理图片，避免二次解析差异）。严格模式下只保留必要的 HTML 排版、链接和图片，SVG、MathML、脚本、事件属性、内联样式、非 HTTP(S) URL 及本机/内网主机名均不进入页面；正文容器统一拦截 `<a>` 点击并交由外部打开或复制，防止 WebView 原地导航或弹出无守卫新窗口。
- 发布版启用 CSP：不允许内联/远端/data URL 脚本，不允许 iframe、表单与页面基址覆盖。Blob 仅用于媒体和 hls.js Worker，不在 `script-src` 中放行。应用主窗口拒绝跳转到远端页面（开发模式下亦限制 `1420` 端口仅允许本机或显式配置的 `TAURI_DEV_HOST`），媒体协议仅向应用来源返回 CORS 授权。
- 图片、HLS、分片、线路探测、镜像发布页及更新元数据的请求经过统一目标检查。只允许 HTTP(S)，拒绝 URL 中的用户名/密码；每次连接前检查全部 DNS 结果，混合公私网答案整体拒绝，检查后将地址固定给直接连接使用的 HTTP 客户端。GET 最多跟随 5 次重定向，每一跳重新检查，禁止 HTTPS 降级到 HTTP。HLS 的子清单、备用音轨、密钥及低延迟分片 URI 也经媒体代理改写，不能绕过目标检查。禁用可从外部 JSON 派生新地址的可选插播/内容引导功能。
- 业务线路必须是纯 HTTPS origin，不接受路径、查询串、片段或认证信息。密码登录和注册仅允许 `https://huangguoai.com`、`https://huangguoai.ai` 两个固定站点；远端镜像列表不能扩大账号信任范围。镜像保留游客浏览。
- token 与具体站点 origin 绑定，跨站点切换清除登录状态。在途请求捕获自己的站点快照，不能把旧 token 带到新站点。包含账号信息的业务 POST 不跟随任何重定向，避免 307/308 重发密码。旧版会话没有站点归属，升级后需要重新登录。
- 网络诊断只返回代理协议、主机和端口；隐藏代理用户名、密码、路径、查询串、片段及 NO_PROXY 列表。无效代理配置隐藏整个原始值。脱敏发生在 Rust 返回数据之前，因此后续写入日志和复制剪贴板均得到脱敏结果。此前已生成的日志/已复制的诊断内容不会自动重写。

## 严格安全模式与兼容回退（命令行 / 设置开关）

默认开启**严格安全模式**。若上游正文排版、特殊内网环境或自定义 FakeIP 网段因严格规则导致功能受阻，可通过以下任一方式回退到兼容模式（关闭激进标签/内网/导航守卫，仍保留基础 DOMPurify 脚本与事件清洗及应用内部协议隔离）：

1. **命令行参数**：启动时传入 `--no-strict`（或 `--no-strict-security`、`--relaxed`），也可设置环境变量 `HGHUB_STRICT=0`；传入 `--strict` 可显式强制开启。
2. **设置页开关**：在应用「设置 → 网络 → 严格安全模式」中切换，立即生效并持久化到应用数据目录 `security.json`。

## FakeIP 与内网策略

明确放行 `198.18.0.0/15`，即 `198.18.0.0` 至 `198.19.255.255`，兼容常见代理软件的 FakeIP。此例外同样适用于 DNS 结果、重定向及 IPv4 映射 IPv6 地址。

拒绝普通私网、回环、链路本地、共享地址空间、组播和保留地址，包括 `10/8`、`172.16/12`、`192.168/16`、`127/8`、`169.254/16`、`100.64/10`、IPv6 ULA/链路本地/回环，以及可封装私网 IPv4 的 NAT64、6to4、Teredo 地址。拒绝 localhost/.localhost、.local、.internal、home.arpa 主机名。其他自定义 FakeIP 网段没有自动放行。

系统代理和 `HGHUB_PROXY` 是用户配置的可信传输层，仍可连接 `127.0.0.1:7897` 等本地代理端点；这不意味着媒体 URL 可以指向该地址。直接连接会使用检查后固定的 DNS 结果；HTTP(S) 代理可能自行解析域名，FakeIP/TUN 也会重新映射目标，应用无法独立验证代理/TUN 最终实际连接的地址。需要同时信任并配置好代理软件；本策略不替代代理侧内网访问控制。

## 本地回归验证

```sh
pnpm build
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- \
  --skip media::tests::live_play_chain --skip media::tests::playlist_rewrite_works \
  --skip media::tests::cover_decrypt_works --skip lines::tests::mirrors_live \
  --skip lines::tests::probe_live --skip net::tests::diag
```

以上 Rust 测试跳过原有的 6 项联网测试，覆盖普通内网/FakeIP 边界、IPv4 变体、IPv6、混合 DNS 答案、重定向、凭据站点绑定、旧会话和代理脱敏，不发送真实账号信息。

浏览器回归入口为 `tests/html-security.html`。启动 `pnpm dev --host 127.0.0.1` 后，在浏览器打开 `http://127.0.0.1:1420/tests/html-security.html`；结果应为 `passed: 29, failed: 0`。测试使用无害页面变量，覆盖严格及兼容模式的原 SVG 漏洞、混淆 URL、事件属性、命名空间/HTML 变异样本，以及正常排版、懒加载图片、重复清洗和安全链接。

本次验证包括 macOS 上的前端构建、Rust 编译和本地测试，以及独立 Chromium 的 15 项正文回归与 5 项生产 CSP 检查；没有启动真实 Tauri 应用连接上游，也没有完成 Windows/Android 实机播放和真实 FakeIP/TUN 联调。

2026-11-07 本地复测：Linux 上前端生产构建、回归测试 TypeScript 检查及 `cargo check --locked` 通过；Rust 本地测试 16 项通过、0 失败（跳过上述 6 项联网测试）。独立 Chromium 分别模拟 Windows、macOS 用户代理，验证 `http://hg.localhost` 与 `hg://localhost` 两套图片协议分支，每套 29 项全部通过；这不等同于原生 Windows/macOS WebView 实测。复测修复了最终 URL 规则误删链接 `target`/`rel` 和 macOS 代理图片重复清洗丢失的问题。Linux 编译库仅下载、解包到 `/tmp` 使用，未安装到系统。

本次仅处理审查编号 1、3、5、8；Windows `cmd /C start` 外链打开方式、媒体/日志容量上限和 token 明文存储仍需另行处理。网络错误文本的 UTF-8 截断在改动网络入口时一并改为安全截断。

参考：[DOMPurify](https://github.com/cure53/DOMPurify)、[Tauri CSP](https://v2.tauri.app/security/csp/)、[reqwest DNS 地址固定接口](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html#method.resolve_to_addrs)。
