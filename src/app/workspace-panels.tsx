import type {
  DockviewApi,
  DockviewGroupPanel,
  IDockviewHeaderActionsProps,
  IDockviewPanelHeaderProps,
  IDockviewPanelProps,
  IWatermarkPanelProps,
  SerializedDockview,
} from "dockview-react";
import {
  BookOpen,
  Columns2,
  FileCode2,
  GitBranch,
  Info,
  type LucideIcon,
  Plus,
  RotateCcw,
  Rows2,
  Waypoints,
  X as XIcon,
} from "lucide-react";
import {
  type FunctionComponent,
  lazy,
  Suspense,
  useEffect,
  useRef,
  useState,
} from "react";
import { EmptyState } from "@/components/empty-state";
import { StoryView } from "@/features/story/story-view";
import { type Lang, strings } from "@/i18n";
import { DOCUMENT_TONE_STYLE } from "@/lib/ui";
import {
  useWorkspaceDoc,
  useWorkspaceEditor,
  useWorkspaceTone,
} from "./workspace-context";

const loadWeftEditorView = () =>
  import("@/features/editor/weft-editor-view").then((m) => ({
    default: m.WeftEditorView,
  }));
const loadEntityView = () =>
  import("@/features/entity/entity-view").then((m) => ({
    default: m.EntityView,
  }));
const loadEventsView = () => {
  void import("@/features/events/events-gantt");
  return import("@/features/events/events-view").then((m) => ({
    default: m.EventsView,
  }));
};
const loadInfoView = () =>
  import("@/features/info/info-view").then((m) => ({
    default: m.InfoView,
  }));

const WeftEditorView = lazy(loadWeftEditorView);
const EntityView = lazy(loadEntityView);
const EventsView = lazy(loadEventsView);
const InfoView = lazy(loadInfoView);

const PANEL_PRELOADERS: Partial<Record<PanelId, () => Promise<unknown>>> = {
  editor: loadWeftEditorView,
  entity: loadEntityView,
  events: loadEventsView,
  info: loadInfoView,
};

function preloadPanel(kind: PanelId | null): void {
  if (!kind) return;
  void PANEL_PRELOADERS[kind]?.();
}

export const PANEL_IDS = [
  "editor",
  "story",
  "entity",
  "events",
  "info",
] as const;
export type PanelId = (typeof PANEL_IDS)[number];

type PanelLabelKey = "panelEditor" | "tab1" | "tab2" | "tab4" | "tabInfo";

export const PANEL_META: Record<
  PanelId,
  { icon: LucideIcon; labelKey: PanelLabelKey }
> = {
  editor: { icon: FileCode2, labelKey: "panelEditor" },
  story: { icon: BookOpen, labelKey: "tab1" },
  entity: { icon: Waypoints, labelKey: "tab2" },
  events: { icon: GitBranch, labelKey: "tab4" },
  info: { icon: Info, labelKey: "tabInfo" },
};

const PANEL_ID_SET: ReadonlySet<string> = new Set(PANEL_IDS);

export function isPanelId(value: string): value is PanelId {
  return PANEL_ID_SET.has(value);
}

export function panelKindOf(panel: {
  id: string;
  params?: Record<string, unknown>;
}): PanelId | null {
  const paramKind = panel.params?.kind;
  if (typeof paramKind === "string" && isPanelId(paramKind)) {
    return paramKind;
  }
  if (isPanelId(panel.id)) {
    return panel.id;
  }
  const prefix = panel.id.split("-")[0];
  return prefix && isPanelId(prefix) ? prefix : null;
}

export function panelTitle(id: PanelId, lang: Lang): string {
  return strings[lang][PANEL_META[id].labelKey];
}

function nextPanelInstanceId(api: DockviewApi, kind: PanelId): string {
  if (!api.getPanel(kind)) return kind;
  let index = 2;
  while (api.getPanel(`${kind}-${index}`)) {
    index += 1;
  }
  return `${kind}-${index}`;
}

function addPanelInGroup(
  api: DockviewApi,
  kind: PanelId,
  lang: Lang,
  group: DockviewGroupPanel,
  direction?: "right" | "below",
): void {
  const id = nextPanelInstanceId(api, kind);
  api.addPanel({
    id,
    component: kind,
    title: panelTitle(kind, lang),
    params: { kind },
    position: direction
      ? { referenceGroup: group, direction }
      : { referenceGroup: group },
  });
}

