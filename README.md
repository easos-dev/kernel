# EasOS Kernel

EasOS 是一个极简的动态可插拔微服务工作区。工作区的每个一级目录都是插件，`kernel/` 是使用 Rust 实现的主插件，负责插件发现、安装、卸载、启动和停止。

```text
easos/
├── kernel/                 # 主插件、独立 Git 仓库
└── <plugin-id>/            # 其他插件、可使用任意语言
```

插件统一遵循以下目录契约：

```text
<plugin-id>/
├── manifest/               # Kernel 主清单与扩展清单
├── source/                 # 开发源码，生产环境可省略
├── bin/                    # Linux 可执行产物
├── config/                 # 插件配置
└── docs/                   # 插件文档，生产环境可省略
```

当前 V1 只实现最小生命周期闭环。Socket Pair、插件间调用协议和多语言 SDK 属于下一阶段。

## 文档

- [整体架构](docs/EasOS-platform-architecture-v1.md)
- [数据结构](docs/data-model.md)
- [开发、容器运行与 CLI](docs/development.md)
