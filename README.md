# AI Editor (Tauri + Vue + TypeScript + Deno)

一个基于 Tauri 2 + Vue 3 + Monaco 的桌面编辑器，目标体验接近主流 IDE：

- Monaco 代码编辑与多语言支持
- Markdown 预览（GFM 风格）
- 项目树与文件操作（新建、重命名、删除、复制/粘贴）
- 集成终端与 Git 面板
- HTML 一键在系统浏览器打开

## 技术栈

- 前端：`Vue 3`、`TypeScript`、`Vite`、`Monaco Editor`
- 桌面：`Tauri 2`（Rust）
- 运行与构建任务：`Deno`

## 环境要求

- Deno 2.x
- Rust stable（建议通过 `rustup` 安装）
- Windows 建议安装 Visual Studio C++ 构建工具

## 常用命令

在项目根目录执行：

```powershell
# 启动前端开发服务（仅前端）
deno task dev

# 启动 Tauri 桌面开发模式
deno task tauri dev

# 类型检查 + 前端构建
deno task build

# 打包桌面应用
deno task tauri build
```

## 应用图标生成（Tauri）

将源 SVG 放到 `src-tauri/icons/icon.svg` 后执行：

```powershell
deno task tauri icon src-tauri/icons/icon.svg
```

会自动生成 `png/icns/ico` 等文件并供打包使用。

## 目录说明

- `src/`：前端代码
- `src/components/layout/`：核心布局（编辑器、项目树、标题栏等）
- `src-tauri/src/commands/`：Rust 端命令（文件系统、终端、Git）
- `src-tauri/capabilities/`：Tauri 权限能力配置

## 已做的关键优化

- 大文件 Monaco 自动降级（减少渲染和诊断开销）
- Git 装饰刷新降频 + 缓存
- `git_tree_decorations` 单次刷新只跑 1 条 `git` 命令
- Windows 下 Git 子进程隐藏控制台窗口（避免 `git.exe` 闪屏）

## 常见问题

- **`tauri dev` 能跑，`tauri build` 报模块找不到**  
  先确保 npm 依赖缓存已刷新并生成 `node_modules`：
  ```powershell
  deno cache --reload --node-modules-dir npm:vue-tsc npm:vite
  ```

- **PowerShell 删除 `node_modules` 报参数错误**  
  使用：
  ```powershell
  Remove-Item -Recurse -Force node_modules
  ```

- **Rust 提示 incremental artifact corrupt**  
  一般可忽略；若反复出现可执行：
  ```powershell
  cargo clean
  ```
