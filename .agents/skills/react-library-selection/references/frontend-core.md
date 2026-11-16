# 前端核心库：状态、数据获取、路由、样式、UI、表单、动画、图表等

## 状态管理

- 内置优先：`useState` / `useReducer` + `useContext` 够用就不要加库。
- **Zustand**：全局状态的事实标准。文章作者"自 2021 年起没再选用其他状态管理库"。API 极简、体积小、无 Provider 包裹。
- **Redux Toolkit**：老项目常见。新项目只在团队已熟练或依赖其中间件生态时选。
- 备选：**XState**（复杂状态机）、**Jotai**（原子化状态）、**MobX**（遗留代码）。
- URL 状态：**nuqs**（把筛选、分页、排序等状态放进 URL，可分享可回退）。

## 数据获取

- 客户端获取（REST / GraphQL）：**TanStack Query**——缓存、重试、失效、devtools 一应俱全，默认选择。
- 服务端获取：优先 **React Server Components / Server Functions**（Next.js 等全栈框架内置），减少客户端数据管道。
- GraphQL：复杂场景 **Apollo Client v4**；轻量选 **urql**；**Relay** 小众。
- Redux 用户：**RTK Query**（已在 Redux 生态时不必另引 TanStack Query）。
- 全栈类型安全：**tRPC**（前后端同仓、类型端到端贯通）。

## 路由

- **React Router**：最常用。v7 起支持"库模式"（纯 SPA）和"框架模式"（全栈）两种用法。
- **TanStack Router**：趋势之选，TypeScript 类型推断一流——完全类型安全的路径与 search params。
- 已在 Next.js / TanStack Start / React Router v7 框架模式中时，直接用框架自带文件路由，不要再叠路由库。

## CSS 样式

- **Tailwind CSS v4**：行业标准。Oxide 引擎更快，配置改为 CSS 内 `@theme`。
- **CSS Modules**：组件级样式封装、零运行时，适合不想要工具链侵入的团队。
- 构建时 CSS-in-JS：**PandaCSS**、**vanilla-extract**；另有 Meta 的 **StyleX**、**UnoCSS**。
- className 条件组合：**clsx**。
- 避免：styled-components（维护模式）、Emotion（运行时方案与 RSC 冲突）——见 SKILL.md"不推荐"。

## UI 组件库

### 无头组件（趋势，配合 Tailwind）

- **shadcn/ui**：事实标准。组件代码直接拷进项目、完全可掌控，基于 Radix + Tailwind。
- **Radix**：shadcn/ui 的底层，无头原语最全。
- **Base UI**：2026 年 v1.0（MUI 团队与 React Aria 相关作者打造）。
- **React Aria**：可访问性最强。
- 其他：Ark UI、Ariakit、Daisy UI（Tailwind 组件类）、Headless UI、Tailwind UI。

### 完整组件库

- **MUI**：老牌、生态全。
- **Ant Design**：企业级、数据密集 B2B 后台的默认选择。
- **Mantine v8**、**Chakra UI v3**：开发者体验好、组件全。
- 其他：Hero UI、Park UI、PrimeReact。

选择建议：要品牌深度定制 → shadcn/ui + Radix；快速搭企业后台 → Ant Design；要成套主题化组件 → MUI / Mantine。

## 表单

- **React Hook Form + zod**：压倒性首选。非受控性能好，zod 负责校验与类型推断（schema 即类型）。
- **TanStack Form**：追求极致类型安全时选。
- **Conform**：配合服务器 action 的渐进增强表单。
- 不用：Formik、React Final Form（均不维护）。

## 动画

- **Motion**（原 Framer Motion）：事实标准，声明式 React 动画。
- **GSAP**：已 MIT 许可，复杂时间线动画。
- **react-spring**：物理动画。

## 图表可视化

- **Recharts**：默认推荐，也是 shadcn/ui 图表的基础。
- **Tremor**：SaaS 仪表盘专用。
- **visx**：低层、按需组合（Airbnb 出品）。
- 其他：nivo、react-chartjs、Victory。
- 底层方案：**D3**（需要完全掌控时）。

## 日期时间

- **date-fns v4**：函数式、tree-shake 友好。
- **Day.js**：极轻、moment.js 兼容 API。
- 注意：Temporal API 已进入 ES2026 标准，是未来的原生选择，新工具链可留意。

## 国际化

- **react-i18next**：生态最全的通用方案。
- **next-intl**：Next.js 项目。
- **Paraglide JS**：编译时类型安全的新秀。
- **Lingui**。

## 拖拽

- **dnd kit**：2025–2026 恢复活跃，现代默认。
- **Pragmatic drag-and-drop**（Atlassian）：react-beautiful-dnd 的继任方向。

## 文件上传

- **react-dropzone**：需要拖拽区时。
- 简单场景直接用原生 `input[type=file]`。
