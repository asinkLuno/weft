export type Phase = {
  components: number[];
  anchor: Phase | null;
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
  entity?: Record<
    string,
    {
      id: string;
      avatar?: string;
      description?: string;
      base_time?: Phase | null;
      base_time_humanizer?: string;
      tags?: string[];
      // 内置 key 之外的输入字段原样透传（后端 extra_props）
      extra_props?: Record<string, unknown>;
    }
  >;
  // 归属 DAG：每个节点只含实体 ID 与已解析的直接上级
  belong_dag?: Array<{ id: string; belong_to?: string[] }>;
  entity_link?: Record<
    string,
    Array<{ entities: [string, string]; relations: string; directed: boolean }>
  >;
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
