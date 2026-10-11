#!/usr/bin/env python3
"""
Worst Day Ever — HTML Report Renderer

Reads assessment results JSON, outputs a standalone HTML report with:
- Executive summary (plain-English themes before technical specifics)
- Severity filter tabs (CRITICAL / HIGH / MEDIUM / LOW / PASS / ALL)
- Accordion finding cards (collapsed by default, expand on click)
- Agent-to-agent frontmatter (collapsible at bottom)

Usage:
    cat results.json | python render_report.py > report.html
    python render_report.py --input results.json --output report.html
    python render_report.py --input results.json --title "My Audit" --output report.html
"""

import argparse
import json
import sys
from datetime import datetime
from html import escape
from pathlib import Path

# ── Config loading ──

def load_config(config_path=None):
    """Load config from JSON file with fallback to hardcoded defaults.

    Auto-discovers wde_config.json alongside the script if no path given.
    """
    if config_path is None:
        config_path = Path(__file__).parent / "wde_config.json"

    path = Path(config_path)
    if path.exists():
        with open(path) as f:
            return json.load(f)

    # Fallback: minimal inline defaults so script works standalone
    return {
        "severity_order": ["CRITICAL", "HIGH", "MEDIUM", "LOW", "PASS"],
        "severity_colors": {
            "CRITICAL": ["#dc2626", "#fef2f2", "#991b1b"],
            "HIGH": ["#ea580c", "#fff7ed", "#9a3412"],
            "MEDIUM": ["#ca8a04", "#fefce8", "#854d0e"],
            "LOW": ["#2563eb", "#eff6ff", "#1e40af"],
            "PASS": ["#16a34a", "#f0fdf4", "#166534"],
        },
        "severity_labels": {
            "CRITICAL": "Ship-blocking", "HIGH": "Must fix",
            "MEDIUM": "Should fix", "LOW": "Nice to have", "PASS": "All clear",
        },
        "dimension_names": {},
        "themes": [],
        "glossary": {},
    }


# Load config at module level — overridable via --config flag
CONFIG = load_config()
SEVERITY_COLORS = CONFIG["severity_colors"]
SEVERITY_ORDER = CONFIG["severity_order"]
SEVERITY_LABELS = CONFIG["severity_labels"]
DIMENSION_NAMES = CONFIG["dimension_names"]
THEMES = CONFIG["themes"]
GLOSSARY = CONFIG["glossary"]


def reload_config(config_path):
    """Reload config from a given path and update all module-level vars.

    Call this in main() before render_html() when --config is provided.
    """
    global CONFIG, SEVERITY_COLORS, SEVERITY_ORDER, SEVERITY_LABELS
    global DIMENSION_NAMES, THEMES, GLOSSARY
    CONFIG = load_config(config_path)
    SEVERITY_COLORS = CONFIG["severity_colors"]
    SEVERITY_ORDER = CONFIG["severity_order"]
    SEVERITY_LABELS = CONFIG["severity_labels"]
    DIMENSION_NAMES = CONFIG["dimension_names"]
    THEMES = CONFIG["themes"]
    GLOSSARY = CONFIG["glossary"]


FRONTMATTER_TEMPLATE = """---
handoff:
  project: {project}
  assessment_type: worst-day-ever
  date: {date}
  mode: {mode}
  total_scenarios: {total}
  severity_counts:
{wrapped_severity}
  theme_summary:
{wrapped_themes}
  critical_findings:
{wrapped_critical}
  next_recommendation: |
{recommendation}
---"""


def theme_for_finding(finding):
    """Assign a finding to a theme. Prefers explicit theme field, falls back to keyword match on title."""
    if "theme" in finding:
        return finding["theme"]
    t = finding.get("title", "").lower()
    for theme in THEMES:
        if any(kw in t for kw in theme.get("match_keywords", [])):
            return theme["id"]
    return None


