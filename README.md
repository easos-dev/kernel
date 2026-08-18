# EasOS Kernel

EasOS 是一个极简的动态可插拔微服务工作区。工作区的每个一级目录都是插件，`kernel/` 是负责插件生命周期的主插件。

```text
easos/                      # 工作区，不是仓库
├── kernel/                 # 主插件，也是独立 Git 仓库
└── <plugin-id>/            # 其他插件，各自可为独立仓库
```

每个插件遵循同一目录契约：

```text
<plugin-id>/
├── manifest/
│   ├── main.json           # Kernel 必读的生命周期声明
│   └── *.json              # 可选；供其他插件/SDK读取
├── bin/                    # Linux 可执行产物
├── config/
│   └── main.json           # 插件自己的配置
├── source/                 # 可选；仅开发环境需要
└── docs/                   # 可选；插件自己的资料
```

生产插件只要求 `manifest/ + bin/ + config/`。开发插件可增加 `source/` 和 `docs/`；因此同一目录既是源码工作区，也是可直接运行的插件。

## 当前能力

- `easos-kerneld`：Rust 常驻进程。
- `easos`：通过 Unix Domain Socket 调用 Kernel 的 CLI。
- 插件发现：目录存在且结构、`manifest/main.json`、`config/main.json` 合法，即为已安装。
- 生命周期：安装、卸载、启动、停止、状态查询、自动启动。
- 配置：直接读写各插件的 `config/main.json`。
- 依赖：启动时先启动 `requires`；停卸时保护被依赖插件。

当前版本只完成生命周期闭环。插件间业务调用的 Socket Pair 与多语言 SDK 是下一阶段，不混入本版代码。

## Debian 容器运行

宿主机不运行 Kernel 或 Rust。Compose 把整个 `easos/` 工作区映射为容器内 `/easos`：

```bash
docker compose up --build -d
docker compose exec kernel easos list
```

容器启动时会把最新 Linux 二进制写入：

```text
/easos/kernel/bin/easos
/easos/kernel/bin/easos-kerneld
/usr/local/bin/easos -> /easos/kernel/bin/easos
```

生命周期示例：

```bash
docker compose exec kernel easos install /easos/kernel/source/fixtures/clock
docker compose exec kernel easos config clock set timezone '"Asia/Tokyo"'
docker compose exec kernel easos autostart clock enable
docker compose exec kernel easos start clock
docker compose exec kernel easos status clock
docker compose exec kernel easos stop clock
docker compose exec kernel easos uninstall clock
```

完整架构见 [docs/EasOS-platform-architecture-v1.md](docs/EasOS-platform-architecture-v1.md)，数据结构见 [docs/data-model.md](docs/data-model.md)。
