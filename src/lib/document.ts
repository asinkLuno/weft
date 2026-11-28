export type Phase = {
  components: number[];
  anchor: Phase | null;
};

export type LinkDomain = "entity" | "event";

export type LinkGraphSpec = {
  domain?: LinkDomain | null;
  acyclic: boolean;
  asymmetric: boolean;
  intransitive: boolean;
  functional: boolean;
  inverse_functional: boolean;
  symmetric: boolean;
  transitive: boolean;
  equivalence: boolean;
  chronological: boolean;
  description?: string | null;
};

export type LinkTime = {
  raw: number[];
  formatted: string;
  tick: number;
};

export type WeftLink = {
  graph: string;
  from: string;
  to: string;
  directed: boolean;
  label?: string | null;
  start?: LinkTime | null;
  end?: LinkTime | null;
  description?: string | null;
};

export type WeftDoc = {
  story?: {
    title?: string;
    description?: string;
    author?: string;
    date: { id: string; name: string; description?: string };
  };
  // 所选历法的时间分量数（后端解析时填充）
  calendar_components?: number;
  graphs?: Record<string, LinkGraphSpec>;
  entity?: Record<
    string,
    {
      id: string;
      avatar?: string;
      description?: string;
      base_time?: Phase | null;
      base_time_humanizer?: string;
      // 内置 key 之外的输入字段原样透传（后端 extra_props）
      extra_props?: Record<string, unknown>;
    }
  >;
  links?: WeftLink[];
  // 事件 ID → 事件定义
  event?: Record<string, Event>;
  // 事件列表名称 → 事件 ID 列表
  events?: Record<string, Events>;
};

export type Events = string[];

export type EntityTimeDelta = {
  start_tick: number;
  end_tick?: number;
  start_display: string;
  end_display?: string;
};

export type Event = {
  start_time: Phase;
  end_time?: Phase | null;
  description?: string;
  entities?: string[];
  start_tick?: number;
  end_tick?: number;
  start_display?: string;
  end_display?: string;
  entity_deltas?: Record<string, EntityTimeDelta>;
};
