# 基本约定

- 需要查库/框架文档时，用 Context7。
- 处于快速开发阶段，可以破坏性重构；不要写任何测试用例。
- 增删 Rust 依赖用 `cargo add` / `cargo remove`；永远不要手工修改 Cargo.lock。
- 前端组件用 shadcn/ui，增删组件用命令行（`npx shadcn add <component>`），不要手工拷贝。
- 不要自己启动 `tauri dev` / `tauri build`；应用由用户自己启动、目视验证。改动验证到 `cargo check` / `yarn build` / `yarn lint` 为止。

