---
name: react-library-selection
description: React 生态技术选型指南，基于 Robin Wieruch《React Libraries》2026 指南。当用户搭建新 React 项目、问"X 该用什么库"（状态管理、数据获取、路由、UI 组件、表单、动画、图表、测试、认证、ORM、桌面/移动等）、评审或替换现有依赖时使用——即使用户没有明说"选型"两个字。
---

# React 库选型指南

## 定位与时效

- 本技能是对 https://www.robinwieruch.de/react-libraries/ 的结构化快照（抓取于 2026-11）。
- React 生态变化快。若当前日期距快照超过一年，或对某库的版本、维护状态拿不准，先用 WebSearch 核实再下结论，不要凭记忆断言。
- 回答选型问题时给出"默认推荐 + 为什么 + 什么时候换备选"，而不是只报一个库名。

## 选型流程

1. 先判断项目形态：纯 SPA（客户端渲染）、全栈 Web 应用、静态站点、桌面应用、移动端。形态决定脚手架，脚手架又决定一批下游选择（路由、数据获取、认证方式）。
2. 在速查表里找该类别的默认答案。需要版本号、对比理由、陷阱或更完整的备选列表时，再读对应 references 文件（见下方路由表）。
3. 只回答用户问的类别，不要一次罗列全部推荐——本指南的核心原则是按需添加库。
4. 结论附一句适用条件，例如"老项目已有 Redux 就别为迁移而迁移，新项目用 Zustand"。

## 速查表

| 类别 | 默认推荐 | 主要备选 |
| --- | --- | --- |
| 新项目脚手架 | Vite（SPA）；Next.js（全栈） | TanStack Start、Astro（静态）、React Router v7（框架模式） |
| 包管理器 | pnpm | npm、Bun（安装最快） |
| 全局状态 | Zustand | Redux Toolkit（老项目）、Jotai、XState、nuqs（URL 状态） |
| 数据获取 | TanStack Query；服务端用 RSC | RTK Query、Apollo Client v4（复杂 GraphQL）、tRPC（全栈类型安全） |
| 路由 | React Router | TanStack Router（类型安全一流） |
| 样式 | Tailwind CSS v4 | CSS Modules、PandaCSS / vanilla-extract |
| className 组合 | clsx | — |
| UI 组件（无头） | shadcn/ui | Radix、Base UI、React Aria、Ark UI |
| UI 组件（完整库） | MUI；Ant Design（企业 B2B 后台） | Mantine、Chakra UI v3、Hero UI |
| 表单 | React Hook Form + zod | TanStack Form、Conform |
| 动画 | Motion（原 Framer Motion） | GSAP（已 MIT）、react-spring |
| 图表 | Recharts | Tremor（SaaS 仪表盘）、visx、nivo、D3（底层） |
| 单元/组件测试 | Vitest + React Testing Library | Jest（留给遗留代码 / React Native） |
| E2E 测试 | Playwright | Cypress |
| Lint + 格式化 | 激进：oxlint + oxfmt；稳健：Biome v2；保守：ESLint + Prettier | — |
| 认证（自托管） | Better Auth | Auth.js v5、Stack Auth |
| 认证（托管服务） | Clerk | AuthKit/WorkOS（企业 SSO）、Supabase Auth、Auth0、Cognito |
| 后端 API | Hono（edge 友好） | Express v5、Fastify、NestJS、Elysia（Bun） |
| ORM | Drizzle | Prisma（最成熟）、Kysely（查询构建器） |
| 不可变数据 | Immer | Mutative（更快） |
| 国际化 | react-i18next；Next.js 项目用 next-intl | Paraglide JS（编译时类型安全）、Lingui |
| 富文本 | Tiptap | BlockNote（Notion 风格）、Plate、Lexical、Slate |
| 日期时间 | date-fns v4 | Day.js |
| 拖拽 | dnd kit | Pragmatic drag-and-drop（Atlassian） |
| 文件上传 | react-dropzone 或原生 input[type=file] | — |
| 邮件模板 | react-email | jsx-email；发送服务推荐 Resend |
| 支付 | Stripe | Paddle、Polar.sh、Lemon Squeezy（MoR 模式） |
| 桌面应用 | Tauri 2.0 | Electron（生态最成熟） |
| 移动端 | React Native + Expo | Tamagui（跨端样式）、Solito |
| VR/AR | react-three-fiber + drei + xr | — |
| 组件文档 | Storybook 9 | Ladle（更快）、React Cosmos |
| 设计原型 | v0 by Vercel | Lovable、Bolt.new；Figma（协作标准） |
| 部署托管 | Vercel（Next.js）/ Netlify | Cloudflare（Workers/Pages）、Coolify（自托管 PaaS）、Fly.io、Railway |
| 渲染性能 | React Compiler（v1.0，自动 memoize） | 手动 useMemo / useCallback |

## 不推荐（新项目避开）

- **styled-components**：已进入维护模式；**Emotion**：运行时 CSS-in-JS 与 React Server Components 冲突。
- **Formik、React Final Form**：均不再维护。
- **Jest** 用于新 Web 项目：选 Vitest（Jest 留给遗留代码库和 React Native）。
- **Relay、MobX**：仅限小众或遗留场景。

## References 路由

需要细节（版本号、对比理由、迁移与陷阱）时再读对应文件：

- `references/foundation.md` — 脚手架与框架对比、包管理器、lint/format、测试、托管部署
- `references/frontend-core.md` — 状态、数据获取、路由、样式、UI 库、表单、动画、图表、日期、i18n、拖拽、上传
- `references/fullstack-and-specialized.md` — 认证、后端 API、数据库/ORM、富文本、支付、邮件、桌面、移动、VR/AR、原型、组件文档

## 核心选型原则

React 本身只负责视图层，生态的灵活选择是特性而非负担：从小规模起步，遇到具体问题再加库；若现有核心库已满足需求，保持轻量配置即可，不要为"最佳实践"而堆依赖。
