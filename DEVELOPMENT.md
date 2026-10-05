# 开发

## 环境

- Node.js + pnpm
- Rust（stable 工具链）

## 常用命令

    pnpm install        # 安装前端依赖
    pnpm tauri dev      # 开发模式
    pnpm tauri build    # 打包

## 打包注意

[wix314-binaries.zip](https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip)在国内极其容易下载失败，请使用代理或手动放置。[相关讨论](https://github.com/tauri-apps/tauri/issues/7338)

[Tauri 的 Win安装器配置](https://tauri.app/distribute/windows-installer/)

随便一提，打包出错得重新编译，比较费时间。

微软商店打包请参考[这里](https://huayemao.run/posts/337)

## 打包产物

`src-tauri/target/release/bundle/` 下有 nsis 和 msi 两个安装包。

## 目录结构

- `src/` — Svelte 前端
- `src-tauri/` — Rust 后端
- `src-tauri/src/` — 核心逻辑（config / paths / files / chapters / meta / watch）

详细的 Tauri 用法请参考 [Tauri 官方文档](https://tauri.app/)。