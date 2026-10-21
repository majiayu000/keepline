# R7 Developer ID 签名与公证检查（2026-10-21）

决定：打包脚本仅增加 `--sign-identity` 与 `--output`。默认仍为 ad-hoc；指定 Developer ID 时给客户端、后台和 bundle 逐层签名，启用 hardened runtime 和安全时间戳。Rust/GPUI 没有 JIT 或第三方动态插件需求，未增加放宽 runtime 的 entitlements。产物与用户正在运行的包分开，不替换、不安装、不发布。

```sh
python3 next/package-macos.py \
  --sign-identity 'Developer ID Application: Ma JiaYu (C5UWZ934C2)' \
  --output next/target/signed-acceptance/Keepline.app
```

| 检查 | 结果 |
|---|---|
| 证书 | 本机找到 Ma JiaYu / C5UWZ934C2，使用指定证书签名；没有选另一个 identity |
| `codesign --verify --deep --strict --verbose=2` | 通过，客户端/嵌套后台及 bundle 均验证 |
| hardened runtime | CodeDirectory flags `0x10000(runtime)`，时间戳存在，TeamIdentifier C5UWZ934C2 |
| entitlements | 无例外权限；不是用 disable-library-validation / allow-jit 绕过约束 |
| `spctl -a -vv --type execute` | **不通过**，exit 3，`source=Unnotarized Developer ID`；不能称 Gatekeeper 放行 |
| 公证凭据 | 仓库没有已有 profile 配置；定向查询 notarytool service 元数据无结果，`xcrun notarytool history --keychain-profile notarytool` 返回 No Keychain password item。未发现可用 profile，停止公证 |
| 发布 | 未上传公证、未 staple、未推送、未发布；用户原 `next/target/bundle/Keepline.app` 未修改 |

[打包回执](package.txt)、[完整校验](checks.json)、[profile 查询](notary-profile-check.json)。产物位于 `next/target/signed-acceptance/Keepline.app`，二进制不进 Git。Python AST 语法检查通过；R6 的同二进制测试包以相同 identity/runtime 另签后原生窗口正常启动，未出现需要增加 runtime exceptions 的证据。

profile 查询有边界：没有遍历整个钥匙串，未读取 Claude 凭据或其他项目的秘密；没有断言机器上绝不存在任意名称的 profile。若用户已有自定义 profile，只需提供名称即可继续 `history` 核实。没有创建 profile，也不索要密码。
