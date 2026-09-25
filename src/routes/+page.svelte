<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    api,
    type AppConfig,
    type ChapterInfo,
    type FileEntry,
    type PathStatus,
    type SavePathMode,
  } from "$lib/api";
  import type { MenuItem, Dialog } from "$lib/types";
  import { displayPath, fmtSize } from "$lib/format";

  // ---------- 状态 ----------
  let config = $state<AppConfig | null>(null);
  let statuses = $state<Record<string, PathStatus>>({});
  let currentId = $state<string | null>(null);
  let files = $state<FileEntry[]>([]);
  let selected = $state<Set<string>>(new Set());
  let chapters = $state<ChapterInfo[]>([]);
  let toast = $state<{ text: string; kind: "info" | "error" } | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let dialog = $state<Dialog | null>(null);
  let noteEdit = $state<{ number: number; value: string } | null>(null);
  let sidebarCollapsed = $state(false);
  let showSettings = $state(false);
  let showExcludeEditor = $state(false);
  let hiddenText = $state("");
  let riskText = $state("");
  let showSmartSelect = $state(false);
  let smartInclude = $state("\\d");
  let smartIncludeMode = $state<"regex" | "substring">("regex");
  let smartExclude = $state("");
  let smartOnlyVisible = $state(true);
  let appVersion = $state("");

  let current = $derived(
    config?.savePaths.find((p) => p.id === currentId) ?? null,
  );
  let visibleFiles = $derived(
    files.filter((f) => config?.showHiddenEntries || !f.hidden),
  );
  let currentMissing = $derived(
    currentId ? statuses[currentId]?.exists === false : false,
  );
  let selectedCount = $derived(selected.size);

  let refreshTimer: number | null = null;
  let unlistenWatch: (() => void) | null = null;

  // ---------- 主题 ----------
  $effect(() => {
    if (!config) return;
    const t = config.theme;
    const root = document.documentElement;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      root.dataset.theme = t === "system" ? (mq.matches ? "dark" : "light") : t;
    };
    apply();
    if (t === "system") {
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  });

  // ---------- 菜单关闭 ----------
  $effect(() => {
    if (!menu) return;
    const close = () => (menu = null);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") menu = null;
    };
    const t = setTimeout(() => {
      window.addEventListener("click", close);
      window.addEventListener("contextmenu", close);
      window.addEventListener("keydown", onKey);
    }, 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("click", close);
      window.removeEventListener("contextmenu", close);
      window.removeEventListener("keydown", onKey);
    };
  });

  // ---------- 启动 ----------
  onMount(async () => {
    appVersion = await getVersion();
    const loaded = await api.loadConfig();
    config = loaded;
    await refreshStatuses();
    const last = loaded.savePaths[loaded.savePaths.length - 1];
    if (last) await selectPath(last.id);

    unlistenWatch = await listen("save-path-changed", () => {
      scheduleRefresh();
    });
  });

  const REPO_URL = "https://github.com/AuthorZiXi/savedata-chapter";
  const DEEPSEEK_URL = "https://www.deepseek.com";

  onDestroy(() => {
    unlistenWatch?.();
    if (refreshTimer) clearTimeout(refreshTimer);
  });

  function scheduleRefresh() {
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = window.setTimeout(async () => {
      refreshTimer = null;
      await refreshFiles();
      await refreshChapters();
    }, 300);
  }

  // ---------- 数据操作 ----------
  async function refreshStatuses() {
    const list = await api.validateSavePaths();
    const map: Record<string, PathStatus> = {};
    for (const s of list) map[s.id] = s;
    statuses = map;
  }

  async function selectPath(id: string) {
    currentId = id;
    selected = new Set();
    if (statuses[id]?.exists === false) {
      files = [];
      chapters = [];
      await api.watchSavePath(null);
      return;
    }
    await api.watchSavePath(id);
    await refreshFiles();
    await refreshChapters();
  }

  async function refreshFiles() {
    if (!currentId || statuses[currentId]?.exists === false) {
      files = [];
      return;
    }
    try {
      files = await api.listSaveFiles(currentId);
      const present = new Set(files.map((f) => f.name));
      const next = new Set([...selected].filter((n) => present.has(n)));
      if (next.size !== selected.size) selected = next;
    } catch (e) {
      files = [];
      showToast(String(e), "error");
    }
  }

  async function refreshChapters() {
    if (!currentId || statuses[currentId]?.exists === false) {
      chapters = [];
      return;
    }
    try {
      chapters = await api.listChapters(currentId);
    } catch (e) {
      chapters = [];
      showToast(String(e), "error");
    }
  }

  async function persistConfig() {
    const cfg = config;
    if (cfg) await api.saveConfig(cfg);
  }

  // ---------- 路径操作 ----------
  async function addPath() {
    const picked = await open({ directory: true, multiple: false });
    if (!picked || typeof picked !== "string") return;
    try {
      const entry = await api.addSavePath(picked, null);
      // 关键：重新拉取配置，让前端拿到最新 savePaths
      config = await api.loadConfig();
      await refreshStatuses();
      await selectPath(entry.id);
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  function removePath(id: string) {
    const cfg = config;
    if (!cfg) return;
    const entry = cfg.savePaths.find((p) => p.id === id);
    if (!entry) return;

    dialog = {
      title: "删除路径注册",
      body: `确定要从列表中移除这条注册吗？\n${entry.name ?? entry.path}\n\n（不会删除任何实际文件）`,
      danger: true,
      confirmText: "移除",
      onConfirm: async () => {
        await api.removeSavePath(id);
        cfg.savePaths = cfg.savePaths.filter((p) => p.id !== id);
        await persistConfig();
        await refreshStatuses();
        if (currentId === id) {
          currentId = null;
          files = [];
          chapters = [];
          await api.watchSavePath(null);
        }
      },
    };
  }

  async function relocatePath(id: string) {
    const picked = await open({ directory: true, multiple: false });
    if (!picked || typeof picked !== "string") return;
    const result = await api.relocateSavePath(id, picked);
    if (result.kind === "success") {
      await refreshStatuses();
      config = await api.loadConfig();
      await selectPath(id);
      showToast("路径已找回", "info");
    } else {
      const tried = result.candidatesTried.map((p) => "· " + p).join("\n");
      dialog = {
        title: "未能找回路径",
        body: `根据你选择的文件夹没有找到原路径。\n\n尝试过的候选路径：\n${tried}\n\n原记录已保留，你可以再次尝试。`,
        confirmText: "知道了",
        onConfirm: () => {},
      };
    }
  }

  // ---------- 文件列表 ----------
  function toggleFile(name: string) {
    const next = new Set(selected);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    selected = next;
  }

  function selectAll() {
    selected = new Set(visibleFiles.map((f) => f.name));
  }
  function selectNone() {
    selected = new Set();
  }
  function invertSelection() {
    const next = new Set<string>();
    for (const f of visibleFiles) if (!selected.has(f.name)) next.add(f.name);
    selected = next;
  }
  function selectWithDigit() {
    const next = new Set<string>();
    for (const f of visibleFiles) if (/\d/.test(f.name)) next.add(f.name);
    selected = next;
  }

  function fileMenu(e: MouseEvent) {
    e.preventDefault();
    const id = currentId;
    const cfg = config;
    if (!id || !cfg) return;
    const path = cfg.savePaths.find((p) => p.id === id)?.path;
    if (!path) return;

    const items: MenuItem[] = [
      { label: "刷新", action: refreshFiles },
      { label: "全选", action: selectAll },
      { label: "全不选", action: selectNone },
      { label: "反选", action: invertSelection },
      { label: "快速选中：带数字", action: selectWithDigit },
      { label: "智能选中…", action: openSmartSelect },
      { separator: true },
      {
        label: "打开存档文件夹",
        action: () => api.revealInExplorer(path, null),
      },
      { separator: true },
      {
        label: "移入回收站 (危险)",
        danger: true,
        action: () => confirmTrash(),
      },
    ];
    openMenu(e, items);
  }

  function confirmTrash() {
    if (!currentId || selected.size === 0) return;
    const list = [...selected];
    dialog = {
      title: "移入回收站",
      body: `将把选中的 ${list.length} 个文件移入回收站：\n\n${list.join("\n")}`,
      danger: true,
      confirmText: "移入回收站",
      onConfirm: async () => {
        try {
          await api.trashFiles(currentId!, list);
          selected = new Set();
          await refreshFiles();
          showToast("已移入回收站", "info");
        } catch (e) {
          showToast(String(e), "error");
        }
      },
    };
  }

  // ---------- 章节操作 ----------
  async function saveChapter(mode: SavePathMode) {
    if (!currentId || selected.size === 0) return;
    const list = [...selected];
    const existing = chapters.map((c) => c.number);
    const next = existing.length ? Math.max(...existing) + 1 : 1;
    dialog = {
      title: "保存章节",
      body: `将把选中的 ${list.length} 个文件以「${mode === "copy" ? "复制" : "移动"}」方式保存为第 ${next} 章。\n\n${list.join("\n")}`,
      confirmText: "保存",
      onConfirm: async () => {
        try {
          await api.createChapter(currentId!, list, mode, null);
          selected = new Set();
          await refreshFiles();
          await refreshChapters();
          showToast(`已保存为第 ${next} 章`, "info");
        } catch (e) {
          showToast(String(e), "error");
        }
      },
    };
  }

  function chapterMenu(e: MouseEvent, c: ChapterInfo) {
    e.preventDefault();
    e.stopPropagation();
    const items: MenuItem[] = [
      { label: "刷新列表", action: refreshChapters },
      { label: "读取到存档目录", action: () => loadChapter(c.number) },
      {
        label: "编辑备注",
        action: () =>
          (noteEdit = { number: c.number, value: c.meta?.note ?? "" }),
      },
      {
        label: "在资源管理器中打开",
        action: () => api.revealInExplorer(c.path, null),
      },
      { separator: true },
      {
        label: "移入回收站 (危险)",
        danger: true,
        action: () => confirmDeleteChapter(c.number),
      },
    ];
    openMenu(e, items);
  }

  function chapterListMenu(e: MouseEvent) {
    // 只在空白处或任意处触发都行，这里简单处理：任何位置都可
    e.preventDefault();
    const items: MenuItem[] = [
      { label: "刷新列表", action: refreshChapters },
      { label: "保存为副本", action: () => saveChapter("copy") },
      { label: "保存并移动", action: () => saveChapter("move") },
    ];
    openMenu(e, items);
  }

  function loadChapter(n: number) {
    if (!currentId) return;
    // 简单冲突检测：检查章节内文件与当前存档目录是否有重名
    // 更精确的检测需要后端支持，这里先弹通用确认
    dialog = {
      title: "读取章节",
      body: `确定要把第 ${n} 章的文件复制回存档目录吗？\n如果存档目录已有同名文件，会按你选择的方式处理。`,
      confirmText: "覆盖同名文件",
      onConfirm: async () => {
        try {
          const r = await api.loadChapter(currentId!, n, "overwrite");
          await refreshFiles();
          showToast(`复制 ${r.copied}，覆盖 ${r.overwritten}`, "info");
        } catch (e) {
          showToast(String(e), "error");
        }
      },
    };
  }

  function confirmDeleteChapter(n: number) {
    if (!currentId) return;
    dialog = {
      title: "删除章节",
      body: `确定要把第 ${n} 章移入回收站吗？`,
      danger: true,
      confirmText: "移入回收站",
      onConfirm: async () => {
        try {
          await api.deleteChapter(currentId!, n);
          await refreshChapters();
          showToast(`已删除第 ${n} 章`, "info");
        } catch (e) {
          showToast(String(e), "error");
        }
      },
    };
  }

  async function saveNote() {
    if (!currentId || !noteEdit) return;
    try {
      await api.updateChapterNote(
        currentId,
        noteEdit.number,
        noteEdit.value || null,
      );
      noteEdit = null;
      await refreshChapters();
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  // ---------- 通用 ----------
  function openMenu(e: MouseEvent, items: MenuItem[]) {
    e.preventDefault();
    const w = 200;
    const h = items.length * 30 + 12;
    const x = Math.min(e.clientX, window.innerWidth - w);
    const y = Math.min(e.clientY, window.innerHeight - h);
    menu = { x, y, items };
  }

  function showToast(text: string, kind: "info" | "error" = "info") {
    toast = { text, kind };
    setTimeout(() => (toast = null), 2500);
  }

  function setTheme(v: "system" | "light" | "dark") {
    const cfg = config;
    if (!cfg) return;
    cfg.theme = v;
    void persistConfig();
  }

  function setSavePathMode(m: SavePathMode) {
    const cfg = config;
    if (!cfg) return;
    cfg.savePathMode = m;
    void persistConfig();
  }

  function toggleShowHidden(e: Event) {
    const cfg = config;
    if (!cfg) return;
    cfg.showHiddenEntries = (e.target as HTMLInputElement).checked;
    void persistConfig();
  }

  function openExcludeEditor() {
    const cfg = config;
    if (!cfg) return;
    hiddenText = cfg.excludeRules.hidden.join("\n");
    riskText = cfg.excludeRules.risk.join("\n");
    showExcludeEditor = true;
  }

  function saveExcludeRules() {
    const cfg = config;
    if (!cfg) return;
    const split = (s: string) =>
      s
        .split("\n")
        .map((x) => x.trim())
        .filter(Boolean);
    cfg.excludeRules.hidden = split(hiddenText);
    cfg.excludeRules.risk = split(riskText);
    void persistConfig();
    showExcludeEditor = false;
    void refreshFiles();
  }

  async function exportConfig() {
    const dest = await save({
      defaultPath: "savedata-chapter-config.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!dest) return;
    try {
      await api.exportConfig(dest);
      showToast("配置已导出", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  }

  async function importConfig() {
    const src = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!src || typeof src !== "string") return;
    dialog = {
      title: "导入配置",
      body: "导入会替换当前所有配置（路径列表、排除规则、主题、默认模式等）。\n确定要继续吗？",
      confirmText: "导入",
      onConfirm: async () => {
        try {
          const cfg = await api.importConfig(src);
          config = cfg;
          await refreshStatuses();
          const first = cfg.savePaths[0];
          if (first) {
            await selectPath(first.id);
          } else {
            currentId = null;
            files = [];
            chapters = [];
            await api.watchSavePath(null);
          }
          showToast("配置已导入", "info");
        } catch (e) {
          showToast(String(e), "error");
        }
      },
    };
  }

  let smartPreview = $derived.by(() => {
    const src = smartOnlyVisible ? visibleFiles : files;
    const excludeList = smartExclude
      .split("\n")
      .map((s) => s.trim().toLowerCase())
      .filter(Boolean);

    return src.filter((f) => {
      if (excludeList.some((x) => f.name.toLowerCase().includes(x)))
        return false;
      if (smartIncludeMode === "regex") {
        try {
          if (!smartInclude) return true;
          return new RegExp(smartInclude).test(f.name);
        } catch {
          return false;
        }
      }
      return smartInclude ? f.name.includes(smartInclude) : true;
    });
  });

  let smartIncludeValid = $derived.by(() => {
    if (smartIncludeMode !== "regex" || !smartInclude) return true;
    try {
      new RegExp(smartInclude);
      return true;
    } catch {
      return false;
    }
  });

  function openSmartSelect() {
    const cfg = config;
    if (!cfg) return;
    smartInclude = "\\d";
    smartIncludeMode = "regex";
    smartExclude = cfg.excludeRules.hidden.join("\n");
    smartOnlyVisible = !cfg.showHiddenEntries;
    showSmartSelect = true;
  }

  function applySmartSelect() {
    selected = new Set(smartPreview.map((f) => f.name));
    showSmartSelect = false;
    showToast(`已选中 ${selected.size} 个文件`, "info");
  }
</script>

<svelte:head>
  <title>SaveData Chapter</title>
</svelte:head>

<div class="app">
  <button
    class="sidebar-handle"
    class:collapsed={sidebarCollapsed}
    onclick={() => (sidebarCollapsed = !sidebarCollapsed)}
    title={sidebarCollapsed ? "展开侧栏" : "折叠侧栏"}
    >{sidebarCollapsed ? "›" : "‹"}</button
  >
  <!-- 侧栏 -->
  <aside class="sidebar" class:collapsed={sidebarCollapsed}>
    <div class="sidebar-head">
      <span class="title">存档路径</span>
      <button class="icon-btn" onclick={addPath} title="添加路径">+</button>
      <button
        class="icon-btn"
        onclick={() => (showSettings = true)}
        title="设置">⚙</button
      >
    </div>
    <ul class="path-list">
      {#each config?.savePaths ?? [] as p (p.id)}
        <li
          class:active={currentId === p.id}
          class:missing={statuses[p.id]?.exists === false}
          oncontextmenu={(e) =>
            openMenu(e, [
              { label: "重新定位", action: () => relocatePath(p.id) },
              {
                label: "删除注册",
                danger: true,
                action: () => removePath(p.id),
              },
            ])}
        >
          <button class="path-btn" onclick={() => selectPath(p.id)}>
            {displayPath(p)}
          </button>
        </li>
      {/each}
      {#if (config?.savePaths ?? []).length === 0}
        <li class="empty">
          <div class="empty-center">
            <div>
              <p>还没有注册存档路径</p>
              <p class="muted">
                点左上「+」添加一个游戏存档文件夹，或从设置里导入配置
              </p>
            </div>
          </div>
        </li>
      {/if}
    </ul>
  </aside>

  <!-- 主区域 -->
  <main>
    {#if currentMissing}
      <div class="missing-banner">
        <span>当前路径已失效</span>
        <button onclick={() => relocatePath(currentId!)}>重新定位</button>
      </div>
    {:else if current}
      <div class="file-pane">
        <div class="pane-head">
          <span>文件列表</span>
          <span class="muted">{selectedCount} / {visibleFiles.length} 已选</span
          >
        </div>
        <ul class="file-list" oncontextmenu={fileMenu}>
          {#each visibleFiles as f (f.name)}
            <li>
              <label>
                <input
                  type="checkbox"
                  checked={selected.has(f.name)}
                  onchange={() => toggleFile(f.name)}
                />
                <span class:risk={f.risk} class:dim={f.hidden}>{f.name}</span>
                <span class="size">{fmtSize(f.size)}</span>
              </label>
            </li>
          {/each}
          {#if visibleFiles.length === 0}
            <li class="empty">没有文件</li>
          {/if}
        </ul>
      </div>

      <div class="chapter-pane">
        <div class="pane-head">
          <span>章节</span>
          <div class="pane-actions">
            <button
              onclick={() => config && saveChapter(config.savePathMode)}
              disabled={selectedCount === 0}
            >
              保存章节
            </button>
          </div>
        </div>
        <ul class="chapter-list" oncontextmenu={chapterListMenu}>
          {#each chapters as c (c.number)}
            <li oncontextmenu={(e) => chapterMenu(e, c)}>
              <span class="num">第 {c.number} 章</span>
              {#if c.meta?.note}
                <span class="note">{c.meta.note}</span>
              {/if}
              <span class="muted">{c.fileCount} 个文件</span>
              <button onclick={() => loadChapter(c.number)}>读取</button>
            </li>
          {/each}
          {#if chapters.length === 0}
            <li class="empty">还没有章节</li>
          {/if}
        </ul>
      </div>
    {:else}
      <div class="empty-center">请在左侧选择一个存档路径</div>
    {/if}
  </main>
</div>

<!-- 右键菜单 -->
{#if menu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="context-menu"
    style="left: {menu.x}px; top: {menu.y}px"
    onclick={(e) => e.stopPropagation()}
    transition:fade={{ duration: 80 }}
  >
    {#each menu.items as item}
      {#if item.separator}
        <div class="menu-sep"></div>
      {:else}
        <button
          class:danger={item.danger}
          onclick={(e) => {
            e.stopPropagation();
            const action = item.action;
            menu = null;
            action?.();
          }}>{item.label}</button
        >
      {/if}
    {/each}
  </div>
{/if}

<!-- 确认对话框 -->
{#if dialog}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onclick={() => (dialog = null)}
    transition:fade={{ duration: 120 }}
  >
    <div
      class="dialog"
      onclick={(e) => e.stopPropagation()}
      transition:scale={{ start: 0.96, duration: 150, easing: cubicOut }}
    >
      <h3>{dialog.title}</h3>
      <pre>{dialog.body}</pre>
      <div class="actions">
        <button onclick={() => (dialog = null)}>取消</button>
        <button
          class:danger={dialog.danger}
          onclick={async () => {
            const fn = dialog!.onConfirm;
            dialog = null;
            await fn();
          }}>{dialog.confirmText ?? "确定"}</button
        >
      </div>
    </div>
  </div>
{/if}

{#if showSettings && config}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onclick={() => (showSettings = false)}
    transition:fade={{ duration: 120 }}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="dialog settings"
      onclick={(e) => e.stopPropagation()}
      transition:scale={{ start: 0.96, duration: 150, easing: cubicOut }}
    >
      <h3>设置</h3>

      <div class="field">
        <span>主题</span>
        <div class="seg">
          {#each [["system", "跟随系统"], ["light", "浅色"], ["dark", "深色"]] as [v, label]}
            <button
              class:active={config.theme === v}
              onclick={() => setTheme(v as "system" | "light" | "dark")}
              >{label}</button
            >
          {/each}
        </div>
      </div>

      <div class="field">
        <span>显示隐藏项</span>
        <input
          type="checkbox"
          checked={config.showHiddenEntries}
          onchange={toggleShowHidden}
        />
      </div>

      <div class="field">
        <span>默认保存模式</span>
        <div class="seg">
          <button
            class:active={config.savePathMode === "copy"}
            onclick={() => setSavePathMode("copy")}
            title="原文件保留，游戏内会提示覆盖">复制</button
          >
          <button
            class:active={config.savePathMode === "move"}
            onclick={() => setSavePathMode("move")}
            title="原文件消失，适合热加载引擎">移动</button
          >
        </div>
      </div>

      <p class="hint">
        默认保存模式只用于右键菜单、快捷键等入口；主按钮仍显式区分复制/移动。
      </p>

      <div class="field">
        <span>排除规则</span>
        <button onclick={openExcludeEditor}>
          {config.excludeRules.hidden.length + config.excludeRules.risk.length} 条，编辑
        </button>
      </div>

      <div class="field">
        <span>配置</span>
        <div class="seg">
          <button onclick={exportConfig}>导出</button>
          <button onclick={importConfig}>导入</button>
          <button onclick={() => api.openConfigDir()}>打开目录</button>
        </div>
      </div>

      <div class="about">
        <div class="row">
          <span>SaveData Chapter v{appVersion}</span>
          <button class="link" onclick={() => openUrl(REPO_URL)}
            >GitHub 仓库</button
          >
        </div>
        <div class="row">
          <span>由</span>
          <button class="link" onclick={() => openUrl(DEEPSEEK_URL)}
            >DeepSeek</button
          >
          <span>制作 &amp; 设计</span>
          <button class="link" onclick={() => openUrl(REPO_URL)}
            >AuthorZiXi</button
          >
        </div>
      </div>

      <div class="actions">
        <button onclick={() => (showSettings = false)}>关闭</button>
      </div>
    </div>
  </div>
{/if}

{#if showExcludeEditor && config}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onclick={() => (showExcludeEditor = false)}
    transition:fade={{ duration: 120 }}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="dialog settings"
      onclick={(e) => e.stopPropagation()}
      transition:scale={{ start: 0.96, duration: 150, easing: cubicOut }}
    >
      <h3>排除规则</h3>
      <p class="hint">
        每行一个文件名。隐藏项默认不显示；风险项会显示，但默认不勾选。
      </p>

      <div class="field-col">
        <span>隐藏项</span>
        <textarea bind:value={hiddenText} rows={7}></textarea>
      </div>

      <div class="field-col">
        <span>风险项</span>
        <textarea bind:value={riskText} rows={4}></textarea>
      </div>

      <div class="actions">
        <button onclick={() => (showExcludeEditor = false)}>取消</button>
        <button onclick={saveExcludeRules}>保存</button>
      </div>
    </div>
  </div>
{/if}

{#if showSmartSelect}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onclick={() => (showSmartSelect = false)}
    transition:fade={{ duration: 120 }}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="dialog settings"
      onclick={(e) => e.stopPropagation()}
      transition:scale={{ start: 0.96, duration: 150, easing: cubicOut }}
    >
      <h3>智能选中</h3>

      <div class="field-col">
        <span>包含条件</span>
        <div class="smart-row">
          <div class="seg">
            <button
              class:active={smartIncludeMode === "regex"}
              onclick={() => (smartIncludeMode = "regex")}>正则</button
            >
            <button
              class:active={smartIncludeMode === "substring"}
              onclick={() => (smartIncludeMode = "substring")}>包含文本</button
            >
          </div>
          <input
            bind:value={smartInclude}
            placeholder={smartIncludeMode === "regex"
              ? "例如 \\d"
              : "例如 save"}
            class:invalid={!smartIncludeValid}
          />
        </div>
        {#if !smartIncludeValid}
          <p class="warn">正则表达式无效</p>
        {/if}
      </div>

      <div class="field-col">
        <span>排除关键词（每行一个，不区分大小写）</span>
        <textarea bind:value={smartExclude} rows={6}></textarea>
      </div>

      <div class="field">
        <span>只在可见项中选中</span>
        <input
          type="checkbox"
          checked={smartOnlyVisible}
          onchange={(e) =>
            (smartOnlyVisible = (e.target as HTMLInputElement).checked)}
        />
      </div>

      <p class="hint">
        将选中 {smartPreview.length} 个文件。
        {#if smartPreview.length > 0 && smartPreview.length <= 8}
          <br />
          {smartPreview.map((f) => f.name).join("、")}
        {/if}
      </p>

      <div class="actions">
        <button onclick={() => (showSmartSelect = false)}>取消</button>
        <button onclick={applySmartSelect} disabled={!smartIncludeValid}>
          应用
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- 备注编辑 -->
{#if noteEdit}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onclick={() => (noteEdit = null)}
    transition:fade={{ duration: 120 }}
  >
    <div
      class="dialog"
      onclick={(e) => e.stopPropagation()}
      transition:scale={{ start: 0.96, duration: 150, easing: cubicOut }}
    >
      <h3>编辑第 {noteEdit.number} 章备注</h3>
      <input
        bind:value={noteEdit.value}
        placeholder="留空则删除备注"
        onkeydown={(e) => e.key === "Enter" && saveNote()}
      />
      <div class="actions">
        <button onclick={() => (noteEdit = null)}>取消</button>
        <button onclick={saveNote}>保存</button>
      </div>
    </div>
  </div>
{/if}

<!-- Toast -->
{#if toast}
  <div class="toast" class:error={toast.kind === "error"}>{toast.text}</div>
{/if}

<style>
  .app {
    display: flex;
    height: 100vh;
  }

  .sidebar {
    width: 260px;
    min-width: 260px;
    margin-left: 0;
    border-right: 1px solid var(--border);
    background: var(--bg-elevated);
    display: flex;
    flex-direction: column;
    transition: margin-left 0.18s ease;
  }

  .sidebar-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    font-weight: 600;
    border-bottom: 1px solid var(--border);
  }

  .path-list {
    list-style: none;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
    flex: 1;
  }

  .path-list li {
    margin-bottom: 2px;
  }

  .path-list li.active .path-btn {
    background: var(--accent);
    color: #fff;
    border-color: transparent;
  }

  .path-list li.missing .path-btn {
    color: var(--danger);
    text-decoration: line-through;
  }

  .path-btn {
    display: block;
    width: 100%;
    text-align: left;
    border: 1px solid transparent;
    background: transparent;
    padding: 6px 10px;
    border-radius: 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: left;
  }

  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .file-pane,
  .chapter-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .file-pane {
    flex: 3;
    border-bottom: 1px solid var(--border);
  }
  .chapter-pane {
    flex: 2;
  }
  .pane-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .file-list,
  .chapter-list {
    list-style: none;
    margin: 0;
    padding: 4px;
    overflow-y: auto;
    flex: 1;
  }

  .file-list li,
  .chapter-list li {
    padding: 6px 10px;
    border-radius: 4px;
  }

  .file-list li:hover,
  .chapter-list li:hover {
    background: var(--bg-hover);
  }

  .file-list label {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }

  .file-list .size {
    margin-left: auto;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .chapter-list li {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .chapter-list .num {
    font-weight: 600;
  }
  .chapter-list .note {
    color: var(--text-muted);
  }
  .chapter-list .muted {
    color: var(--text-muted);
  }
  .chapter-list button {
    margin-left: auto;
  }

  .muted {
    color: var(--text-muted);
  }
  .dim {
    opacity: 0.55;
  }
  .risk {
    color: var(--risk);
  }
  .empty {
    color: var(--text-muted);
    padding: 8px 12px;
  }

  .missing-banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border);
    color: var(--danger);
  }

  .empty-center {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--text-muted);
  }

  .context-menu {
    position: fixed;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 1000;
    min-width: 180px;
  }

  .context-menu button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    border: none;
    background: transparent;
    border-radius: 4px;
  }

  .context-menu button:hover {
    background: var(--bg-hover);
  }
  .context-menu button.danger {
    color: var(--danger);
  }

  .context-menu .menu-sep {
    height: 1px;
    background: var(--border);
    margin: 4px 6px;
  }

  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .dialog {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
    min-width: 360px;
    max-width: 560px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }

  .dialog h3 {
    margin: 0 0 10px;
    font-size: 15px;
  }

  .dialog pre {
    margin: 0 0 14px;
    white-space: pre-wrap;
    word-break: break-all;
    font-family: inherit;
    font-size: 13px;
    max-height: 300px;
    overflow-y: auto;
  }

  .dialog input {
    width: 100%;
    background: var(--bg-elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 14px;
    padding: 7px 10px;
    margin-bottom: 14px;
  }

  .dialog .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .dialog button.danger {
    background: var(--danger);
    color: #fff;
    border-color: transparent;
  }

  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 16px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
    z-index: 3000;
  }

  .toast.error {
    color: var(--danger);
    border-color: var(--danger);
  }

  .sidebar.collapsed {
    margin-left: -260px;
  }

  .sidebar-handle {
    position: fixed;
    top: 50%;
    transform: translateY(-50%);
    z-index: 300;
    left: 260px; /* 侧栏右边缘 */
    width: 14px;
    height: 52px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    border-left: none;
    border-radius: 0 6px 6px 0;
    background: var(--bg-elevated);
    color: var(--text-muted);
    opacity: 0.5;
    font-size: 13px;
    line-height: 1;
    transition:
      left 0.15s ease,
      width 0.15s ease,
      opacity 0.15s ease;
  }

  .sidebar-handle:hover {
    width: 22px;
    opacity: 1;
  }

  .sidebar-handle.collapsed {
    left: 0;
  }

  .sidebar-head,
  .pane-head {
    height: 40px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
    font-size: 14px;
  }
  .sidebar-head .title {
    flex: 1;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    line-height: 1;
  }

  .dialog.settings {
    min-width: 380px;
  }
  .dialog.settings .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 0;
  }
  .dialog.settings .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }
  .dialog.settings .seg button {
    border: none;
    border-radius: 0;
    padding: 4px 10px;
    background: transparent;
  }
  .dialog.settings .seg button.active {
    background: var(--accent);
    color: #fff;
  }

  .dialog.settings .seg button + button {
    border-left: 1px solid var(--border);
  }
  .dialog.settings .seg button.active + button,
  .dialog.settings .seg button.active {
    border-left-color: transparent;
  }

  .dialog.settings .hint {
    color: var(--text-muted);
    font-size: 12px;
    margin: 8px 0 12px;
    line-height: 1.6;
  }

  .dialog input[type="checkbox"] {
    width: auto;
    margin: 0;
    padding: 0;
    flex: 0 0 auto;
  }

  .dialog.settings .field-col {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 10px 0;
  }
  .dialog.settings textarea {
    width: 100%;
    min-height: 100px;
    padding: 8px 10px;
    background: var(--bg-elevated);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-family: Consolas, "Courier New", monospace;
    font-size: 13px;
    resize: vertical;
  }

  .dialog.settings .smart-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .dialog.settings .smart-row input {
    flex: 1;
    margin-bottom: 0;
  }

  .dialog.settings input.invalid,
  .dialog.settings input.invalid:focus {
    border-color: var(--danger);
  }

  .dialog.settings .warn {
    color: var(--danger);
    font-size: 12px;
    margin: 4px 0 0;
  }

  .dialog.settings .about {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.9;
  }
  .dialog.settings .about .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dialog.settings .about .link {
    border: none;
    background: transparent;
    color: var(--accent);
    padding: 0;
    font: inherit;
    cursor: pointer;
  }
  .dialog.settings .about .link:hover {
    text-decoration: underline;
  }
</style>
