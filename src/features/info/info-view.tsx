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

entity 霍格沃茨魔法学校 {
  description = "英国的魔法学校，下设四大学院。"
}

entity 格兰芬多 {
  belong_to = [霍格沃茨魔法学校]
}

entity 哈利·波特 {
  base_time        = [1980, 7, 31]
  belong_to        = [格兰芬多]
  calculable_props = [constellation]
  gender           = "男"
}

link 家人 {
  哈利·波特 -- "父子" -- 詹姆·波特
}

event 分院帽晚宴 {
  start    = [1991, 9, 1, 19]
  end      = start + [0, 0, 0, 2]
  entities = [哈利·波特]
}

events 第一学年 = [分院帽晚宴]`,
  en: `story {
  title  = "Harry Potter and the Philosopher's Stone"
  author = "J.K. Rowling"
  # Omit date_mode to use built-in Gregorian calendar
  calculable_props = {
    constellation = "constellation.rn"
  }
}

entity hogwarts {
  description = "British school of magic with four houses."
}

entity gryffindor {
  belong_to = [hogwarts]
}

entity harry {
  base_time        = [1980, 7, 31]
  belong_to        = [gryffindor]
  calculable_props = [constellation]
  gender           = "male"
}

link family {
  harry -- "father & son" -- james
}

event sorting_feast {
  start    = [1991, 9, 1, 19]
  end      = start + [0, 0, 0, 2]
  entities = [harry]
}

events year_one = [sorting_feast]`,
};

const copy: Record<Lang, Copy> = {
  zh: {
    intro:
      "WEFT 使用类 HCL 的 .weft 声明式语言描述故事：元信息、时间锚点、实体、实体关系、事件与事件列表。除 story 外其余声明均可省略；保存后视图自动更新。",
    topLevelTitle: "顶层声明",
    topLevel: [
      {
        name: "story { ... }",
        required: true,
        desc: "故事元信息，唯一必填的顶层块",
      },
      { name: "time <名称> = <时间>", desc: "可选的独立命名时间锚点" },
      {
        name: "entity <名称> { ... }",
        desc: "实体定义；人物、团体、地点、道具都算实体",
      },
      { name: "link <组名> { ... }", desc: "实体关系组，按图边简写组织" },
      { name: "event <名称> { ... }", desc: "事件定义，支持跨事件时间偏移" },
      {
        name: "events <名称> = [...]",
        desc: "事件列表：按展示顺序排列的事件 ID 列表",
      },
    ],
    exampleTitle: "完整示例",
    example: EXAMPLE.zh,
    timeTitle: "时间表达式",
    timeLead:
      "所有时间向量都按 story.date_mode 指定的历法分量解读，可省略末尾分量；支持引用命名锚点、实体 base_time、事件 start/end 并用 + / - 做分量偏移。",
    timeRows: [
      { code: "[1991, 9, 1, 19]", desc: "绝对时间" },
      { code: "start + [0, 0, 0, 2]", desc: "当前事件开始后 2 小时" },
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
      "实体上不属于内置键的字段不校验，原样收进 extra_props 透传给前端。",
      "所有实体/事件/时间引用均会静态校验并定位到行列；belong_to 与时间引用均不许成环。",
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
        lead: "每个 entity 块声明一个实体。",
        fields: schemaFields("entity", "zh", [
          { name: "其他字段", desc: "不校验，原样收进 extra_props" },
        ]),
      },
      {
        title: "link",
        lead: '按组声明实体两两关系，使用 A -- "关系" -- B 边语法。',
        fields: [],
        code: `link 波特一家 {
  哈利·波特 -- "母子" -- 莉莉·波特
  哈利·波特 -- "父子" -- 詹姆·波特
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
      "WEFT describes a story in a single HCL-style .weft file: story metadata, time anchors, entities, links, events and event lists. Every declaration except story is optional; views refresh when the file is saved.",
    topLevelTitle: "Top-level declarations",
    topLevel: [
      {
        name: "story { ... }",
        required: true,
        desc: "Story metadata; the only required block",
      },
      { name: "time <name> = <expr>", desc: "Optional named time anchor" },
      {
        name: "entity <name> { ... }",
        desc: "Entity definition; people, groups, places and items",
      },
      { name: "link <group> { ... }", desc: "Grouped entity relation edges" },
      {
        name: "event <name> { ... }",
        desc: "Event definition with relative time expressions",
      },
      {
        name: "events <name> = [...]",
        desc: "Events: ordered list of event ids",
      },
    ],
    exampleTitle: "Full example",
    example: EXAMPLE.en,
    timeTitle: "Time expressions",
    timeLead:
      "Time vectors follow the components defined by story.date_mode. Trailing components may be omitted; reference named time anchors, entity base_time, or event start/end with + and - offsets.",
    timeRows: [
      { code: "[1991, 9, 1, 19]", desc: "absolute time" },
      { code: "start + [0, 0, 0, 2]", desc: "2 hours after this event starts" },
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
      "Entity fields outside built-in keys are passed through as extra_props.",
      "All entity, event, and time references are statically checked with line/column diagnostics; belong_to and time references must stay acyclic.",
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
        lead: "Each entity block declares a single entity.",
        fields: schemaFields("entity", "en", [
          { name: "other fields", desc: "Not validated; kept in extra_props" },
        ]),
      },
      {
        title: "link",
        lead: 'Grouped pairwise entity relations using A -- "relation" -- B edges.',
        fields: [],
        code: `link potters {
  harry -- "mother & son" -- lily
  harry -- "father & son" -- james
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
