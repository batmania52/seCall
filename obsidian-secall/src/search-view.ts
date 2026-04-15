import { ItemView, TFile, WorkspaceLeaf } from "obsidian";
import type SeCallPlugin from "./main";
import { SESSION_VIEW_TYPE } from "./session-view";

// ── Session recall types ─────────────────────────────────────────────────────

interface SearchResultMeta {
  agent: string;
  project?: string;
  date: string;
  vault_path?: string;
  summary?: string;
}

interface SearchResult {
  session_id: string;
  snippet?: string;
  metadata: SearchResultMeta;
}

// ── Wiki search types ────────────────────────────────────────────────────────

interface WikiResult {
  path: string;
  title: string;
  preview?: string;
  created?: string;
  updated?: string;
}

// ── View ─────────────────────────────────────────────────────────────────────

export const SEARCH_VIEW_TYPE = "secall-search";

type Tab = "sessions" | "wiki";

export class SearchView extends ItemView {
  plugin: SeCallPlugin;
  private activeTab: Tab = "sessions";
  private inputEl!: HTMLInputElement;
  private resultsEl!: HTMLElement;

  constructor(leaf: WorkspaceLeaf, plugin: SeCallPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType() { return SEARCH_VIEW_TYPE; }
  getDisplayText() { return "seCall Search"; }
  getIcon() { return "search"; }

  async onOpen() {
    const container = this.containerEl.children[1] as HTMLElement;
    container.empty();
    container.addClass("secall-container");

    // ── Tab bar ──────────────────────────────────────────────────────────────
    const tabBar = container.createDiv({ cls: "secall-tab-bar" });
    const sessionsTab = tabBar.createEl("button", {
      text: "Sessions",
      cls: "secall-tab secall-tab-active",
    });
    const wikiTab = tabBar.createEl("button", {
      text: "Wiki",
      cls: "secall-tab",
    });

    // ── Search bar ───────────────────────────────────────────────────────────
    const searchBar = container.createDiv({ cls: "secall-search-bar" });
    this.inputEl = searchBar.createEl("input", {
      type: "text",
      placeholder: "Search sessions...",
      cls: "secall-search-input",
    });
    this.inputEl.addEventListener("keydown", (e: KeyboardEvent) => {
      if (e.key === "Enter") this.doSearch(this.inputEl.value);
    });

    // ── Results ──────────────────────────────────────────────────────────────
    this.resultsEl = container.createDiv({ cls: "secall-results" });

    // ── Tab switch handlers ──────────────────────────────────────────────────
    sessionsTab.addEventListener("click", () => {
      this.activeTab = "sessions";
      sessionsTab.addClass("secall-tab-active");
      wikiTab.removeClass("secall-tab-active");
      this.inputEl.placeholder = "Search sessions...";
      this.resultsEl.empty();
    });

    wikiTab.addEventListener("click", () => {
      this.activeTab = "wiki";
      wikiTab.addClass("secall-tab-active");
      sessionsTab.removeClass("secall-tab-active");
      this.inputEl.placeholder = "Search wiki...";
      this.resultsEl.empty();
    });
  }

  // ── Search dispatcher ─────────────────────────────────────────────────────

  private async doSearch(query: string) {
    if (!query.trim()) return;
    if (this.activeTab === "sessions") await this.doSessionSearch(query);
    else await this.doWikiSearch(query);
  }

  // ── Session search ────────────────────────────────────────────────────────

  private async doSessionSearch(query: string) {
    this.resultsEl.empty();
    this.resultsEl.createEl("div", { text: "Searching...", cls: "secall-loading" });

    try {
      const data = await this.plugin.api.recall(query);
      this.resultsEl.empty();

      if (!data.results || data.results.length === 0) {
        this.resultsEl.createEl("div", { text: "No results found." });
        return;
      }

      for (const r of data.results as SearchResult[]) {
        const meta = r.metadata;
        const item = this.resultsEl.createDiv({ cls: "secall-result-item" });
        item.createEl("div", {
          text: meta.summary || r.session_id,
          cls: "secall-result-title",
        });
        item.createEl("div", {
          text: `${meta.project || "?"} · ${meta.agent} · ${meta.date}`,
          cls: "secall-result-meta",
        });
        if (r.snippet) {
          item.createEl("div", { text: r.snippet, cls: "secall-result-snippet" });
        }
        const graphBtn = item.createEl("button", {
          text: "Graph",
          cls: "secall-graph-btn",
        });
        graphBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          this.plugin.openGraphView(`session:${r.session_id}`);
        });
        item.addEventListener("click", () => this.openSession(r));
      }
    } catch (e) {
      this.showError(e);
    }
  }

  private async openSession(result: SearchResult) {
    const leaf = this.app.workspace.getLeaf(false);
    await leaf.setViewState({
      type: SESSION_VIEW_TYPE,
      state: { sessionId: result.session_id },
    });
    this.app.workspace.revealLeaf(leaf);
  }

  // ── Wiki search ───────────────────────────────────────────────────────────

  private async doWikiSearch(query: string) {
    this.resultsEl.empty();
    this.resultsEl.createEl("div", { text: "Searching...", cls: "secall-loading" });

    try {
      const data = await this.plugin.api.wikiSearch(query);
      this.resultsEl.empty();

      if (!data.results || data.results.length === 0) {
        this.resultsEl.createEl("div", { text: "No wiki pages found." });
        return;
      }

      for (const r of data.results as WikiResult[]) {
        const item = this.resultsEl.createDiv({ cls: "secall-result-item" });
        item.createEl("div", { text: r.title, cls: "secall-result-title" });
        item.createEl("div", {
          text: `${r.path}${r.updated ? " · " + r.updated : ""}`,
          cls: "secall-result-meta",
        });
        if (r.preview) {
          // preview는 frontmatter 포함 원문이므로 --- 이후 첫 줄만 표시
          const bodyPreview = r.preview
            .replace(/^---[\s\S]*?---\s*/m, "")
            .trim()
            .slice(0, 120);
          if (bodyPreview) {
            item.createEl("div", { text: bodyPreview, cls: "secall-result-snippet" });
          }
        }
        item.addEventListener("click", () => this.openWikiPage(r.path));
      }
    } catch (e) {
      this.showError(e);
    }
  }

  private async openWikiPage(relativePath: string) {
    const file = this.app.vault.getAbstractFileByPath(relativePath);
    if (file instanceof TFile) {
      await this.app.workspace.getLeaf(false).openFile(file);
    }
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  private showError(e: unknown) {
    this.resultsEl.empty();
    this.resultsEl.createEl("div", {
      text: `Error: ${e instanceof Error ? e.message : String(e)}`,
      cls: "secall-error",
    });
  }
}
