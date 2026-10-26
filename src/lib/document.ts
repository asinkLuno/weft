// 后端已按 src-tauri/src/weft_doc.rs 校验并滤掉未声明键，这里只读序列化结果
export type Phase = {
  components: number[];
  anchor: Phase | null;
};

export type WeftDoc = {
  story?: {
    title?: string;
    description?: string;
    author?: string;
    date: { name: string; description?: string };
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
      // 内置 key 之外的输入字段原样透传（后端 extra_props）
      extra_props?: Record<string, unknown>;
    }
  >;
  // 归属 DAG：每个节点只含实体 ID 与已解析的直接上级
  belong_dag?: Array<{ id: string; belong_to?: string[] }>;
  entity_link?: Record<
    string,
    Array<{ entities: [string, string]; relations: string }>
  >;
  // 事件 ID → 事件定义
  event?: Record<string, Event>;
  // 叙事线名称 → 事件 ID 列表
  narrative?: Record<string, Narrative>;
};

export type Narrative = string[];

export type Event = {
  start_time: Phase;
  end_time?: Phase | null;
  description?: string;
  entities?: string[];
  start_tick?: number;
  end_tick?: number;
  start_display?: string;
  end_display?: string;
};
