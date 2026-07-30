# Keel v0.10.1

null 不再往 Postgres 带类型。`Val::Null` 绑的是 `None::<i64>`，驱动因此在预备语句时
把该参数声明成 `int8`，而 Postgres 连同这些类型一起缓存语句。于是**第一行留空的列
就被钉死成 `int8`**，之后任何为它提供文本的行，都是声明八字节却只给了四字节：

```
INSERT INTO "@unit_@field" (...) VALUES ($1..$14)
ERROR:  insufficient data left in message
CONTEXT:  unnamed portal parameter $6
```

现在 null 绑的值产生 OID 零：类型保持未指定，Postgres 从目标列推断，首次 prepare
读的是 schema，而不是碰巧第一个到达的那一行。

这个绑定和适配器一样老，不是 v0.10.0 的回归。v0.10.0 改变的是 manifest 写成行，
于是 `@field.serial`——多数字段为 null、scoped serial 为文本——把下毒的次序直接摆进了
启动路径。任何带 `serial` 字段的模型在 Postgres 上根本无法 bootstrap。

影响面比 bootstrap 宽。一个普通模型，可选文本字段在第一行缺席、第二行出现，
在普通写路径上以同样方式失败。两个测试各钉一半：`scoped` 用带 scoped serial 的模型
bootstrap，`spare` 在省略过该字段的行之后写入可选字段。此前 pg 套件两者都没有——
引擎带着这个缺陷发布，正是因此。

Sqlite 不受影响。它不声明参数类型，它的 null 绑定从来钉不死任何列。
