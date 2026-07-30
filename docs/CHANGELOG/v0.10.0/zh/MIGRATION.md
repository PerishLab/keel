# 迁移到 v0.10.0

## 所有库必须重建

estate format 从九推进到十一。旧格式的库会以 `estate format 9 needs 11`
拒绝打开，且没有升级通路：引擎单元的表只在 bootstrap 建立，永不参与生成代
迁移，所以新的 schema 行无法嫁接到既有 estate 上。

删掉命名空间重新 bootstrap。本版本的其余内容都要先做完这一步才能采用。

## 如果你读 `keel.toml`

自己声明配置面，用自己的格式、自己的环境前缀。Keel 不发现任何文件，也不读
任何环境变量，因此 `keel::config::NAME` 已移除，文件名由你决定。

`Estate`、`Generation`、`Cleanup`、`Retain` 仍是 Keel 词汇，仍然 derive
`Deserialize`。你可以把它们放进自己的文件，再把值交给 `bind`：

```rust
let core = keel::bind(graph, store).estate(&estate).await?;
```

## 如果你用了 `keel::config::Hold`

它只是在 core 与 `core.bare()` 之间二选一。改成按你自己的条件调 `.bare()`。

## 如果你用了 `keel::config::Listen`

`listen` 以参数接收 host、port、prefix：

```rust
keel::listen(core.share(), "127.0.0.1", 3000, "").await?;
```

## 如果你用了 `adapt::db::Store` 或 `adapt::pg::Store`

这些片段承载的是 store 自己的配置。改为自己用 `Sqlite::memory`、
`Sqlite::file` 或 `Postgres::at` 打开 store，再把打开后的 store 交给 `bind`。

## 如果你依赖 Keel 顺带引入 Plumb

它不再引入。如果你使用 Plumb，请自行声明；两者的版本从此互相独立。
