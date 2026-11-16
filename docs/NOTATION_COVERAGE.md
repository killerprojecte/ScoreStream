# 乐谱覆盖清单：先有边界，再扩展能力

目标是把逐个源文件补洞改为**版本化标准清单、逐域建模、分层验收**。
初始交付提供可运行的覆盖基础设施和已存在能力的证据账本。本地集成检查点进一步合并
已完成的连音线作者数据和四种有限奏法实现，不把清单本身当成新增能力。
也没有宣称 MusicXML 或 SMuFL 能代表世界上的全部乐理与记谱传统。

## 五条独立能力轴

`docs/notation/coverage.json` 每个域必须分别记录：

1. `model`：作者数据能否精确表达身份、锚点、作用域、缺省与显式值。
2. `import`：MusicXML 的哪些上下文、属性、值能进入该模型。
3. `export`：能否精确写出并重新导入；何时必须拒绝而不是取整或删除。
4. `render`：实际预览是否正确绘制，或明确 withheld；导入成功不是绘制证据。
5. `playback`：哪些显式解释策略产生已验证的声音计划；字形存在不是演奏语义。

状态仅允许 `partial`（有明确范围与正向测试）、`blocked`（有拒绝测试）、
`unassessed`（未在此账本验收）和 `not_applicable`（须说明原因）。
故意没有把整个标签或字形标为 `supported` 的入口。`partial` 也只指该行文字界定的范围，
不是所关联 schema 声明的全部值、属性、上下文或组合均被支持。

集成账本现有 23 个域：14 个有限能力域和 9 个待验收域，引用 89 个可运行的精确测试。
模型/导入/导出各 14 条 `partial`；绘制 2 条 `partial`、3 条 `blocked`、18 条未验收；
演奏 10 条 `partial`、2 条 `blocked`、11 条未验收。这些是**账本行数，不是标准支持率**。

已录入音高/节奏/和弦、延音线、连音线、变音符、符杠、反复与房子、延长记号、倚音、
速度、力度、移调与单乐器元数据。例如连音线作者字段可以往返，但该 profile 的预览和演奏
仍明确拒绝；变音符可以导入 7 种，已录入的预览证据只有 natural/sharp/flat。
延长记号和倚音的 `partial` 演奏只覆盖要求明确策略的入口，不扩大普通播放权限。

待验收域包括装饰音、乐器技术、歌词、和声/数字低音、打击乐/指法谱、方向跨度、
谱表状态词汇、页面系统排版和自定义/SMuFL 记号。`unassessed` 不表示程序接受，也不表示
程序一定不支持；生产转换器的严格诊断仍是每个实际输入的最终依据。

## 本地奏法集成的明确边界

合并来源保留在 `coverage.json` 的 `integrated_checkpoints`：清单 `d6041ed7`（含连音线
`53dfbce`）与奏法 `5555cfdd`。它们在共享 MusicXML/SFS 入口并存，而非选择一方覆盖另一方。
新增联合测试验证同一音符上的连音线和奏法可往返，连音线的预览和演奏仍明确拒绝。

四种有限奏法为 staccato、staccatissimo、tenuto、accent；只接收有节拍的有音高事件，
以及缺省/above/below placement。导入、导出、模型、真实 Verovio SVG 和演奏计划分别有
精确测试。形状、坐标、字体替换、倚音/休止符锚点与全部扩展属性未因此获得支持。
详见 [`ARTICULATION_IMPORT.md`](ARTICULATION_IMPORT.md)。

`articulations.choice_inventory` 覆盖固定 schema 的全部 17 个直接 choice，并记录名称、
精确声明 ID、schema 类型、scope class、导入状态、不完整原因和证据。以下 13 项在默认 `LegacySimple` 导入/运行时路径仍未完成；其作者数据现已在后述显式 `AuthoredV1` 路径另行建模：

