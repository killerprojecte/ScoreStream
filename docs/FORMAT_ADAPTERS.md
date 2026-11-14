# 格式转换：共享模型和第一批 MusicXML 映射

这是**严格、有限的 MusicXML 4.0 score-partwise 转换器**，不是“所有 MusicXML 已支持”。压缩 `.mxl`、score-timewise、MuseScore `.mscz` 均未实现。用户提到的 MCSZ 可能是 MSCZ，不能在没有确认格式的情况下套用 XML 解析。

## 分层与可运行命令

- `scorestream-core::score::ScoreDocument`：格式无关的作者模型，持有共享 `DocumentMetadata`、声部/声轨、逐小节谱表状态、独立音符/休止符时值和书写时值。不是原来 SFS 的字段字典，也不是只适合播放的 `ResolvedScore`。
- `scorestream-formats`：小型 `ScoreAdapter` 读写接口，以及 `MappingReport`。SFS 与 MusicXML 各自解析输入，再显式映射到公共字段。新增格式时实现另一适配模块即可，不注册“任意 markup 透传”。
- MusicXML XML 解析、导入、导出分开；使用锁定的 crates.io `quick-xml 0.38.4`，不手写 XML tokenizer。该依赖为 MIT 许可，只有已有的 memchr 依赖。
- 此切片不改变渲染器、原 SFS canonical 规则、播放策略、桌面或 React，也没有新增 MCP 转换工具或 CI。SFS 的元数据扩展来自单独的元数据改动。

```sh
# 查看解析后的公共模型，不启动界面/声音
cargo run --locked -p scorestream-cli -- inspect-format musicxml crates/scorestream-formats/tests/fixtures/two-staff.musicxml

# XML → 公共模型 → SFS。成功内容仅写 stdout；JSON 诊断写 stderr。
cargo run --locked -p scorestream-cli -- convert musicxml sfs crates/scorestream-formats/tests/fixtures/two-staff.musicxml > /tmp/two-staff.sfs

# SFS → 同一个公共模型 → 新生成的 XML
cargo run --locked -p scorestream-cli -- convert sfs musicxml /tmp/two-staff.sfs > /tmp/two-staff.musicxml

# 不支持的方向/装饰音：退出码 1，stdout 为空，stderr 指明元素路径和源位置
cargo run --locked -p scorestream-cli -- convert musicxml sfs crates/scorestream-formats/tests/fixtures/unsupported.musicxml
```

原 `validate`、`inspect`、`canonical` 命令仍处理完整的既有 SFS 范围，不受此转换子集限制。转换命令只支持参数 `sfs` 和 `musicxml`；未实现的格式名返回用法错误，而不是猜测处理。

## 当前能保留什么

| 域 | 映射与边界 |
| --- | --- |
| 声部与谱表 | part-list/score-part/part、单乐器的 instrument-name、1–64 个谱表、每部最多 256 声轨。所有声部的小节边界导出时必须对齐。 |
| 时间 | divisions 和 duration 精确转为有理数，以四分音符为单位。backup/forward 只移动当前小节游标；负回退、越界、不明确的中途状态更改均拒绝。 |
| 小节 | 保存小节标签、显式 ID、起点、长度、implicit。普通小节按活动拍号确定长度，无拍号按已解析的最大游标推导；隐含小节按实际长度。不会自动补发声休止符。 |
| 谱表状态 | 小节开头的五度圈 key、普通/加法拍号、G/F/C clef。状态跨小节继承；每个谱表单独建模。不支持 mode、非传统调号、特殊谱号、中途变化、改变谱表数量或不等长复拍。 |
| 音符 | 有拼写的音高和休止符、声轨与谱表、独立精确时值。可跨谱表和弦，但 `<chord/>` 不推进游标；每个和弦音保持自身时值，副音不可长于首音。SFS 允许更长副音时，导出明确失败，不私自换首音。 |
| 写法 | note type、dots、time-modification，以及 normal-type/normal-dot。真实 duration 与书写时值分别保留，不二次套用连音比例。 |
| 元数据 | title/work-title、work_number、movement-title、composer/lyricist/arranger、显式 type=author 的顺序 authors、rights。所有内容按纯文本转义；不会当 markup 执行。 |