function EditorPanel() {
  const { path, lang } = useWorkspaceDoc();
  const {
    draftStore,
    dirty,
    saving,
    diskStatus,
    revealRequest,
    onDraftChange,
    onSave,
    onForceSave,
    onReloadFromDisk,
  } = useWorkspaceEditor();

  return (
    <Suspense fallback={null}>
      <WeftEditorView
        path={path}
        draftStore={draftStore}
        dirty={dirty}
        saving={saving}
        diskStatus={diskStatus}
        revealRequest={revealRequest}
        lang={lang}
        onChange={onDraftChange}
        onSave={onSave}
        onForceSave={onForceSave}
        onReloadFromDisk={onReloadFromDisk}
      />
    </Suspense>
  );
}

function StoryPanel() {
  const { doc, lang } = useWorkspaceDoc();
  const t = strings[lang];

  return (
    <div className="@container h-full overflow-auto bg-base-100 p-4">
      {doc?.story ? (
        <StoryView doc={doc} lang={lang} />
      ) : (
        <EmptyState icon={BookOpen}>{t.storyEmpty}</EmptyState>
      )}
    </div>
  );
}

// 画布类视图（实体图/甘特图）需在面板首次可见时挂载才有真实宽高，之后常驻保留视角与交互状态。
function useMountWhenVisible(api: IDockviewPanelProps["api"]): boolean {
  const [mountedOnce, setMountedOnce] = useState(() => api.isVisible);

  useEffect(() => {
    if (mountedOnce) return;
    if (api.isVisible) {
      setMountedOnce(true);
      return;
    }
    const disposable = api.onDidVisibilityChange((event) => {
      if (event.isVisible) {
        setMountedOnce(true);
      }
    });
    return () => {
      disposable.dispose();
    };
  }, [api, mountedOnce]);

  return mountedOnce;
}

function EntityPanel({ api }: IDockviewPanelProps) {
  const { doc, lang, path, revealInEditor } = useWorkspaceDoc();
  const mountedOnce = useMountWhenVisible(api);

  return (
    <div className="flex h-full min-h-0 flex-col bg-base-100 p-4">
      {mountedOnce ? (
        <Suspense fallback={<EmptyState>{strings[lang].loading}</EmptyState>}>
          <EntityView
            doc={doc}
            lang={lang}
            path={path}
            onRevealInEditor={revealInEditor}
          />
        </Suspense>
      ) : null}
    </div>
  );
}

function InfoPanel() {
  const { lang } = useWorkspaceDoc();
  return (
    <div className="h-full overflow-auto bg-base-100 p-4">
      <Suspense fallback={null}>
        <InfoView lang={lang} />
      </Suspense>
    </div>
  );
}

function EventsPanel({ api }: IDockviewPanelProps) {
  const { doc, lang, revealInEditor } = useWorkspaceDoc();
  const mountedOnce = useMountWhenVisible(api);

  return (
    <div className="flex h-full min-h-0 flex-col bg-base-100 p-4">
      {mountedOnce ? (
        <Suspense fallback={null}>
          <EventsView doc={doc} lang={lang} onRevealInEditor={revealInEditor} />
        </Suspense>
      ) : null}
    </div>
  );
}

export const WORKSPACE_COMPONENTS: Record<
  PanelId,
  FunctionComponent<IDockviewPanelProps>
> = {
  editor: EditorPanel,
  story: StoryPanel,
  entity: EntityPanel,
  events: EventsPanel,
  info: InfoPanel,
};

function EditorDirtyDot() {
  const tone = useWorkspaceTone();
  if (tone === "synced") return null;
  return (
    <span
      className={`size-1.5 rounded-full ${DOCUMENT_TONE_STYLE[tone].dot}`}
      aria-hidden="true"
    />
  );
}