def generate_eli5(finding):
    """Auto-generate ELI5 explanation from finding data + glossary.

    Produces 4 paragraphs: What you could do, What the system does, The reason, The fix.
    Everything derived from the finding's own fields — no hand-authored content.
    """
    title = finding.get("title", "")
    observed = finding.get("observed", "")
    expected = finding.get("expected", "")
    remediation = finding.get("remediation", "")
    flow = finding.get("flow", "")
    severity = finding.get("severity", "")

    # P1: What you could do — derive from flow field
    if flow:
        cmd_part = flow.split("\u2192")[0].strip() if "\u2192" in flow else flow
        if len(cmd_part) > 80:
            cmd_part = cmd_part[:77] + "..."
        what_you_could_do = f"**What you could do**: `{cmd_part}`"
    else:
        triggers = {
            "CRITICAL": "Send an unexpected or malformed input to the system",
            "HIGH": "Exercise an edge case in normal operation",
            "MEDIUM": "Operate the system in a slightly unusual way",
            "LOW": "Encounter a fringe scenario during use",
        }
        fallback = triggers.get(severity, "Interact with the system")
        what_you_could_do = f"**What you could do**: {fallback}"

    # P2: What the system does — observed with severity framing
    if observed:
        what_system_does = f"**What the system does**: {observed}"
    else:
        what_system_does = "**What the system does**: The system behaves incorrectly."

    # P3: The reason — scan title + observed for glossary terms
    search_text = f"{title} {observed}".lower()
    matched_terms = []
    for term, definition in GLOSSARY.items():
        if term.lower() in search_text:
            matched_terms.append(f"**{term}**: {definition}")

    if matched_terms:
        the_reason = "**The reason**: " + " ".join(matched_terms)
    elif expected:
        the_reason = f"**The reason**: The system does what it was told, but what it was told is wrong. {expected}"
    else:
        the_reason = "**The reason**: The system's behavior violates correctness expectations."

    # P4: The fix — remediation with lead-in
    if remediation:
        the_fix = f"**The fix**: {remediation}"
    else:
        the_fix = "**The fix**: Install a guard at this boundary to catch the invalid state."

    return (what_you_could_do, what_system_does, the_reason, the_fix)


def build_frontmatter(results):
    """Generate agent-to-agent frontmatter from results."""
    summary = results.get("summary", {})
    findings = results.get("findings", [])

    sev_counts = "\n".join(
        f"    {sev}: {summary.get(sev, 0)}"
        for sev in SEVERITY_ORDER
        if summary.get(sev, 0) > 0
    )

    # Theme counts
    theme_counts = {}
    for f in findings:
        tid = theme_for_finding(f)
        if tid:
            theme_counts[tid] = theme_counts.get(tid, 0) + 1
    theme_lines = "\n".join(
        f"    {t['id']}: {theme_counts.get(t['id'], 0)}"
        for t in THEMES
        if theme_counts.get(t['id'], 0) > 0
    )

    criticals = [f for f in findings if f["severity"] == "CRITICAL"]
    crit_lines = [f"- {c['title']} (Dim {c['dimension']}): {c['remediation']}" for c in criticals]
    crit_block = "\n".join("    " + l for l in crit_lines) if crit_lines else "    (none)"

    recommendation = (
        "Fix CRITICAL items (NaN/Inf guards) before any other work. "
        "Then address HIGH items: idempotent replay detection, gate hysteresis, "
        "force-assignment authZ, and remove-agent command."
    )

    return FRONTMATTER_TEMPLATE.format(
        project=escape(results.get("project", "Unknown")),
        date=datetime.now().strftime("%Y-%m-%d"),
        mode=escape(results.get("mode", "unknown")),
        total=results.get("total_scenarios", 0),
        wrapped_severity=sev_counts,
        wrapped_themes=theme_lines,
        wrapped_critical=crit_block,
        recommendation=recommendation,
    )


