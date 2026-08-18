# Kernel 开发与运行

Kernel 的 Rust 工程完整位于 `source/`：

```text
source/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── lib.rs
├── bin/
├── fixtures/
├── packaging/
└── scripts/
```

宿主机不直接构建或运行 Kernel。格式化、静态检查、测试和运行都在 Debian 容器内完成。

## 启动 Kernel

在 `kernel/` 目录执行：

```bash
docker compose up --build -d
docker compose exec kernel easos list
```

Compose 把整个 `easos/` 工作区映射到容器 `/easos`。容器启动时把 Linux 产物安装到：

```text
/easos/kernel/bin/easos
/easos/kernel/bin/easos-kerneld
/usr/local/bin/easos -> /easos/kernel/bin/easos
```

## 开发检查

```bash
docker compose --profile tools run --rm dev cargo fmt --check
docker compose --profile tools run --rm dev cargo clippy --all-targets --locked -- -D warnings
docker compose --profile tools run --rm dev cargo test --locked
```

`dev` 容器的工作目录是 `/easos/kernel/source`，构建缓存位于独立 Docker Volume，不会在宿主机生成 `target/`。

## 生命周期示例

```bash
docker compose exec kernel easos install /easos/kernel/source/fixtures/clock
docker compose exec kernel easos config clock set timezone '"Asia/Tokyo"'
docker compose exec kernel easos autostart clock enable
docker compose exec kernel easos start clock
docker compose exec kernel easos status clock
docker compose exec kernel easos stop clock
docker compose exec kernel easos uninstall clock
```

安装只复制插件目录，默认不启动。插件运行时直接读取自己的 `config/main.json`。

## 非容器 Debian 安装

发布包中的安装脚本负责：

- 建立 `kernel/manifest`、`kernel/bin`、`kernel/config`；
- 安装 `easos` 与 `easos-kerneld`；
- 建立 `/usr/local/bin/easos` 链接；
- 可选安装并启用 `easos-kernel.service`。

入口脚本与 systemd 模板分别位于：

```text
source/packaging/container-entrypoint.sh
source/packaging/install.sh
config/easos-kernel.service
```
