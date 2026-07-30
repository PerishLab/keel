# 迁移到 Keel v0.10.2

对调用方没有任何要求。公开表面未变,estate format 仍是十一,没有库需要重建。

如果你以 release profile 构建 Keel——容器镜像、打包的二进制,任何超出 `cargo build`
的场合——就升级。在 v0.10.0 与 v0.10.1 上那次构建会直接失败,报
`no method named mirrors found for struct Manifest`。debug 构建(测试与本地运行所用)
从不受影响,所以这个失败最早出现在打包的时候。
