import { Info } from "lucide-react";
import { memo, type ReactNode } from "react";
import {
  type AttrBlockKind,
  BLOCK_FIELDS,
} from "@/features/editor/weft-schema";
import { type Lang, strings } from "@/i18n";

type Field = { name: string; required?: boolean; desc: string };
type Section = { title: string; lead: string; fields: Field[]; code?: string };
type Copy = {
  intro: string;
  topLevelTitle: string;
  topLevel: Field[];
  exampleTitle: string;
  example: string;
  sections: Section[];
  timeTitle: string;
  timeLead: string;
  timeRows: { code: string; desc: string }[];
  rulesTitle: string;
  rules: string[];
  fieldName: string;
  required: string;
  fieldDesc: string;
};

function schemaFields(
  block: AttrBlockKind,
  lang: Lang,
  extra?: Field[],
): Field[] {
  const base: Field[] = BLOCK_FIELDS[block].map((f) => ({
    name: f.name,
    required: f.required,
    desc: f.docDesc[lang],
  }));
  return extra ? [...base, ...extra] : base;
}

const EXAMPLE: Record<Lang, string> = {
  zh: `story {
  title  = "哈利·波特与魔法石"
  author = "J.K. 罗琳"
  # 省略 date_mode 时使用内置公历
  calculable_props = {
    constellation = "constellation.rn"
  }
}

entity 霍格沃茨 {}
entity 格兰芬多 {}

entity 哈利·波特 {
  base_time        = [1980, 7, 31]
  calculable_props = [constellation]
  kind             = "人物"
}

entity 罗恩·韦斯莱 {}
entity 赫敏·格兰杰 {}
entity 隐形衣 {}
entity 伏地魔 {}
entity 汤姆·里德尔 {}

event 分院帽晚宴 {
  start    = [1991, 9, 1, 19]
  end      = start + [0, 0, 0, 2]
  entities = [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰]
}

event 圣诞节礼物 {
  start    = [1991, 12, 25, 8]
  entities = [哈利·波特, 隐形衣]
}

event 夜探禁书区 {
  start    = [1991, 12, 25, 21]
  entities = [哈利·波特, 隐形衣]
}

events 第一学年 = [分院帽晚宴, 圣诞节礼物, 夜探禁书区]

link 组织归属 {
  acyclic    = true
  transitive = true

  格兰芬多 -> 霍格沃茨
  [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰] -> 格兰芬多 {
    start = 分院帽晚宴.start
  }
}

link 人物关系 {
  <- "好友" -> [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰]
}

link 身份等价 {
  equivalence = true

  伏地魔 <-> 汤姆·里德尔
}

link 剧情阻塞 {
  acyclic       = true
  chronological = true

  圣诞节礼物 -- "获得隐形衣" -> 夜探禁书区
}`,
  en: `story {
  title  = "Harry Potter and the Philosopher's Stone"
  author = "J.K. Rowling"
  # Omit date_mode to use built-in Gregorian calendar
  calculable_props = {
    constellation = "constellation.rn"
  }
}

entity hogwarts {}
entity gryffindor {}

entity harry {
  base_time        = [1980, 7, 31]
  calculable_props = [constellation]
  kind             = "character"
}

entity ron {}
entity hermione {}
entity invisibility_cloak {}
entity voldemort {}
entity tom_riddle {}

event sorting_feast {
  start    = [1991, 9, 1, 19]
  end      = start + [0, 0, 0, 2]
  entities = [harry, ron, hermione]
}

event christmas_gift {
  start    = [1991, 12, 25, 8]
  entities = [harry, invisibility_cloak]
}

event restricted_section {
  start    = [1991, 12, 25, 21]
  entities = [harry, invisibility_cloak]
}

events year_one = [sorting_feast, christmas_gift, restricted_section]

link membership {
  acyclic    = true
  transitive = true

  gryffindor -> hogwarts
  [harry, ron, hermione] -> gryffindor {
    start = sorting_feast.start
  }
}

link relationships {
  <- "friends" -> [harry, ron, hermione]
}

link identity {
  equivalence = true

  voldemort <-> tom_riddle
}

link plot_blocking {
  acyclic       = true
  chronological = true

  christmas_gift -- "gets cloak" -> restricted_section
}`,
};