def render_html(results):
    """Render full HTML report from assessment results."""
    findings = results.get("findings", [])
    summary = results.get("summary", {})
    frontmatter = build_frontmatter(results)

    total = results.get("total_scenarios", 0)
    tested_dims = results.get("dimensions_tested", [])
    skipped_dims = results.get("dimensions_not_tested", [])

    # Group findings by severity
    grouped = {s: [] for s in SEVERITY_ORDER}
    for f in findings:
        sev = f.get("severity", "PASS")
        if sev in grouped:
            grouped[sev].append(f)

    # ── Severity stat cards ──
    stat_cards = ""
    for sev in SEVERITY_ORDER:
        count = summary.get(sev, 0)
        if count == 0:
            continue
        color, bg, text = SEVERITY_COLORS[sev]
        label = SEVERITY_LABELS.get(sev, "")
        stat_cards += f"""
        <div class="stat" style="background:{bg}; border-left:4px solid {color};">
            <span class="stat-count" style="color:{color};">{count}</span>
            <span class="stat-label" style="color:{text};">{sev}</span>
            <span class="stat-context">{label}</span>
        </div>"""

    # ── Sticky severity bar ──
    total_non_pass = sum(summary.get(s, 0) for s in SEVERITY_ORDER if s != "PASS")
    sticky_sevs = ""
    for sev in SEVERITY_ORDER:
        count = summary.get(sev, 0)
        if count == 0:
            continue
        color, _, _ = SEVERITY_COLORS[sev]
        sticky_sevs += f'<span class="sticky-sev" style="background:{color}22; color:{color};"><span class="sticky-count">{count}</span> {sev}</span>'
    sticky_bar = f"""
    <div class="sticky-bar">
        <span class="sticky-label">{total_non_pass} findings</span>
        {sticky_sevs}
    </div>""" if total_non_pass > 0 else ""

    # ── Dimension tags ──
    dim_tags = "".join(
        f'<span class="dim-tag">{DIMENSION_NAMES.get(d, f"Dim {d}")}</span>'
        for d in tested_dims
    )

    # ── Executive summary: themes (collapsible hero cards) ──
    def finding_id(title):
        tid = title.lower().replace(' ', '-').replace('/', '-').replace('"', '').replace("'", '').replace(',', '').replace('.', '')
        return f"f-{tid[:50]}"

    theme_assignments = {}
    for f in findings:
        if f["severity"] == "PASS":
            continue
        tid = theme_for_finding(f)
        if tid:
            theme_assignments.setdefault(tid, []).append(f)

    theme_cards = ""
    for theme in THEMES:
        tid = theme["id"]
        members = theme_assignments.get(tid, [])
        if not members:
            continue
        worst_sev = max(
            (m["severity"] for m in members),
            key=lambda s: SEVERITY_ORDER.index(s) if s in SEVERITY_ORDER else 99
        )
        wc, wbg, wtxt = SEVERITY_COLORS.get(worst_sev, ("#64748b", "#f8fafc", "#475569"))
        member_items = "".join(
            f"""<a class="theme-finding-item" href="#{finding_id(m['title'])}">
                    <span class="theme-finding-sev" style="background:{SEVERITY_COLORS.get(m['severity'], ('#94a3b8',))[0]};">{m['severity']}</span>
                    <span>{escape(m['title'])}</span>
                  </a>"""
            for m in members
        )
        theme_cards += f"""
        <details class="theme-hero" style="border-left: 4px solid {wc};" id="theme-{tid}">
            <summary class="theme-hero-header">
                <span class="theme-hero-icon">{theme['icon']}</span>
                <div class="theme-hero-body">
                    <div class="theme-hero-title">
                        {theme['name']}
                        <span class="theme-hero-worst" style="background:{wc};">worst: {worst_sev}</span>
                    </div>
                    <div class="theme-hero-summary">{theme['description']}</div>
                </div>
                <span class="theme-hero-meta">{len(members)} finding{'s' if len(members) != 1 else ''}</span>
                <span class="theme-hero-expand">&#9660;</span>
            </summary>
            <div class="theme-findings">{member_items}</div>
        </details>"""

    exec_summary_section = ""
    if theme_cards:
        exec_summary_section = f"""
        <section id="exec-summary" class="exec-summary anchor">
            <h2 class="section-title">💡 In Plain English</h2>
            <p class="section-subtitle">The big-picture problems — each theme groups related findings. Click a theme to see the specific findings, then click any finding to jump to its details.</p>
            <div class="theme-grid">{theme_cards}</div>
        </section>"""

    # ── Tab bar ──
    all_count = len(findings)
    tab_labels = ""
    tab_defs = ""
    checked = "checked"
    for sev_label, sev_key in [("ALL", "all"), ("CRITICAL", "critical"), ("HIGH", "high"),
                                ("MEDIUM", "medium"), ("LOW", "low"), ("PASS", "pass")]:
        count = summary.get(sev_label, 0) if sev_label != "ALL" else all_count
        tab_id = f"st-{sev_key}"
        tab_defs += f'<input type="radio" name="sev-tab" id="{tab_id}" {checked}>\n'
        color, _, _ = SEVERITY_COLORS.get(sev_label, ("#64748b", "", ""))
        tab_labels += f'<label for="{tab_id}" class="tab-label" data-color="{color}">{sev_label} <span class="tab-count">{count}</span></label>\n'
        checked = ""

    # ── Finding cards (accordion, grouped by dimension) ──
    all_cards = ""
    for sev in SEVERITY_ORDER:
        items = grouped.get(sev, [])
        if not items:
            continue
        color, _, _ = SEVERITY_COLORS[sev]
        sev_label = sev.lower()

        # Group items by dimension
        dim_groups = {}
        for f in items:
            dim = f.get("dimension", "?")
            dim_groups.setdefault(dim, []).append(f)

        for dim in sorted(dim_groups.keys()):
            dim_name = DIMENSION_NAMES.get(dim, f"Dimension {dim}")
            dim_items = dim_groups[dim]
            all_cards += f'<div class="dim-subhead">Dim {dim}: {dim_name} ({len(dim_items)} finding{"s" if len(dim_items) != 1 else ""})</div>'

            for f in dim_items:
                fid = finding_id(f["title"])
                eli5_html = ""
                if f["severity"] != "PASS":
                    eli5_parts = generate_eli5(f)
                    paras = "".join(f"<p>{p}</p>" for p in eli5_parts)
                    eli5_html = f'<div class="eli5">{paras}</div>'

                all_cards += f"""
                <details class="finding-card finding-{sev_label}" id="{fid}">
                    <summary class="card-summary">
                        <div class="card-summary-left">
                            <span class="severity-badge" style="background:{color}; color:#fff;">{sev}</span>
                            <span class="card-title-text">{escape(f['title'])}</span>
                        </div>
                        <div class="card-summary-right">
                            <span class="expand-icon">▼</span>
                        </div>
                    </summary>
                    <div class="card-body">
                        <div class="card-meta">
                            <span class="dimension-badge">Dim {dim}: {dim_name}</span>
                            <span class="flow"><strong>Flow:</strong> <code>{escape(f.get('flow', ''))}</code></span>
                        </div>
                        {eli5_html}
                        <div class="card-details">
                            <div class="detail-block observed">
                                <h4>👀 What happened</h4>
                                <p>{escape(f.get('observed', ''))}</p>
                            </div>
                            <div class="detail-block expected">
                                <h4>✅ What should have happened</h4>
                                <p>{escape(f.get('expected', ''))}</p>
                            </div>
                            <div class="detail-block remediation">
                                <h4>🔧 The boundary note</h4>
                                <p>{escape(f.get('remediation', ''))}</p>
                            </div>
                        </div>
                    </div>
                </details>"""

    if not all_cards:
        all_cards = '<p style="color:#94a3b8; text-align:center; padding:2rem;">No findings to display.</p>'

    # ── Assemble HTML ──
    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Worst Day Ever \u2014 {escape(results.get('project', 'Assessment'))}</title>
