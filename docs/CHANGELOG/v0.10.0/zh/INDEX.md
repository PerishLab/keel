# Keel v0.10.0

Keel 是库，不拥有任何配置面。`Config`、`load`、`NAME`、`Listen`、`Cache`、
`Hold`、`Identity` 以及适配器的 store 片段全部移除。调用方构造每一个运行时
值并交给 Keel：`Estate` 交给 `bind`，host、port、prefix 交给 `listen`，
store 由调用方自己打开。Keel 不读任何文件，也不读任何环境变量。

demo binary 与仓库根的 `keel.toml` 随之移除。Keel 不再发布任何 binary。
那些 binary 承载的场景现在是 Rust 测试，覆盖 router、权限矩阵、gate 的门
与一个真实 relay，因此同一片表面由断言持有，而不是由一个没人部署的进程。

Keel 不再依赖 Plumb。Keel 需要的词汇由 Keel 自己拥有。

schema 是行。`@unit`、`@bond`、`@field`、`@scope`、`@value` 是承载每一代
manifest 的引擎单元；unit 行记录自己属于哪一代，其余经包含关系挂在它下面。
写入者只有 bootstrap、adoption 与 evolution，普通 put 路径像拒绝 `@pulse`
一样拒绝它们。bind 汇集活跃代并与存下的 digest 比对——那是对内容的校验，
不是对格式的校验。

plan 不再必须来自编译期类型。`Graph::add` 接受运行时构造的 `Spec`，
`Graph::read` 从 Keel 写下的 manifest 水化回来，于是 schema 可以作为数据
进入引擎。

estate format 从九推进到十一。