export function WorkspaceTab({ api, params }: IDockviewPanelHeaderProps) {
  const { lang } = useWorkspaceDoc();
  const kind = panelKindOf({ id: api.id, params });
  const Icon = kind ? PANEL_META[kind].icon : null;
  const title = kind ? panelTitle(kind, lang) : (api.title ?? api.id);
  const tabRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const target = tabRef.current?.parentElement;
    if (!target || !kind || !PANEL_PRELOADERS[kind]) return;
    const warm = () => preloadPanel(kind);
    target.addEventListener("pointerenter", warm, {
      passive: true,
      once: true,
    });
    target.addEventListener("focusin", warm, { passive: true, once: true });
    return () => {
      target.removeEventListener("pointerenter", warm);
      target.removeEventListener("focusin", warm);
    };
  }, [kind]);

  return (
    <div
      ref={tabRef}
      className="inline-flex h-full items-center gap-1.5 select-none"
    >
      {Icon && <Icon className="size-3.5 shrink-0" aria-hidden="true" />}
      <span className="whitespace-nowrap text-xs font-medium">{title}</span>
      {kind === "editor" && <EditorDirtyDot />}
      <button
        type="button"
        className="btn btn-ghost btn-xs btn-square"
        aria-label={`Close ${title}`}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          api.close();
        }}
      >
        <XIcon className="size-3" />
      </button>
    </div>
  );
}

function GroupActionMenu({
  label,
  icon: TriggerIcon,
  lang,
  align = "end",
  onSelect,
}: {
  label: string;
  icon: LucideIcon;
  lang: Lang;
  align?: "auto" | "end";
  onSelect: (kind: PanelId) => void;
}) {
  const [alignEnd, setAlignEnd] = useState(align === "end");

  return (
    <details
      className={alignEnd ? "dropdown dropdown-end" : "dropdown"}
      onToggle={(event) => {
        if (align !== "auto" || !event.currentTarget.open) return;
        const triggerRect = event.currentTarget.getBoundingClientRect();
        const bounds = event.currentTarget
          .closest(".dv-groupview")
          ?.getBoundingClientRect() ?? { left: 0, right: window.innerWidth };
        const spaceRight = bounds.right - triggerRect.left;
        const spaceLeft = triggerRect.right - bounds.left;
        setAlignEnd(spaceRight < 168 && spaceLeft > spaceRight);
      }}
    >
      <summary
        className="btn btn-ghost btn-xs btn-square"
        aria-label={label}
        title={label}
      >
        <TriggerIcon className="size-3.5" />
      </summary>
      <ul className="menu menu-xs dropdown-content z-20 mt-1 w-40 rounded-box border border-base-300 bg-base-100 p-1 shadow-md">
        <li className="menu-title text-xs">{label}</li>
        {PANEL_IDS.map((kind) => {
          const Icon = PANEL_META[kind].icon;
          return (
            <li key={kind}>
              <button
                type="button"
                onPointerEnter={() => preloadPanel(kind)}
                onFocus={() => preloadPanel(kind)}
                onClick={(event) => {
                  onSelect(kind);
                  event.currentTarget
                    .closest("details")
                    ?.removeAttribute("open");
                }}
              >
                <Icon className="size-3.5" />
                {panelTitle(kind, lang)}
              </button>
            </li>
          );
        })}
      </ul>
    </details>
  );
}

export function WorkspaceAddTabAction({
  containerApi,
  group,
}: IDockviewHeaderActionsProps) {
  const { lang } = useWorkspaceDoc();
  const t = strings[lang];

  return (
    <div className="flex h-full items-center px-1">
      <GroupActionMenu
        label={t.addTab}
        icon={Plus}
        lang={lang}
        align="auto"
        onSelect={(kind) => addPanelInGroup(containerApi, kind, lang, group)}
      />
    </div>
  );
}

export function WorkspaceGroupActions({
  containerApi,
  group,
}: IDockviewHeaderActionsProps) {
  const { lang } = useWorkspaceDoc();
  const t = strings[lang];

  return (
    <div className="inline-flex h-full items-center gap-0.5 px-1.5">
      <GroupActionMenu
        label={t.splitRight}
        icon={Columns2}
        lang={lang}
        onSelect={(kind) =>
          addPanelInGroup(containerApi, kind, lang, group, "right")
        }
      />
      <GroupActionMenu
        label={t.splitDown}
        icon={Rows2}
        lang={lang}
        onSelect={(kind) =>
          addPanelInGroup(containerApi, kind, lang, group, "below")
        }
      />
    </div>
  );
}