const copy: Record<Lang, Copy> = {
  zh: {
    intro:
      "WEFT 由 6 个正交的顶层声明构成：story、time、entity、event、events 与 link。每张 link 块即一张拥有明确代数结构的时变关系图，图结构与边标签正交分离。",
    topLevelTitle: "顶层声明",
    topLevel: [
      {
        name: "story { ... }",
        required: true,
        desc: "故事元信息与 .rn 脚本注册表（全局唯一）",
      },
      { name: "time <名称> = <时间>", desc: "命名时间锚点" },
      {
        name: "entity <名称> { ... }",
        desc: "实体定义（人物、组织、地点、道具等）",
      },
      {
        name: "event <名称> { ... }",
        desc: "事件/剧情点定义（与 entity 共享符号命名空间）",
      },
      {
        name: "events <名称> = [...]",
        desc: "有序事件列表（叙事线编排）",
      },
      {
        name: "link <图名> { ... }",
        desc: "同质关系图（纯 entity 时变状态图或纯 event 剧情图，图名全局唯一）",
      },
    ],
    exampleTitle: "完整示例",
    example: EXAMPLE.zh,
    timeTitle: "时间表达式",
    timeLead:
      "所有时间向量都按 story.date_mode 指定的历法分量解读，可省略末尾分量；支持引用命名锚点、实体 base_time、事件 start/end 并用 + / - 做分量偏移。",
    timeRows: [
      { code: "[1991, 9, 1, 19]", desc: "绝对时间" },
      { code: "start + [0, 0, 0, 2]", desc: "当前块开始后 2 小时" },
      {
        code: "海格午夜送信.start - [0, 0, 7]",
        desc: "引用另一事件并倒推 7 天",
      },
      {
        code: "哈利·波特.base_time + [11]",
        desc: "引用实体基准时间并加 11 年",
      },
      { code: "1881", desc: "单个整数，等价于 [1881]" },
    ],
    rulesTitle: "语法与校验规则",
    rules: [
      '标识支持中文与中点（如 哈利·波特）；含空格、运算符或以数字开头时加双引号（如 "9又3/4站台"）。',
      '支持 #、// 单行注释，/* ... */ 块注释，以及 """...""" 多行字符串。',
      "entity 与 event 共享节点命名空间（不允许重名）；每张 link 图必须同质——要么只连接 entity（可带 start/end 时变区间），要么只连接 event（用于伏笔/阻塞/因果，不可写 start/end 与 equivalence），实体在剧情点出场唯一通过 event.entities 声明。",
      "每张 link 块是独立的关系图（不可重名），图级布尔属性决定原子时间切片与时序单调性校验规则；边上的双引号字符串仅为语义标签，与图结构正交。",
    ],
    required: "必填",
    fieldName: "字段",
    fieldDesc: "说明",
    sections: [
      {
        title: "story",
        lead: "故事的元信息，唯一必填的顶层块。",
        fields: schemaFields("story", "zh"),
      },
      {
        title: "entity",
        lead: "每个 entity 块声明一个实体；内置字段之外的任意字段原样透传进 extra_props。",
        fields: schemaFields("entity", "zh", [
          { name: "其他字段", desc: "不校验，原样收进 extra_props" },
        ]),
      },
      {
        title: "link",
        lead: '一张 link 块 = 一张关系图。可选块级代数属性（均默认 false）控制整张图的拓扑校验与渲染；边支持 A -> B / A -- "标签" -> B（有向）、A <-> B / A <- "标签" -> B（无向）、<-> [A, B, C] / <- "标签" -> [A, B, C]（团）与列表批量端点，尾部可附 { start, end, description }。',
        fields: schemaFields("link", "zh"),
        code: `link 人物关系 {
  莉莉·波特 <- "夫妻" -> 詹姆·波特
  <- "好友" -> [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰]
  哈利·波特 -- "怀疑" -> 斯内普 {
    start       = 分院帽晚宴.start
    end         = 冥想盆真相.start
    description = "一直以为斯内普在偷魔法石。"
  }
}`,
      },
      {
        title: "event",
        lead: "每个 event 块声明一个事件；支持通过 事件名.start / end 或块内 start 引用时间。",
        fields: schemaFields("event", "zh"),
        code: `event 分院帽晚宴 {
  start    = [1991, 9, 1, 19]
  end      = start + [0, 0, 0, 2] # 宴会两小时后结束
  entities = [哈利·波特, 罗恩·韦斯莱]
}`,
      },
      {
        title: "events",
        lead: "有序事件 id 列表，顺序即展示顺序。",
        fields: [],
        code: "events 哈利的第一学年 = [分院帽晚宴, 厄里斯魔镜]",
      },
    ],
  },
  en: {
    intro:
      "WEFT consists of 6 orthogonal top-level declarations: story, time, entity, event, events, and link. Each link block is a distinct temporal graph with its own algebraic constraints, cleanly separated from edge labels.",
    topLevelTitle: "Top-level declarations",
    topLevel: [
      {
        name: "story { ... }",
        required: true,
        desc: "Story metadata and .rn script registry (unique)",
      },
      { name: "time <name> = <expr>", desc: "Named time anchor" },
      {
        name: "entity <name> { ... }",
        desc: "Entity definition (characters, groups, places, items)",
      },
      {
        name: "event <name> { ... }",
        desc: "Event / plot point definition (shares symbol namespace with entity)",
      },
      {
        name: "events <name> = [...]",
        desc: "Ordered event list (narrative track)",
      },
      {
        name: "link <graph> { ... }",
        desc: "Homogeneous graph over entities (temporal) or events (plot DAG)",
      },
    ],
    exampleTitle: "Full example",
    example: EXAMPLE.en,
    timeTitle: "Time expressions",
    timeLead:
      "Time vectors follow the components defined by story.date_mode. Trailing components may be omitted; reference named time anchors, entity base_time, or event start/end with + and - offsets.",
    timeRows: [
      { code: "[1991, 9, 1, 19]", desc: "absolute time" },
      { code: "start + [0, 0, 0, 2]", desc: "2 hours after this block starts" },
      {
        code: "hagrid_letter.start - [0, 0, 7]",
        desc: "7 days before another event",
      },
      {
        code: "harry.base_time + [11]",
        desc: "11 years after an entity's base_time",
      },
      { code: "1881", desc: "single integer, same as [1881]" },
    ],
    rulesTitle: "Syntax & validation rules",
    rules: [
      'Identifiers support Unicode letters and middle dots (e.g. 哈利·波特); quote names containing spaces, operators, or leading digits (e.g. "9又3/4站台").',
      'Supports # and // line comments, /* ... */ block comments, and """...""" multiline strings.',
      "Entities and events share a single symbol namespace; each link graph must be homogeneous (all entities for temporal state graphs, or all events for plot foreshadowing/blocking DAGs without start/end or equivalence).",
      "Each link block is a uniquely named graph validated across atomic time slices and chronological constraints; quoted strings on arrows are purely semantic labels.",
    ],
    required: "Required",
    fieldName: "Field",
    fieldDesc: "Notes",
    sections: [
      {
        title: "story",
        lead: "Story metadata; the only required top-level block.",
        fields: schemaFields("story", "en"),
      },
      {
        title: "entity",
        lead: "Each entity block declares a single entity; extra fields pass through as extra_props.",
        fields: schemaFields("entity", "en", [
          { name: "other fields", desc: "Not validated; kept in extra_props" },
        ]),
      },
      {
        title: "link",
        lead: 'One link block = one graph. Optional boolean properties define graph-level constraints; edges support A -> B / A -- "label" -> B (directed), A <-> B / A <- "label" -> B (symmetric), <-> [A, B, C] / <- "label" -> [A, B, C] (clique), and list endpoints, with optional { start, end, description }.',
        fields: schemaFields("link", "en"),
        code: `link relationships {
  lily <- "married" -> james
  <- "friends" -> [harry, ron, hermione]
  harry -- "suspects" -> snape {
    start       = sorting_feast.start
    end         = pensieve_truth.start
    description = "Suspected Snape of trying to steal the stone."
  }
}`,
      },
      {
        title: "event",
        lead: "Each event block declares an event; reference event.start / event.end or self start.",
        fields: schemaFields("event", "en"),
        code: `event sorting_feast {
  start    = [1991, 9, 1, 19]
  end      = start + [0, 0, 0, 2] # ends two hours later
  entities = [harry, ron]
}`,
      },
      {
        title: "events",
        lead: "Ordered event ids in display order.",
        fields: [],
        code: "events year_one = [sorting_feast, mirror_of_erised]",
      },
    ],
  },
};

