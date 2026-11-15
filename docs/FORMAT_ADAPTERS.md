# 格式转换：共享模型和第一批 MusicXML 映射

这是**严格、有限的 MusicXML 4.0 score-partwise 转换器**，不是“所有 MusicXML 已支持”。压缩 `.mxl` 已有独立的[严格容器读取入口](MXL_CONTAINERS.md)，复用同一个语义转换器；这不代表压缩包内的所有乐谱符号都能转换。score-timewise、MuseScore `.mscz` 仍未实现。用户提到的 MCSZ 可能是 MSCZ，不能在没有确认格式的情况下套用 XML 解析。

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
| 移调 | 小节开头的 transpose：chromatic 精确有理数、可选 diatonic、octave-change。无 number 时展开到全部谱表，有 number 时只影响指定谱表；继承到后续小节，显式零值可以重置。保留书写音高和书写调号，播放阶段只应用一次。见 [移调边界与验证](TRANSPOSITION_IMPORT.md)。 |
| 音符 | 有拼写的音高和休止符、声轨与谱表、独立精确时值。可跨谱表和弦，但 `<chord/>` 不推进游标；每个和弦音保持自身时值，副音不可长于首音。SFS 允许更长副音时，导出明确失败，不私自换首音。 |
| 延音线 | 普通成对 `<tie>` 声音与 `<tied>` 记谱端点进入有身份的 `ScoreTie`，保留独立音符和跨小节连续性。支持精确编号配对、连续 stop/start 和可判定的跨谱表连接；局部编号规范化为关系身份。详见 [边界与播放验证](TIE_IMPORT.md)。 |
| 写法 | note type、dots、time-modification，以及 normal-type/normal-dot。真实 duration 与书写时值分别保留，不二次套用连音比例。 |
| 元数据 | title/work-title、work_number、movement-title、composer/lyricist/arranger、显式 type=author 的顺序 authors、rights。所有内容按纯文本转义；不会当 markup 执行。 |

仅保留语义，不承诺逐字保留 XML 排版、注释、属性排列或原 divisions 数字。导出使用所需分母的最小公倍数，限制为 1,000,000,000；不能精确表示就拒绝，不四舍五入。微分音支持有限十进制，导出最多 32 位小数。MusicXML octave 限于 0–9，超出时报告。所支持 XML ID 为 ASCII 字母/下划线开头，后续字母数字/下划线/点/横线；其他合法但尚未映射的 Unicode ID 也会明确拒绝。

MusicXML 的声轨编号是声部局部的，不等于 SFS 全局身份。MusicXML 没有独立的声轨默认谱表声明；若首音与 SFS 声轨默认谱表不同而无法保留这一作者默认值，导出也会拒绝。导入会分配确定的公共声轨 ID 并报告 `normalized`；SFS 需要的其它不合法 ID 也会报告重命名。MusicXML 未指定的事件/小节 ID 会生成不与原 ID 冲突的内部身份。MusicXML 只编码有序和弦成员而不编码和弦组 ID，所以导出会报告和弦组身份在再次导入时重新生成。单乐器 XML 的引用 ID 也会报告规范化，保留的是 instrument-name；有额外乐器引用的音符仍拒绝。MusicXML 的 part-name **不是**音色身份：模型分别保存名称和 instrument-name。SFS 的可选 `!part display_name="..."` 保存独立纯文本显示名称，不把“钢琴独奏声部”冒充 instrument。缺少名称与显式空名称分别保留；只有显示层在缺少时回退到 instrument。XML 未提供乐器时，SFS 使用报告中的 `unknown` 占位，同时保留名称。富文本 part-name-display、缩写、MIDI 设备/音色元数据仍未映射并明确拒绝。已知位置的 midi-device/midi-instrument/@id 和 note/instrument/@id 是 IDREF，不误报为重复定义；这不表示其内容已被接受。

## 明确的拒绝，不静默删掉

当前所有未实现元素和属性都造成失败，包括表情/力度 direction、普通延音线以外的连线/装饰 notations、歌词、和声、反复、打击乐、倚音、cue、谱面布局与外部资源。SFS 侧无法进入公共转换模型的 marks/relations/state/event 属性同样失败。因此 `ScoreDocument` 目前是第一批转换域模型，**尚不是**所有音乐符号的完整作者模型。既有 SFS 播放方向类型仍在 notation 模块中，不会因为转换器未映射而被删掉或绕过。

