import { ItemView, MarkdownRenderer, TFile, type ViewStateResult, WorkspaceLeaf } from "obsidian";
import type SeCallPlugin from "./main";

export const SESSION_VIEW_TYPE = "secall-session";

// relation → 표시 레이블
const RELATION_LABELS: Record<string, string> = {
  same_project: "같은 프로젝트",
  same_day: "같은 날",
  uses_tool: "같은 툴 사용",
  discusses_topic: "관련 토픽",
  fixes_bug: "관련 버그픽스",
  modifies_file: "같은 파일 수정",
};

// 관련 세션 패널에 표시할 relation 우선순위
const RELATION_PRIORITY = [
  "same_project",
  "same_day",
  "discusses_topic",
  "fixes_bug",
  "modifies_file",
];

interface GraphNode {
  node_id: string;
  relation: string;
  direction: string;
}

export class SessionView extends ItemView {
  plugin: SeCallPlugin;
  sessionId!: string;

  constructor(leaf: WorkspaceLeaf, plugin: SeCallPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType() { return SESSION_VIEW_TYPE; }
  getDisplayText() { return `Session: ${this.sessionId || "..."}`; }
  getIcon() { return "file-text"; }

  async setState(state: { sessionId: string }, result: ViewStateResult) {
    this.sessionId = state.sessionId;
    await this.render();
    await super.setState(state, result);
  }

  getState() {
    return { sessionId: this.sessionId };
  }

  async render() {
    const container = this.containerEl.children[1] as HTMLElement;
    container.empty();

    if (!this.sessionId) {
      container.createEl("div", { text: "No session selected." });
      return;
    }

    container.createEl("div", { text: "Loading...", cls: "secall-loading" });

    try {
      const data = await this.plugin.api.get(this.sessionId, true);
      container.empty();

      // ── Header ──────────────────────────────────────────────────────────────
      const header = container.createDiv({ cls: "secall-session-header" });
      header.createEl("h3", { text: data.summary || this.sessionId });
      header.createEl("div", {
        text: `${data.project || "?"} · ${data.agent} · ${data.date}`,
        cls: "secall-result-meta",
      });

      // ── Content ──────────────────────────────────────────────────────────────
      if (data.content) {
        const contentEl = container.createDiv({ cls: "secall-session-content" });
        await MarkdownRenderer.render(
          this.app,
          data.content,
          contentEl,
          "",
          this.plugin
        );
      }

      // ── Related sessions (graph) ─────────────────────────────────────────────
      this.renderRelated(container, this.sessionId);

    } catch (e) {
      container.empty();
      container.createEl("div", {
        text: `Error: ${e instanceof Error ? e.message : String(e)}`,
        cls: "secall-error",
      });
    }
  }

  private async renderRelated(container: HTMLElement, sessionId: string) {
    // full UUID로 node_id 구성 (session_id가 앞 8자리인 경우 DB 조회 필요)
    // API는 session: prefix + full id를 요구하므로 get() 응답의 session_id 사용
    const nodeId = `session:${sessionId}`;

    let graphData: { results: GraphNode[]; count: number };
    try {
      graphData = await this.plugin.api.graphQuery(nodeId, 1);
    } catch {
      return; // 그래프 실패는 조용히 무시
    }

    if (!graphData.results || graphData.results.length === 0) return;

    // uses_tool / by_agent 제외 — 너무 많고 노이즈
    const filtered = graphData.results.filter(
      (n) => !["uses_tool", "by_agent", "belongs_to"].includes(n.relation)
    );
    if (filtered.length === 0) return;

    // relation 기준으로 그룹핑
    const groups = new Map<string, GraphNode[]>();
    for (const node of filtered) {
      const list = groups.get(node.relation) ?? [];
      list.push(node);
      groups.set(node.relation, list);
    }

    // ── 패널 렌더링 ──────────────────────────────────────────────────────────
    const panel = container.createDiv({ cls: "secall-related" });
    panel.createEl("h4", { text: "관련 세션", cls: "secall-related-title" });

    const orderedRelations = [
      ...RELATION_PRIORITY.filter((r) => groups.has(r)),
      ...[...groups.keys()].filter((r) => !RELATION_PRIORITY.includes(r)),
    ];

    for (const relation of orderedRelations) {
      const nodes = groups.get(relation)!;
      const label = RELATION_LABELS[relation] ?? relation;

      const section = panel.createDiv({ cls: "secall-related-section" });
      section.createEl("div", { text: label, cls: "secall-related-label" });

      // 요약을 병렬로 가져옴
      const sessionNodes = nodes
        .slice(0, 5)
        .filter((n) => n.node_id.startsWith("session:"));

      const summaries = await Promise.allSettled(
        sessionNodes.map((n) =>
          this.plugin.api.get(n.node_id.replace(/^session:/, ""), false)
        )
      );

      for (let i = 0; i < sessionNodes.length; i++) {
        const node = sessionNodes[i];
        const rawId = node.node_id.replace(/^session:/, "");
        const shortId = rawId.slice(0, 8);
        const settled = summaries[i];
        const meta =
          settled.status === "fulfilled" ? settled.value : null;

        const item = section.createDiv({ cls: "secall-related-item" });
        const titleText = meta?.summary
          ? `[${shortId}] ${meta.summary}`
          : shortId;
        item.createEl("span", { text: titleText, cls: "secall-related-id" });
        if (meta?.date) {
          item.createEl("span", {
            text: ` · ${meta.date}`,
            cls: "secall-result-meta",
          });
        }
        item.addEventListener("click", async () => {
          await this.setState({ sessionId: rawId }, {} as ViewStateResult);
        });
      }

      if (nodes.length > 5) {
        section.createEl("div", {
          text: `+${nodes.length - 5}개 더`,
          cls: "secall-related-more",
        });
      }
    }
  }

  private async openVaultFile(vaultPath: string) {
    const adapter = this.app.vault.adapter as any;
    const vaultRoot: string = adapter.basePath || "";
    const relativePath =
      vaultRoot && vaultPath.startsWith(vaultRoot + "/")
        ? vaultPath.slice(vaultRoot.length + 1)
        : vaultPath;
    const file = this.app.vault.getAbstractFileByPath(relativePath);
    if (file instanceof TFile) {
      await this.app.workspace.getLeaf(false).openFile(file);
    }
  }
}