export function WorkspaceWatermark({ containerApi }: IWatermarkPanelProps) {
  const { lang } = useWorkspaceDoc();
  const t = strings[lang];

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 bg-base-100 p-6">
      <p className="text-sm text-base-content/60">{t.workspaceEmpty}</p>
      <div className="flex flex-wrap items-center justify-center gap-2">
        {PANEL_IDS.map((id) => {
          const Icon = PANEL_META[id].icon;
          return (
            <button
              key={id}
              type="button"
              className="btn btn-outline btn-sm border-base-300"
              onPointerEnter={() => preloadPanel(id)}
              onFocus={() => preloadPanel(id)}
              onClick={() => focusOrOpenPanel(containerApi, id, lang)}
            >
              <Icon className="size-3.5" />
              {panelTitle(id, lang)}
            </button>
          );
        })}
        <button
          type="button"
          className="btn btn-neutral btn-sm"
          onClick={() => applyDefaultLayout(containerApi, lang)}
        >
          <RotateCcw className="size-3.5" />
          {t.resetLayout}
        </button>
      </div>
    </div>
  );
}

export function applyDefaultLayout(api: DockviewApi, lang: Lang): void {
  api.clear();
  const editorPanel = api.addPanel({
    id: "editor",
    component: "editor",
    title: panelTitle("editor", lang),
    params: { kind: "editor" },
  });
  const storyPanel = api.addPanel({
    id: "story",
    component: "story",
    title: panelTitle("story", lang),
    params: { kind: "story" },
    position: { referencePanel: editorPanel, direction: "right" },
  });
  for (const kind of ["entity", "events", "info"] as const) {
    api.addPanel({
      id: kind,
      component: kind,
      title: panelTitle(kind, lang),
      params: { kind },
      position: { referencePanel: storyPanel, direction: "within" },
    });
  }
  storyPanel.api.setActive();
  if (api.width > 0) {
    editorPanel.group.api.setSize({
      width: Math.max(320, Math.round(api.width * 0.42)),
    });
  }
}

export function focusOrOpenPanel(
  api: DockviewApi,
  kind: PanelId,
  lang: Lang,
): void {
  const existing = api.panels.find((panel) => panelKindOf(panel) === kind);
  if (existing) {
    existing.api.setActive();
    return;
  }

  const id = nextPanelInstanceId(api, kind);
  const title = panelTitle(kind, lang);
  const params = { kind };

  if (kind === "editor") {
    const firstOther = api.panels[0];
    api.addPanel({
      id,
      component: kind,
      title,
      params,
      position: firstOther
        ? { referencePanel: firstOther, direction: "left" }
        : undefined,
    });
    return;
  }

  const siblingView = api.panels.find(
    (panel) => panelKindOf(panel) !== "editor",
  );
  if (siblingView) {
    api.addPanel({
      id,
      component: kind,
      title,
      params,
      position: { referencePanel: siblingView, direction: "within" },
    });
    return;
  }

  const editorPanel = api.panels.find(
    (panel) => panelKindOf(panel) === "editor",
  );
  api.addPanel({
    id,
    component: kind,
    title,
    params,
    position: editorPanel
      ? { referencePanel: editorPanel, direction: "right" }
      : undefined,
  });
}

function isSerializedDockview(value: unknown): value is SerializedDockview {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<SerializedDockview>;
  if (
    !candidate.grid ||
    !candidate.panels ||
    typeof candidate.panels !== "object"
  ) {
    return false;
  }
  const entries = Object.values(candidate.panels);
  if (entries.length === 0) return false;
  return entries.every(
    (panel) =>
      panel &&
      typeof panel === "object" &&
      typeof panel.id === "string" &&
      typeof panel.contentComponent === "string" &&
      isPanelId(panel.contentComponent),
  );
}

export function restoreLayout(api: DockviewApi, raw: unknown): boolean {
  if (!isSerializedDockview(raw)) return false;
  try {
    api.fromJSON(raw);
    return api.panels.length > 0;
  } catch {
    return false;
  }
}