共享元数据中的 description、header、footer、subtitle、language 在 SFS → 模型 → SFS 中保留；当前没有可证明无损的 MusicXML 导出映射，所以只在导出 XML 时明确失败。未知 creator 角色、缺少角色的 creator、多个 composer 等不能用现有单字符串字段表达，也不会合并丢失身份。XML 不允许的控制字符不会被清洗后假装成功。

每条诊断有格式、影响类别、feature、对象/元素路径和可用的来源行列。`unsupported`、`invalid`、`resource_limit` 都阻止输出；`normalized` 只报告等价规范化或明确的占位身份策略。API 返回 `Err(report)` 时没有部分文档；CLI 也不输出半成品。

## 安全与验证边界

转换不再按整个文件的 8 MiB 或总计 100,000 元素拒绝输入，也不限制生成 XML 的总字节数。文件入口 `musicxml::read_reader(BufRead + Seek)` 做两次增量读取：第一次逐小节检查结构并保留源 ID 索引，第二次逐小节映射并释放 XML 节点。两次输入的 SHA-256 必须一致，拒绝读取期间被改写的文件。共同模型和 ID 索引随真实音乐内容增长，但不会再保留全谱 XML 树或整份 XML 字符串。字符串便捷 API 也复用此路径。CLI 的 MusicXML 输入使用 `BufReader<File>`；SFS 不再误用 XML 的字节上限，但仍使用自己的文本解析器。

安全预算针对异常的单个结构：48 层深度、单个 token/元素文本最多 1 MiB、当前一个头部或小节最多 100,000 个保留元素和 8 MiB 源内容及保留路径的字节预算、元素/属性名最多 256 字节、每节点最多 256 属性和不同子元素名。这些不是整首乐谱的长度上限。诊断最多 1,024 条，截断摘要保留失败状态，单条文本也有长度预算。

只接受 UTF-8 XML 1.0，转换仍要求显式 MusicXML version=4.0。唯一接受的 DOCTYPE 是固定的官方 MusicXML 4.0 partwise PUBLIC 声明，作为不执行的声明元数据处理；不会打开其中的 URL。内部 DTD 子集、实体声明、其他外部标识及处理指令仍拒绝。没有外部实体解析器，也不会读取本机文件或请求网络实体。标准字符引用及预定义实体可解析，非法 XML 字符不能进入输出。压缩 MXL 使用[独立容器验证](MXL_CONTAINERS.md)读取并校验声明的根乐谱，再进入同一个增量 XML 读取器；不会按归档路径任意解压文件。

独立的只读检查命令 `cargo run --locked -p scorestream-cli -- scan-musicxml input.musicxml` 输出字节、元素、声部、小节、音符数及最大同时保留节点数。它检查 XML 读取和结构安全，不执行符号映射；成功不能解释为全谱无损转换成功。`inspect-format` 和 `convert` 仍完整执行严格映射，不会忽略未支持的音符或符号。

这是已列出子集的结构/语义检查，不是完整 MusicXML XSD 验证器，也不声称验证过任何第三方制谱器的全部输出。常见带布局、自动编码说明或 directions 的软件导出仍会报告不支持，需要后续逐域映射。官方 DOCTYPE 可以安全读过并不代表这些音乐语义已经能转换。

测试包括双谱表/多声轨、不同 divisions、跨小节状态继承、休止符、精确三连音、独立和弦时值、生成 ID 冲突、metadata 转义/拒绝、SFS→模型→SFS 与 XML→模型→SFS→模型→XML、恶意 DTD/未知实体、深度/单结构边界、超过原先两个总量上限的 48,000 音符精确往返、短缓冲 UTF-8、诊断截断和 CLI 不输出失败半成品。执行：

```sh
cargo test --locked -p scorestream-formats
cargo test --locked -p scorestream-cli
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

后续应按域补充 ScoreDocument 和双向映射，并在每次新增符号前确定锚点、作用域和损失策略。MXL 容器读取、MuseScore MSCZ、渲染迁移和其它演奏语义各有独立验收；本次普通延音线的实际播放计划测试不代表所有符号、桌面谱面或原版总谱已经导入成功。

## 一手参考

- [MusicXML 4.0 score-partwise](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/score-partwise/)
- [divisions](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/divisions/) 与 [duration](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/duration/)
- [chord 的独立时值和游标规则](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/chord/)
- [backup / 多声轨与谱表](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/backup/)
- [transpose 的书写音高到实际音高语义](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/transpose/)
- [attributes 的作用时间](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/attributes/)