- strong-accent：保留方向需要独立作者字段；默认路径没有方向字形或声音验收。
- detached-legato、spiccato：不可当作现有短音/长音组合或乐器技巧的别名。
- scoop、plop、doit、falloff：音高进入/离开记号，不推测起点、终点或弯音演奏。
- breath-mark、caesura：形状与暂停时间/范围是不同层，后者仍未实现。
- stress、unstress、soft-accent：独立重音类别，不等同于已有 accent 或力度数值。
- other-articulation：被动文字/字形名的保存不是自定义字形渲染或声音实现。

这 13 项的 `blocked` **仅指有精确拒绝测试的导入边界**；不把拒绝当作完成实现，也不把
原生 SFS 已有的某些声音策略误记为 MusicXML 导入或绘制支持。scope class 是工作分类，
不是 MusicXML 新类型或推导支持率的依据。其余域的未知状态和清单缺口继续显式保留。
独立原件验收仍未建立：固定版本的 [W3C 社区测试集](https://github.com/w3c-cg/musicxmlTestSuite/tree/0618bdee1bfcb965ffdd9f4e52271c909d744fd1)
32c 的非映射 identification/miscellaneous 元数据是较早边界，其合法的重复 articulations
容器也不在本 profile 内。此集成未下载、运行或宣称接受该原件；没有删改原件来获得绿色结果。
默认路径继续拒绝容器身份、空/重复组与堆叠；下述独立作者 profile 精确保留这些字段，不通过展平容器获得运行时绿灯。
离线 gate 会拒绝 choice 缺项、重项、失效引用、类型变化、缺范围/原因和证据不匹配。

## 显式作者奏法家族 v1

新增 `authored-articulations` 账本行，与上面的默认四符号 profile 分开记账。
`MusicXmlOptions.articulations = AuthoredV1` 才启用它；不是桌面或 CLI 的默认导入模式。
17 个 choice 的作者模型、导入、导出均为有边界的 `partial`。绘制为 `partial`：
仅已验证的 10 个普通符号，其中 3 个要求显式 placement；整套作者能力的演奏仍为 `blocked`。
拒绝测试不是绘制或演奏完成率，默认路径的 4/13 状态不因另一 profile 而改写。

版本化模型保留事件锚点、notations wrapper 的 id/print-object、articulations 组的 id、
空组/附加空 wrapper、重复符号及 wrapper/组/符号顺序。17 种封闭分支有独立 placement、
方向/呼吸形状/中断形状/线条字段和精确排版属性；缺省与显式值分开。other-articulation
仅保存被动原文和可选字形名，不存外来 XML、不猜声音、不宣称字形存在。
SFS 用 `scorestream.articulation@1` 的显式标量记录往返；不会改写为旧 articulation 音效。

这是**有边界的表示、交换与普通符号绘制检查点**。仍只接收有节拍的有音高宿主；
本 `AuthoredV1` 单独使用时拒绝同一事件上的 tied/slur/fermata 混合 wrapper。
不同事件上的这些家族可以并存；新的显式混合 profile 见下节。
休止符/无音高/倚音锚点、editorial footnote/level、完整 Unicode ID/NMTOKEN 等仍不在范围内。
数值沿用输入 64 字符、XML 输出有限小数 32 位的精确边界，不能表示时拒绝而非取整。
跨事件关联列表在 XML 随既有声部/音符序列显式报告规范化；事件内部作者顺序不变。

真实导入→SFS→Workspace 的合成全 17-choice JSON 因包含尚未支持的可见对象，仍作为
完整预览拒绝验收，不能用已验证的 10 个符号替代或删除其余对象。完整作者能力的播放
仍在声音计划前拒绝；旧四符号的真实 Verovio/播放策略回归不变。
32c 原件仍没有新增接受证据，其独立 metadata 限制未解除；合成 wrapper 探针不是原件。
完整契约与测试入口见 [`ARTICULATION_IMPORT.md`](ARTICULATION_IMPORT.md)。

## 显式同音符混合记谱交换

独立 `AuthoredMixedV1` / `scorestream.notations@1` 在有节拍、有音高且带作者奏法组的
宿主上，用封闭类型的顺序引用保留 tied/slur/fermata 与 articulation 组的 wrapper 边界和
排列；不是把声音 tie 和图形曲线混为一谈。原有家族的字段、锚点和拒绝边界继续有效。
新账本 `mixed-authored-notations` 的模型/导入/导出为 `partial`，**整个混合预览和演奏
都为 `blocked`**，与 `AuthoredV1` 的 10 个普通符号绘制证据分开记账。未宣称外部完整
原谱已经可接受。精确范围、15 个交换回归和独立原生验证见
[`MIXED_NOTATION_INTERCHANGE.md`](MIXED_NOTATION_INTERCHANGE.md)。

## 固定参考边界与可重复计数

来源固定在 MusicXML **4.0** 的 `musicxml.xsd` 和 SMuFL **1.4** 的
`glyphnames.json`、`ranges.json`、`classes.json`。完整版本、不可变 Git commit、下载 URL、
SHA-256、版权与排除范围见 [`sources.json`](notation/sources.json)。不从 `latest` 生成账本。

MusicXML 主 schema 包含：

- 480 个元素声明位置，442 个不同名称。
- 352 个属性声明/引用位置，620 个枚举值出现位置。
- 145 个具名简单类型、224 个具名复杂类型、27 个 group、45 个 attributeGroup。

SMuFL 包含 2,932 个规范字形名/码点、131 个 range、85 个 class。class 可以重叠，
也并非每个字形都有 class；range/class 成员关系不证明字体包含该字形，更不证明渲染支持。

紧凑的 5,041 条参考记录拆成小于 70 KB 的确定性 JSON 分片，索引是
[`catalog.json`](notation/catalog.json)。这不是 5,041 种音乐符号：包含结构容器、元数据、
排版、类型定义、枚举与字形。账本只对明确审查过的声明做 `structure`、`metadata`、
`layout`、`musical_data` 分类；其余保持 `unclassified`，不靠名字猜测。

MusicXML ID 保留 schema 的具名类型/group、sequence/choice 的位置及重复声明的位置。
例如 note 的两处 tie/cue 和 note-notation/barline 的两处 fermata 不会合并。
属性 `ref` 保留原值，**不会把未解析的继承属性自动算成支持**。
紧凑清单不是有效 XSD 验证器：完整 compositor、类型引用、attributeGroup、union、facet、
pattern、default/fixed、use 和 min/maxOccurs 树可以从同一哈希固定源文件生成到额外输出，
而不是塞进日常账本。后续要判定有效属性/值空间时必须解析这些引用和约束及规范正文。

排除边界：此版本没有盘点 `opus.xsd`、`container.xsd`、`sounds.xsd`、外部 `xml.xsd`/
`xlink.xsd` 或只写在规范正文的约束。MXL 容器能力不能由主 schema 的数量推断。
SMuFL 清单没有囊括字体私有/可选字形、stylistic alternates、ligatures、sets、anchors、
字宽/BBox 或字体本身。MusicXML `other-notation/@smufl` 也不形成 XML 名称与字形一一对应。
这些均是后续清单工作，不从统计中悄悄消失。

## 离线运行

Python 3.10+ 标准库即可做清单检查，不下载、不启动界面、不触发 CI：

```sh
python3 scripts/notation_coverage.py check
python3 -m unittest discover -s scripts/tests -p 'test_notation_coverage.py' -v
python3 scripts/notation_coverage.py report > /tmp/notation-report.json
python3 scripts/notation_coverage.py report --gaps > /tmp/notation-gaps.json
```

`check` 校验分片与组合清单哈希、唯一 ID、引用、五层状态、范围/限制、生产代码路径和
精确测试入口。缺失层、裸标签引用、失效测试名、把拒绝测试冒充正向支持等都会失败。
它只检查证据声明，输出明确说明 **Behavior tests NOT executed**。
该离线 gate 和自己的回归测试已加入 `scripts/check.sh`，未新增或修改 CI workflow。

`report` 输出逐层已验收/未验收的行数及没有直接域链接的记录。`--gaps` 列出全部这类 ID，
便于后续认领。没有链接不等于运行时不支持；有链接也不把完整声明提升为支持。
即使某个 feature 引用一个 SMuFL range，也不会给它的所有 glyph 自动记分。
报告不计算误导性的单一覆盖百分比。

真正重跑证据（需要现有 Rust 工具链、锁定依赖，以及 frontend 的 `npm ci` 环境）：

```sh
python3 scripts/notation_coverage.py test-evidence --feature articulations
python3 scripts/notation_coverage.py test-evidence --feature authored-articulations
python3 scripts/notation_coverage.py test-evidence --feature authored-slurs
python3 scripts/notation_coverage.py test-evidence --feature printed-accidentals
# 不带 --feature：重跑当前账本全部精确测试。
python3 scripts/notation_coverage.py test-evidence
```

Rust 使用 integration target 和 `--exact`；Vitest 使用文件、suite 和完整测试名；参数化用例同时绑定源码中的字面量模板和展开后的精确名称。
零测试、被忽略的测试或不匹配的测试不会当成成功；完全未验收的域不允许用零测试过关。
该入口输出执行记录，但不把结果写成永久绿色状态，也不代替完整 workspace、平台、音频或
视觉回归。人工仍须检查测试是否真的断言了声明的含义，文件中出现一个函数名不能证明语义。

## 重建与升级参考源

先按 `sources.json` 的不可变 URL 把四个文件存到本地目录，不改它们的字节：

```sh
python3 scripts/notation_coverage.py check --source-dir /path/to/upstream
python3 scripts/notation_coverage.py regenerate --source-dir /path/to/upstream
python3 scripts/notation_coverage.py regenerate --source-dir /path/to/upstream \
  --expanded-output /tmp/notation-expanded.json
```

任何上游字节不匹配即拒绝。重建要求与组合清单哈希完全相同；不会下载或偷偷更新版本。
升级标准是另一个明确审查：更新源 pin、重新生成、检查所有 ID/上下文变化、重新绑定证据、
调整 pinned-count 测试，再提交差异。旧 ID 不静默转移给同名新声明。

## 每次扩域的完成条件

1. 从相应 schema 上下文、值/属性约束及规范正文定义功能范围，覆盖特殊与否定情况。
2. 定义作者模型的身份、锚点、作用域、显式缺省/缺失值、跨声部/跨谱表与精确时间关系。
3. 增加有边界的导入与导出、模型/SFS/JSON/规范化往返测试及明确拒绝测试。
4. 独立验收真实渲染结果和实际播放计划；装饰、解释策略、乐器能力不能从字形推导。
5. 测组合而不只测孤立符号：反复后的 occurrence、倚音/延长/连线、声轨和节奏交互。
6. 将 untouched 原件验收、源片段解码、合成 fixture 分开记录。禁止删去原件未知符号来
   宣称全谱导入成功。更新覆盖行并跑离线 gate、精确证据和受影响完整回归。

这让缺口从个别曲目的临时补丁变成可认领、可审查、可测试的工作清单，但不以清单代替实现。

## 来源与许可

此处数据是对上游规范标识符、结构和元数据事实的派生清单，不含字体、图像或规范说明正文。
保留来源版权：

- Copyright © 2004–2021 the Contributors to the MusicXML Specification, published by the W3C Music Notation Community Group.
- Copyright © 2021 the Contributors to the SMuFL Specification version 1.4, published by the W3C Music Notation Community Group.

两者按 [W3C Community Final Specification Agreement](https://www.w3.org/community/about/agreements/final/)
发布；[人类可读摘要](https://www.w3.org/community/about/agreements/fsa-deed/)。不要把历史版本的
MIT 或 Bravura 字体的 OFL 误记为这两个规范版本的许可。

一手定义：[MusicXML 4.0 主 schema](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/)、
[SMuFL 1.4](https://www.w3.org/2021/03/smufl14/)、
[SMuFL 许可](https://www.w3.org/2021/03/smufl14/preamble/license.html)、
[recommended / optional glyphs](https://www.w3.org/2021/03/smufl14/about/recommended-chars-optional-glyphs.html)、
[other-notation](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/other-notation/)。
