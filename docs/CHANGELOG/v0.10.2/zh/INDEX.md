# Keel v0.10.2

## Keel 重新能以 release profile 编译

`Manifest::lift` 用 `debug_assert!` 断言 manifest 的往返稳定性。该宏在断言关闭时会把调用
编译掉,但**它仍然在每个 profile 下对表达式做类型检查**——而 `mirrors` 带着
`#[cfg(debug_assertions)]`。于是 release 构建里调用还在、方法没了:

```
error[E0599]: no method named `mirrors` found for struct `Manifest`
  keel/src/model/manifest/mod.rs:89
```

v0.10.0 与 v0.10.1 根本无法以 release 构建。现在去掉了这道门:宏本就移除了调用,
所以方法在 release 下不占成本,而 dead-code 分析仍视其为已使用。

## guard 现在会类型检查那个 profile

此前这里从没有编译过它。guard 跑 `cargo test` 与 clippy `--all-targets`,两者都是 debug;
release lane 发的是 crate,而 `cargo publish` 的校验也是 debug。Keel 是库,
**它自己没有任何一条 lane 会走到 release 构建**——第一个走到的是下游的 Dockerfile,
晚了两个版本。

guard 增加 `cargo check --release`。这个缺陷类是不同 `cfg` 下的类型错误,
所以 check 就够,几秒钟而不是几分钟。
