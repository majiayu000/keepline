# R13：与 CI 一致的格式、Clippy 和完整测试

决定：采用固定 Rust 1.95.0 / rustfmt 1.9.0-stable / Zig 0.16.0；不修改 crate edition、不降低 lint 门槛。原 CI 用浮动 stable，不能宣称与本地工具链一致，现由 workflow 与 next/rust-toolchain.toml 固定。

## 根因与改动

- daemon 是 edition 2021，core/app 是 2024；旧格式混用排序。`cd next && cargo fmt` 由 Cargo 给各 crate 传实际 edition，未统一强制 2024。
- 原 `cargo fmt --manifest-path next/Cargo.toml` 从仓库根目录运行，本机 rustfmt 返回 Failed to find targets；同一命令在 next 运行能正确列出差异。CI 改为 job 默认 working-directory=next，避免相对路径解析差异。
- 干净 checkout 没有被忽略的 `.deps`；CI 补调用已有 bootstrap.sh 拉取固定版本、安装 Zig 0.16.0。setup-zig v2 固定到 commit，不使用 latest。Zig 版本依据[官方发布索引](https://ziglang.org/download/index.json)，action 用法见[上游](https://github.com/mlugg/setup-zig)。
- CI clippy/test 显式 --workspace，防止 default-members 只覆盖 core/daemon、漏掉 app。
- 等价修正 nested-if/let-chain、默认对象初始化、无用 unit binding、matches!、contains、enumerate、format_args 与测试模块位置；没有删验收断言。
- 删掉仅转发的 paint_pane wrapper，完整 capture 限定为 test/visual-test；保留实际绘制路径。唯一新增局部 allow 是 paint_cached 的 too_many_arguments，旁边说明保留显式输入及 GPUI Window/App 所有权，避免只为凑参数数量增加包装抽象；没有全局 allow。

## 验证

工作目录 next；ZIG 指向本机 0.16.0，LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast，与 CI 一致。bootstrap 检查固定依赖与补丁通过，workflow YAML 解析通过，git diff --check 通过。

- cargo fmt --check：通过。
- cargo clippy --workspace --all-targets -- -D warnings：通过。workspace 自身零警告；固定上游 GPUI/cocoa 仍输出弃用警告（Cargo 对依赖 cap-lints，不是本轮新增屏蔽），没有改 .deps。
- cargo test --workspace：**91 通过，0 失败**；1 个 ignored 是 integration fixture 的子进程入口，不是跳过的验收项。详见 tests.txt。

本轮未推送，未触发远程 Actions；本地通过不等于远程 CI 已通过。尚未验证干净 GitHub runner 的全量下载/构建。
