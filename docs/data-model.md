# Kernel V1 数据结构

V1 只保留三个持久化事实源和一个运行时控制协议。

## 1. 主 Manifest

位置：`easos/<plugin-id>/manifest/main.json`

```json
{
  "schema_version": 1,
  "id": "clock",
  "name": "Example Clock",
  "version": "0.1.0",
  "kind": "process",
  "runtime": {
    "entrypoint": "bin/service",
    "args": [],
    "environment": {},
    "stop_timeout_ms": 3000
  },
  "provides": ["example.clock.v1"],
  "requires": ["kernel"]
}
```

| 字段 | 约束 |
|---|---|
| `schema_version` | V1 固定为 `1` |
| `id` | 1–64 位安全标识；必须等于插件目录名 |
| `version` | SemVer |
| `kind` | `kernel` 为 `builtin`；其他插件为 `process` |
| `runtime.entrypoint` | 相对路径，必须位于 `bin/` |
| `runtime.args` | 启动参数数组 |
| `runtime.environment` | 插件进程环境变量 |
| `runtime.stop_timeout_ms` | `1..=60000` |
| `provides` | 能力声明；V1 仅展示 |
| `requires` | 插件依赖；用于启停顺序和停卸保护 |

未知字段直接拒绝。附加 Manifest 可放在同一 `manifest/` 目录，Kernel V1 只读取 `main.json`。

## 2. 插件配置

位置：`easos/<plugin-id>/config/main.json`

```json
{
  "schema_version": 1,
  "settings": {
    "timezone": "Asia/Tokyo"
  }
}
```

配置属于插件本身。CLI 修改后原子写回该文件；Kernel 启动插件时通过 `EASOS_PLUGIN_CONFIG_PATH` 传递其绝对路径，不再生成配置快照。

## 3. Kernel 生命周期状态

位置：`easos/kernel/config/state.json`

```json
{
  "schema_version": 1,
  "autostart": ["clock"]
}
```

此文件只保存自动启动插件 ID，不保存安装列表和插件业务配置：

- 安装状态来自工作区目录扫描。
- 插件设置来自各自的 `config/main.json`。
- 卸载插件时同步从 `autostart` 删除其 ID。

## 4. CLI 控制协议

传输：`/run/easos/kernel.sock`；Unix Domain Socket；一行一个 JSON；单条最大 1 MiB。

请求：

```json
{
  "protocol_version": 1,
  "command": "start",
  "id": "clock"
}
```

成功响应：

```json
{
  "protocol_version": 1,
  "ok": true,
  "data": {
    "type": "plugin",
    "value": {}
  }
}
```

失败响应：

```json
{
  "protocol_version": 1,
  "ok": false,
  "error": {
    "code": "NOT_FOUND",
    "message": "plugin not found: clock"
  }
}
```

控制协议只服务 CLI 与 Kernel 生命周期管理，不等同于下一阶段插件间业务调用协议。

## 5. 运行时环境变量

| 变量 | 内容 |
|---|---|
| `EASOS_HOME` | 整个 EasOS 工作区，例如 `/easos` |
| `EASOS_RUNTIME_HOME` | Socket、PID、日志目录，例如 `/run/easos` |
| `EASOS_PLUGIN_ID` | 当前插件 ID |
| `EASOS_PLUGIN_HOME` | 当前插件目录 |
| `EASOS_PLUGIN_CONFIG_PATH` | 当前插件 `config/main.json` 的绝对路径 |

运行态文件不写入插件目录；插件目录只保留可持久化、可检查、可发布的内容。