function SectionCard({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="card card-sm card-border min-w-0 bg-base-100">
      <div className="card-body gap-3">
        <h2 className="card-title text-sm font-semibold">{title}</h2>
        {children}
      </div>
    </section>
  );
}

function FieldTable({ fields, lang }: { fields: Field[]; lang: Lang }) {
  if (fields.length === 0) return null;
  const t = copy[lang];
  return (
    <div className="overflow-x-auto">
      <table className="table table-xs">
        <thead>
          <tr>
            <th>{t.fieldName}</th>
            <th>{t.required}</th>
            <th>{t.fieldDesc}</th>
          </tr>
        </thead>
        <tbody>
          {fields.map((field) => (
            <tr key={field.name}>
              <td className="font-mono text-xs">{field.name}</td>
              <td className="text-base-content/50">
                {field.required ? (
                  <span className="badge badge-xs badge-outline border-primary text-primary">
                    {t.required}
                  </span>
                ) : (
                  "—"
                )}
              </td>
              <td className="whitespace-normal text-xs text-base-content/70">
                {field.desc}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export const InfoView = memo(function InfoView({ lang }: { lang: Lang }) {
  const t = copy[lang];
  return (
    <div className="mx-auto w-full max-w-3xl">
      <header className="mb-6 flex flex-col gap-2 border-b border-base-300 pb-5">
        <div className="flex items-center gap-1.5 text-xs font-medium text-primary">
          <Info className="size-3.5" aria-hidden="true" />
          <span>{strings[lang].tabInfo}</span>
        </div>
        <h1 className="text-2xl font-bold tracking-tight text-base-content">
          WEFT DSL (.weft)
        </h1>
        <p className="max-w-2xl text-sm leading-6 text-base-content/70">
          {t.intro}
        </p>
      </header>

      <div className="flex flex-col gap-4">
        <SectionCard title={t.topLevelTitle}>
          <FieldTable fields={t.topLevel} lang={lang} />
        </SectionCard>

        <SectionCard title={t.exampleTitle}>
          <pre className="overflow-x-auto rounded-box border border-base-300 bg-base-200 p-3 font-mono text-xs leading-5">
            {t.example}
          </pre>
        </SectionCard>

        {t.sections.map((section) => (
          <SectionCard key={section.title} title={section.title}>
            <p className="text-xs leading-5 text-base-content/70">
              {section.lead}
            </p>
            <FieldTable fields={section.fields} lang={lang} />
            {section.code && (
              <pre className="overflow-x-auto rounded-box border border-base-300 bg-base-200 p-3 font-mono text-xs leading-5">
                {section.code}
              </pre>
            )}
          </SectionCard>
        ))}

        <SectionCard title={t.timeTitle}>
          <p className="text-xs leading-5 text-base-content/70">{t.timeLead}</p>
          <ul className="flex flex-col gap-2">
            {t.timeRows.map((row) => (
              <li
                key={row.code}
                className="flex flex-wrap items-baseline gap-2"
              >
                <code className="rounded-field border border-base-300 bg-base-200 px-1.5 py-0.5 font-mono text-xs">
                  {row.code}
                </code>
                <span className="text-xs text-base-content/70">{row.desc}</span>
              </li>
            ))}
          </ul>
        </SectionCard>

        <SectionCard title={t.rulesTitle}>
          <ul className="list-disc space-y-1 pl-5 text-xs leading-5 text-base-content/70">
            {t.rules.map((rule) => (
              <li key={rule}>{rule}</li>
            ))}
          </ul>
        </SectionCard>
      </div>
    </div>
  );
});
