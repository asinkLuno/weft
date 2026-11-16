# 全栈与专项：认证、后端、数据库、富文本、支付、桌面、移动等

## 认证

### 自托管开源

- **Better Auth**：自托管开源的事实标准，文章作者的新默认。
- **Auth.js v5**（NextAuth）：注意其维护者已指向 Better Auth，新项目选前留意。
- **Stack Auth**：备选。

### 托管服务

- **Clerk**：开发体验好，默认推荐。
- **AuthKit / WorkOS**：企业 SSO 场景。
- **Kinde**：付费方案。
- BaaS 自带：Supabase Auth、Convex Auth、Firebase Auth（配合各自 BaaS 时顺手）。
- 企业遗留：Auth0、AWS Cognito（已在 AWS 栈时）。

## 后端 API

- 全栈框架内置优先：Next.js Server Actions、TanStack Start Server Functions 够用就先用。
- **Hono**：edge 运行时领先者，轻快，可部署到 Cloudflare Workers 等多种运行时。
- **tRPC v11**：前后端端到端类型安全。
- 传统选择：**Express v5**、**Fastify**、**NestJS**（大型工程化项目）。
- **Elysia**：Bun 优先。

## 数据库与 ORM

- **Drizzle ORM**：2025 年末下载量已超过 Prisma；serverless / edge 场景的默认；SQL-like API，生成物轻。
- **Prisma**：最成熟、DX 好，schema 优先，适合传统长驻服务。
- **Kysely**：轻量 SQL 查询构建器（不是 ORM）。
- BaaS 数据库：**Supabase**（Postgres + 认证 + 存储）、Firebase、**Convex**（响应式后端）、**Neon**（serverless Postgres）、Turso、Xata。

## 富文本编辑器

- **Tiptap**：默认选择（基于 ProseMirror，扩展生态好）。
- **BlockNote**：Notion 风格块编辑器，开箱即用。
- **Plate**：基于 Slate 的组件化方案。
- **Lexical**（Meta 出品）、**Slate**（底层、需自己组装）。

## 支付

- **Stripe / PayPal**：标准方案。
- Merchant of Record（替你处理全球税务合规）：Paddle、Polar.sh、Lemon Squeezy、Braintree。独立开发者卖软件授权时 MoR 常更省心。

## 邮件

- 模板：**react-email**（标准，用 React 组件写邮件模板）、jsx-email。
- 发送服务：**Resend** 推荐。

## 桌面应用

- **Tauri 2.0**：默认推荐。Rust 内核，安装包体积比 Electron 小一个数量级，且支持构建移动端目标。前端直接复用 React 代码。
- **Electron**：生态最成熟，需要 Node 深度集成或团队只有 JS 能力时选。

## 移动端

- **React Native + Expo**：Expo 已成默认——文件路由（expo-router）、构建、OTA 更新一站式。
- 跨端样式：**Tamagui**（React Native + Web 通用）。
- RN 与 Next.js 共享导航：**Solito**。

## VR / AR

- **react-three-fiber**：Three.js 的 React 渲染器。
- **@react-three/drei**：常用辅助组件集。
- **@react-three/xr**：XR（VR/AR）支持。

## 设计与原型

- **v0 by Vercel**：AI 生成 React UI 的默认。
- **Lovable**、**Bolt.new**：备选。
- **Figma**：协作设计的行业标准。

## 组件文档 / 工作台

- **Storybook 9**：标准选择。
- **Ladle**：更快更轻的备选。
- **React Cosmos**。
- 项目文档站：**Docusaurus**。