<style>
  *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    background: #f8fafc; color: #1e293b; line-height: 1.6;
  }}
  .container {{ max-width: 960px; margin: 0 auto; padding: 2rem 1.5rem; }}

  /* ── Header ── */
  header {{ margin-bottom: 1.5rem; }}
  header h1 {{ font-size: 1.75rem; font-weight: 700; color: #0f172a; }}
  header .subtitle {{ color: #64748b; font-size: 0.95rem; margin-top: 0.25rem; }}
  .meta-row {{
    display: flex; flex-wrap: wrap; gap: 0.5rem 1rem; margin-top: 0.75rem;
    font-size: 0.85rem; color: #475569;
  }}
  .meta-row span {{ background: #f1f5f9; padding: 0.25rem 0.75rem; border-radius: 999px; }}

  /* ── Stats bar ── */
  .stats {{ display: flex; gap: 0.75rem; flex-wrap: wrap; margin-bottom: 1.5rem; }}
  .stat {{
    flex: 1; min-width: 110px; padding: 0.85rem 1rem; border-radius: 10px;
    display: flex; flex-direction: column; align-items: center;
  }}
  .stat-count {{ font-size: 1.75rem; font-weight: 800; line-height: 1.1; }}
  .stat-label {{ font-size: 0.7rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.06em; }}
  .stat-context {{ font-size: 0.65rem; color: #64748b; margin-top: 0.15rem; }}

  /* ── Dims ── */
  .dims {{ margin-bottom: 1.5rem; }}
  .dims h2 {{ font-size: 0.85rem; font-weight: 600; color: #64748b; margin-bottom: 0.4rem; }}
  .dim-tag {{
    display: inline-block; background: #e2e8f0; color: #334155;
    padding: 0.2rem 0.6rem; border-radius: 6px; font-size: 0.75rem; font-weight: 500;
    margin: 0.15rem;
  }}
  .skipped {{ margin-bottom: 1.5rem; background: #fef9c3; border: 1px solid #facc15; border-radius: 8px; padding: 0.6rem 1rem; font-size: 0.8rem; }}

  /* ── Section titles ── */
  .section-title {{ font-size: 1.15rem; font-weight: 700; color: #0f172a; margin-bottom: 0.25rem; }}
  .section-subtitle {{ font-size: 0.85rem; color: #64748b; margin-bottom: 1rem; }}

  /* ── Executive summary / theme cards ── */
  .exec-summary {{ margin-bottom: 2rem; }}
  .theme-grid {{ display: grid; gap: 0.75rem; }}
  .theme-card {{
    background: #fff; border-radius: 10px; padding: 1rem 1.25rem;
    box-shadow: 0 1px 3px rgba(0,0,0,0.05);
  }}
  .theme-head {{ display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.5rem; }}
  .theme-icon {{ font-size: 1.5rem; }}
  .theme-head-text h3 {{ font-size: 0.95rem; font-weight: 600; color: #0f172a; }}
  .theme-count {{ font-size: 0.75rem; color: #64748b; }}
  .theme-desc {{ font-size: 0.85rem; color: #475569; margin-bottom: 0.5rem; line-height: 1.5; }}
  .theme-list {{ list-style: none; padding: 0; }}
  .theme-list li {{
    font-size: 0.8rem; color: #334155; padding: 0.2rem 0;
    border-bottom: 1px solid #f1f5f9;
  }}
  .theme-list li:last-child {{ border: none; }}
  .theme-find-sev {{ font-weight: 600; font-size: 0.75rem; }}

  /* ── Tab bar ── */
  input[name="sev-tab"] {{ display: none; }}
  .tab-bar {{
    display: flex; gap: 0.4rem; flex-wrap: wrap; margin-bottom: 1rem;
    border-bottom: 2px solid #e2e8f0; padding-bottom: 0;
  }}
  .tab-label {{
    display: inline-block; padding: 0.5rem 0.9rem; font-size: 0.8rem; font-weight: 600;
    color: #64748b; cursor: pointer; border-radius: 6px 6px 0 0;
    transition: all 0.15s; border-bottom: 2px solid transparent; margin-bottom: -2px;
  }}
  .tab-label:hover {{ background: #f1f5f9; color: #334155; }}
  .tab-count {{
    display: inline-block; background: #e2e8f0; color: #475569;
    padding: 0.05rem 0.45rem; border-radius: 999px; font-size: 0.65rem; font-weight: 600;
    margin-left: 0.25rem;
  }}
  #st-all:checked ~ .tab-bar label[for="st-all"],
  #st-critical:checked ~ .tab-bar label[for="st-critical"],
  #st-high:checked ~ .tab-bar label[for="st-high"],
  #st-medium:checked ~ .tab-bar label[for="st-medium"],
  #st-low:checked ~ .tab-bar label[for="st-low"],
  #st-pass:checked ~ .tab-bar label[for="st-pass"] {{
    color: #0f172a; border-bottom-color: #0f172a; background: #fff;
  }}
  #st-all:checked ~ .tab-bar label[for="st-all"] .tab-count,
  #st-critical:checked ~ .tab-bar label[for="st-critical"] .tab-count,
  #st-high:checked ~ .tab-bar label[for="st-high"] .tab-count,
  #st-medium:checked ~ .tab-bar label[for="st-medium"] .tab-count,
  #st-low:checked ~ .tab-bar label[for="st-low"] .tab-count,
  #st-pass:checked ~ .tab-bar label[for="st-pass"] .tab-count {{
    background: #e2e8f0;
  }}

  /* ── Filtered findings grid ── */
  .findings-grid .finding-card {{ display: none; }}
  #st-all:checked ~ .findings-section .findings-grid .finding-card {{ display: block; }}
  #st-critical:checked ~ .findings-section .findings-grid .finding-critical {{ display: block; }}
  #st-high:checked ~ .findings-section .findings-grid .finding-high {{ display: block; }}
  #st-medium:checked ~ .findings-section .findings-grid .finding-medium {{ display: block; }}
  #st-low:checked ~ .findings-section .findings-grid .finding-low {{ display: block; }}
  #st-pass:checked ~ .findings-section .findings-grid .finding-pass {{ display: block; }}

  /* ── Accordion finding card ── */
  .finding-card {{
    background: #fff; border-radius: 10px; margin-bottom: 0.6rem;
    box-shadow: 0 1px 2px rgba(0,0,0,0.05);
    border-left: 4px solid #cbd5e1; overflow: hidden;
  }}
  .finding-critical {{ border-left-color: #dc2626; }}
  .finding-high {{ border-left-color: #ea580c; }}
  .finding-medium {{ border-left-color: #ca8a04; }}
  .finding-low {{ border-left-color: #2563eb; }}
  .finding-pass {{ border-left-color: #16a34a; }}

  .card-summary {{
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.7rem 1rem; cursor: pointer; list-style: none; gap: 0.5rem;
  }}
  .card-summary::-webkit-details-marker {{ display: none; }}
  .card-summary:hover {{ background: #fafafa; }}
  .card-summary-left {{
    display: flex; align-items: center; gap: 0.5rem; flex: 1; min-width: 0;
  }}
  .card-summary-right {{
    display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0;
  }}
  .card-title-text {{
    font-size: 0.9rem; font-weight: 500; color: #0f172a;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }}
  .expand-icon {{ font-size: 0.65rem; color: #94a3b8; transition: transform 0.2s; }}
  details[open] .expand-icon {{ transform: rotate(180deg); }}

  .severity-badge {{
    display: inline-block; padding: 0.12rem 0.5rem; border-radius: 4px;
    font-size: 0.65rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em;
    flex-shrink: 0;
  }}
  .dimension-badge {{
    background: #f1f5f9; color: #475569; padding: 0.12rem 0.5rem; border-radius: 4px;
    font-size: 0.7rem; font-weight: 500; flex-shrink: 0;
  }}

  .card-body {{ padding: 0 1rem 1rem 1rem; }}
  .card-meta {{ font-size: 0.8rem; color: #64748b; margin-bottom: 0.75rem; }}
  .card-meta code {{ background: #f1f5f9; padding: 0.1rem 0.4rem; border-radius: 3px; font-size: 0.75rem; }}

  .eli5 {{
    background: #f0f9ff; border: 1px solid #bae6fd; border-radius: 8px;
    padding: 0.85rem 1rem; margin-bottom: 0.85rem; font-size: 0.85rem;
  }}
  .eli5 p {{ margin-bottom: 0.4rem; }}
  .eli5 p:last-child {{ margin-bottom: 0; }}
  .eli5 strong {{ color: #0369a1; }}

  .card-details {{ display: grid; grid-template-columns: 1fr; gap: 0.6rem; }}
  @media (min-width: 640px) {{ .card-details {{ grid-template-columns: 1fr 1fr; }} }}
  .detail-block {{
    background: #f8fafc; border-radius: 6px; padding: 0.65rem 0.75rem; font-size: 0.82rem;
  }}
  .detail-block.remediation {{ grid-column: 1 / -1; background: #fffbeb; border: 1px solid #fde68a; }}
  .detail-block h4 {{ font-size: 0.75rem; font-weight: 600; margin-bottom: 0.25rem; color: #475569; }}
  .detail-block p {{ color: #334155; }}

  /* ── Frontmatter ── */
  details.frontmatter-wrap {{
    margin-top: 2rem; background: #1e293b; border-radius: 8px; overflow: hidden;
  }}
  details.frontmatter-wrap summary {{
    padding: 0.6rem 1rem; cursor: pointer; color: #94a3b8;
    font-size: 0.75rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em;
    list-style: none;
  }}
  details.frontmatter-wrap summary::-webkit-details-marker {{ display: none; }}
  details.frontmatter-wrap summary:hover {{ background: #334155; }}
  .frontmatter-pre {{
    padding: 0 1rem 1rem 1rem; color: #e2e8f0;
    font-family: 'SF Mono', 'Fira Code', monospace; font-size: 0.75rem;
    line-height: 1.5; overflow-x: auto; white-space: pre-wrap;
  }}

  /* ── Sticky severity bar ── */
  .sticky-bar {{
    position: sticky; top: 0; z-index: 100;
    background: rgba(255,255,255,0.95); backdrop-filter: blur(8px);
    border-bottom: 1px solid #e2e8f0; padding: 0.5rem 1rem;
    margin: 0 -1rem 1rem -1rem; display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap;
  }}
  @media (min-width: 640px) {{ .sticky-bar {{ margin: 0 -1.5rem 1rem -1.5rem; padding: 0.5rem 1.5rem; }} .container {{ padding: 2rem 1.5rem; }} }}
  .sticky-bar .sticky-sev {{
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-size: 0.7rem; font-weight: 700; text-transform: uppercase;
    padding: 0.15rem 0.5rem; border-radius: 4px;
  }}
  .sticky-bar .sticky-count {{ font-size: 0.9rem; font-weight: 800; }}
  .sticky-bar .sticky-label {{ color: #64748b; font-size: 0.7rem; font-weight: 500; margin-right: 0.5rem; }}

  /* ── Nav anchors ── */
  .nav-bar {{
    display: flex; gap: 0.4rem; flex-wrap: wrap; margin-bottom: 1.25rem;
  }}
  .nav-link {{
    display: inline-block; font-size: 0.75rem; font-weight: 600; color: #475569;
    background: #f1f5f9; padding: 0.3rem 0.75rem; border-radius: 6px;
    text-decoration: none; transition: all 0.15s;
  }}
  .nav-link:hover {{ background: #e2e8f0; color: #0f172a; }}
  .anchor {{ scroll-margin-top: 3.5rem; }}

  /* ── Theme card hero redesign ── */
  .theme-hero {{
    background: #fff; border-radius: 12px; margin-bottom: 0.75rem;
    box-shadow: 0 1px 3px rgba(0,0,0,0.06); overflow: hidden;
  }}
  .theme-hero-header {{
    display: flex; align-items: center; gap: 0.75rem; padding: 1rem 1.25rem;
    cursor: pointer; user-select: none;
  }}
  .theme-hero-header:hover {{ background: #fafafa; }}
  .theme-hero-icon {{ font-size: 1.75rem; flex-shrink: 0; }}
  .theme-hero-body {{ flex: 1; min-width: 0; }}
  .theme-hero-title {{
    font-size: 1rem; font-weight: 700; color: #0f172a; display: flex; align-items: center; gap: 0.5rem;
  }}
  .theme-hero-worst {{
    font-size: 0.6rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em;
    padding: 0.1rem 0.4rem; border-radius: 3px; color: #fff;
  }}
  .theme-hero-summary {{
    font-size: 0.82rem; color: #475569; margin-top: 0.15rem; line-height: 1.5;
  }}
  .theme-hero-meta {{ font-size: 0.72rem; color: #94a3b8; flex-shrink: 0; }}
  .theme-hero-expand {{ font-size: 0.6rem; color: #94a3b8; transition: transform 0.2s; flex-shrink: 0; }}
  .theme-hero[open] .theme-hero-expand {{ transform: rotate(180deg); }}

  .theme-findings {{
    border-top: 1px solid #f1f5f9; padding: 0.5rem 1.25rem 0.75rem 1.25rem;
  }}
  .theme-finding-item {{
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.35rem 0; font-size: 0.82rem; color: #334155;
    border-bottom: 1px solid #f8fafc; text-decoration: none;
  }}
  .theme-finding-item:last-child {{ border: none; }}
  .theme-finding-item:hover {{ color: #0f172a; }}
  .theme-finding-sev {{
    font-size: 0.62rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.03em;
    padding: 0.1rem 0.35rem; border-radius: 3px; color: #fff; flex-shrink: 0;
  }}

  /* ── Dimension subheadings in findings grid ── */
  .dim-subhead {{
    font-size: 0.8rem; font-weight: 600; color: #64748b;
    padding: 0.75rem 0 0.25rem 0; margin-top: 0.5rem;
    border-bottom: 1px solid #e2e8f0;
  }}
  .dim-subhead:first-child {{ margin-top: 0; }}

  footer {{ text-align: center; color: #94a3b8; font-size: 0.7rem; padding: 2rem 0 0.5rem; }}
</style>
</head>
<body>
<div class="container">

  <header>
    <h1>⚠️ Worst Day Ever — {escape(results.get('project', 'Assessment'))}</h1>
    <p class="subtitle">Adversarial assessment of correctness, security, and resilience</p>
    <div class="meta-row">
      <span>📅 {datetime.now().strftime('%Y-%m-%d %H:%M')}</span>
      <span>🎯 {total} scenarios</span>
      <span>🔌 {results.get('mode', 'unknown')} mode</span>
    </div>
  </header>

  {sticky_bar}

  <div class="nav-bar">
    <a class="nav-link" href="#exec-summary">💡 Plain English</a>
    <a class="nav-link" href="#findings">🔬 All Findings</a>
    {' '.join(f'<a class="nav-link" href="#theme-{t["id"]}">{t["icon"]} {t["name"]}</a>' for t in THEMES if theme_assignments.get(t['id'], []))}
  </div>

  {tab_defs}

  {exec_summary_section}

  <div class="stats">
    {stat_cards}
  </div>

  <div class="dims">
    <h2>Dimensions tested</h2>
    {dim_tags}
    {f"<br><span class='skipped' style='display:inline-block;margin-top:0.5rem;'>⏭️ Skipped: {', '.join('Dim {} ({})'.format(d, DIMENSION_NAMES.get(d, '')) for d in skipped_dims)} — out of scope</span>" if skipped_dims else ""}
  </div>

  <div class="tab-bar">
    {tab_labels}
  </div>

  <section id="findings" class="findings-section anchor">
    <div class="findings-grid">
      {all_cards}
    </div>
  </section>

  <details class="frontmatter-wrap">
    <summary>\U0001f916 Agent handoff frontmatter (click to expand)</summary>
    <div class="frontmatter-pre">{escape(frontmatter)}</div>
  </details>

  <footer>
    Generated by Worst Day Ever skill &mdash; {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}
  </footer>

</div>
</body>
</html>"""
    return html


def main():
    parser = argparse.ArgumentParser(description="Worst Day Ever HTML Report Renderer")
    parser.add_argument("--input", "-i", help="Path to results JSON (default: stdin)")
    parser.add_argument("--output", "-o", help="Output HTML file (default: stdout)")
    parser.add_argument("--title", "-t", help="Override project title in report")
    parser.add_argument("--config", "-c", help="Path to wde_config.json (default: auto-discover)")
    args = parser.parse_args()

    if args.config:
        reload_config(args.config)

    if args.input:
        with open(args.input) as f:
            results = json.load(f)
    else:
        results = json.load(sys.stdin)

    if args.title:
        results["project"] = args.title

    html = render_html(results)

    if args.output:
        with open(args.output, "w") as f:
            f.write(html)
        print(f"Report written to {args.output}", file=sys.stderr)
    else:
        print(html)

    fm = build_frontmatter(results)
    print("--- FRONTMATTER ---", file=sys.stderr)
    print(fm, file=sys.stderr)


if __name__ == "__main__":
    main()
