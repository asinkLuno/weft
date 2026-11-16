# 基础设施：脚手架、工具链、测试、部署

## 新建 React 项目（脚手架 / 框架）

- **Vite**：客户端渲染 SPA 的默认选择。社区最流行，开发体验和性能出色。适合不需要 SEO、不依赖服务端渲染的应用。
- **Next.js**：全栈应用的默认选择。内置多种渲染技术（SSR / SSG / ISR / RSC）、文件路由、API 路由。
- **TanStack Start**：2026 年 3 月达到 v1.0。类型安全路由 + 文件路由 + 服务器函数，值得关注的新秀。
- **React Router v7**：2024 年底与 Remix 合并，既可以当纯路由库用，也可以当全栈框架用。
- **Astro**：静态站点优先，默认只向浏览器发送 HTML/CSS，适合内容型网站、博客、文档站。
- 其他：RedwoodSDK（Cloudflare 优先，1.0 beta）、Waku（轻量 RSC 支持）。
- **React Compiler**：2025 年 10 月达到 v1.0，自动 memoize 组件和 hooks，新项目建议开启，替代大部分手动 useMemo/useCallback。

选择建议：纯前端工具、内部仪表盘 → Vite；要 SEO 或服务端逻辑 → Next.js；内容站 → Astro；想要更现代的全栈类型安全体验 → TanStack Start。

## 包管理器

- **npm**：默认、最广泛使用。
- **pnpm**：更快、磁盘高效，monorepo 支持出色（配合 Turborepo）。
- **Bun**：一体化工具链，安装最快。

默认推荐 pnpm，除非团队已统一其他选择。

## Lint 与格式化

- 前瞻选择：**oxlint + oxfmt**（Rust 实现，比 ESLint 快约 30 倍）、tsgo（TypeScript 7 beta）。
- 稳健单工具：**Biome v2**（lint + format 一体）。
- 保守选择：**ESLint + Prettier**（插件生态最全，依赖特定规则集时仍需它）。

选择建议：新项目追求速度 → Biome；依赖特定 ESLint 插件规则集（安全、a11y 等）→ ESLint + Prettier。

## 测试

- 单元 / 组件：**Vitest**（v3 浏览器模式已成熟）+ **React Testing Library**。
- E2E：**Playwright** 是明确的默认；Cypress 备选。
- Jest v30：留给遗留代码库和 React Native 场景。

## 托管部署

- **Vercel**：Next.js 最顺滑的托管方。
- **Netlify**：定价友好。
- **Cloudflare**：Workers / Pages / R2 / D1，edge 场景强。
- 自托管：**Coolify**（领先的开源 PaaS）；Render、Fly.io、Railway 均可选。