仅保留语义，不承诺逐字保留 XML 排版、注释、属性排列或原 divisions 数字。导出使用所需分母的最小公倍数，限制为 1,000,000,000；不能精确表示就拒绝，不四舍五入。微分音支持有限十进制，导出最多 32 位小数。MusicXML octave 限于 0–9，超出时报告。所支持 XML ID 为 ASCII 字母/下划线开头，后续字母数字/下划线/点/横线；其他合法但尚未映射的 Unicode ID 也会明确拒绝。

MusicXML 的声轨编号是声部局部的，不等于 SFS 全局身份。MusicXML 没有独立的声轨默认谱表声明；若首音与 SFS 声轨默认谱表不同而无法保留这一作者默认值，导出也会拒绝。导入会分配确定的公共声轨 ID 并报告 `normalized`；SFS 需要的其它不合法 ID 也会报告重命名。MusicXML 未指定的事件/小节 ID 会生成不与原 ID 冲突的内部身份。MusicXML 只编码有序和弦成员而不编码和弦组 ID，所以导出会报告和弦组身份在再次导入时重新生成。单乐器 XML 的引用 ID 也会报告规范化，保留的是 instrument-name；有额外乐器引用的音符仍拒绝。MusicXML 的 part-name **不是**音色身份：模型分别保存名称和 instrument-name。SFS 不能保留独立显示名称时，转换失败；不把“钢琴独奏声部”冒充 instrument。XML 未提供乐器且名称为空时，SFS 才可以用报告中的 `unknown` 占位。

## 明确的拒绝，不静默删掉

当前所有未实现元素和属性都造成失败，包括表情/力度 direction、连线/延音/装饰 notations、歌词、和声、反复、移调、打击乐、倚音、cue、谱面布局与外部资源。SFS 侧无法进入公共转换模型的 marks/relations/state/event 属性同样失败。因此 `ScoreDocument` 目前是第一批转换域模型，**尚不是**所有音乐符号的完整作者模型。既有 SFS 播放方向类型仍在 notation 模块中，不会因为转换器未映射而被删掉或绕过。

共享元数据中的 description、header、footer、subtitle、language 在 SFS → 模型 → SFS 中保留；当前没有可证明无损的 MusicXML 导出映射，所以只在导出 XML 时明确失败。未知 creator 角色、缺少角色的 creator、多个 composer 等不能用现有单字符串字段表达，也不会合并丢失身份。XML 不允许的控制字符不会被清洗后假装成功。

每条诊断有格式、影响类别、feature、对象/元素路径和可用的来源行列。`unsupported`、`invalid`、`resource_limit` 都阻止输出；`normalized` 只报告等价规范化或明确的占位身份策略。API 返回 `Err(report)` 时没有部分文档；CLI 也不输出半成品。

## 安全与验证边界

输入/输出最多 8 MiB；XML 最多 100,000 元素、48 层。使用流式 parser 构造有界树，检查重复属性/未知节点/声明位置。只接受 UTF-8 XML 1.0、显式 MusicXML version=4.0。所有 DOCTYPE、实体声明和非 XML 声明的处理指令均拒绝，不会读取本机文件或请求网络实体。标准字符引用及预定义实体可解析，非法 XML 字符不能进入输出。

这是已列出子集的结构/语义检查，不是完整 MusicXML XSD 验证器，也不声称验证过任何第三方制谱器的全部输出。常见带 DOCTYPE、布局、自动编码说明或 directions 的软件导出会报告不支持，需要后续逐域映射。

测试包括双谱表/多声轨、不同 divisions、跨小节状态继承、休止符、精确三连音、独立和弦时值、生成 ID 冲突、metadata 转义/拒绝、SFS→模型→SFS 与 XML→模型→SFS→模型→XML、恶意 DTD/未知实体、深度/大小边界和 CLI 不输出失败半成品。执行：

```sh
cargo test --locked -p scorestream-formats
cargo test --locked -p scorestream-cli
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

后续应按域补充 ScoreDocument 和双向映射，并在每次新增符号前确定锚点、作用域和损失策略。MXL 解包、MuseScore MSCZ、渲染迁移、演奏语义接入各有独立验收，不由当前 CLI 入口推定完成。

## 一手参考

- [MusicXML 4.0 score-partwise](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/)
- [divisions](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/divisions/) 与 [duration](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/duration/)
- [chord 的独立时值和游标规则](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/chord/)
- [backup / 多声轨与谱表](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/backup/)
- [attributes 的作用时间](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/attributes/)
