# 原型文件

网页版画布的源文件镜像。人在网页版改原型，改完同步到这里。实现界面时以这里的文件为参考。

**不要手改这个目录**。要改原型，在网页版画布里改，再同步过来。读法和同步步骤见 `AGENTS.md`「原型怎么读」。

## 本地看

`.dc.html` 是模板源码，直接双击打开没有样式：根节点的 `class="{{rootClass}}"` 不会被替换，「墨格」的 token 全部失效，弹窗也会全部叠在一起。要用仓库里的预览器看：

```bash
python3 -m http.server 8000
```

在仓库根目录运行，然后打开 http://localhost:8000/tools/prototype-preview/ 。目录页列出全部画板，每张都有浅色和深色两个入口。画板可以点，画板之间的链接也能跳。地址参数 `dark=1` 是深色，`density=标准` 是项目页、体检页的标准密度。

## 同步记录

每次同步改写这一节。

| 项 | 值 |
| --- | --- |
| 画布 | https://claude.ai/artifact/WAK5PGWGuUmNNAYS8hFcXq |
| 版本 | 1794610127-a083 |
| 同步于 | 2026-11-14 |

## 画板

| 文件 | 画板 | 设计文档 |
| --- | --- | --- |
| `project/canvas.json` | 画板索引：位置、尺寸、标题、便签 | — |
| `project/DesignSystem.dc.html` | 方寸设计系统 · 墨格 | 6.10 |
| `project/Main.dc.html` | 项目 · 生效上下文 | 6.3 |
| `project/Checkup.dc.html` | 首次启动 · 体检报告 | 6.2 |
| `project/Library.dc.html` | 资产库 · 资产详情 | 6.4 |
| `project/Bundles.dc.html` | 套装 · 资产组合 | 6.5 |
| `project/Global.dc.html` | 全局 · 瘦身 | 6.6 |
| `project/Activity.dc.html` | 活动 · 时间线与回滚 | 6.7 |
