import json
from pathlib import Path
from typing import Optional
from .models import TestSuiteResult

class ArtifactReporter:
    """Generates formatted Markdown test report artifacts and colorized terminal outputs."""

    def __init__(self, output_path: str = "test_results.md"):
        self.output_path = Path(output_path)

    def print_terminal_summary(self, suite: TestSuiteResult):
        GREEN = "\033[92m"
        RED = "\033[91m"
        YELLOW = "\033[93m"
        CYAN = "\033[96m"
        BOLD = "\033[1m"
        RESET = "\033[0m"

        print(f"\n{BOLD}{CYAN}════════════════════════════════════════════════════════════════════════════════════{RESET}")
        print(f"{BOLD}{CYAN}                    🧪 AI AGENT IDE TEST SUITE SUMMARY                             {RESET}")
        print(f"{BOLD}{CYAN}════════════════════════════════════════════════════════════════════════════════════{RESET}")
        print(f"Timestamp:   {suite.timestamp}")
        print(f"Mode:        {suite.mode.upper()}")
        print(f"Model:       {suite.model}")
        status_color = GREEN if suite.failed == 0 else RED
        print(f"Result:      {status_color}{BOLD}{suite.passed}/{suite.total} PASSED ({suite.pass_percentage:.1f}%){RESET}\n")

        print(f"{BOLD}{'Domain':<38} | {'Total':<6} | {'Pass':<5} | {'Fail':<5} | {'Avg Latency':<12} | {'Status'}{RESET}")
        print("-" * 84)

        for d in suite.domains:
            d_status = f"{GREEN}PASS{RESET}" if d.failed == 0 else f"{RED}FAIL{RESET}"
            print(f"{d.name:<38} | {d.total:<6} | {d.passed:<5} | {d.failed:<5} | {d.avg_latency:.2f}s        | {d_status}")

        print("-" * 84)
        print(f"{BOLD}{'TOTAL':<38} | {suite.total:<6} | {suite.passed:<5} | {suite.failed:<5} |                  | {status_color}{suite.passed}/{suite.total} OK{RESET}")
        print(f"{BOLD}{CYAN}════════════════════════════════════════════════════════════════════════════════════{RESET}\n")

    def generate_markdown_report(self, suite: TestSuiteResult) -> str:
        status_badge = "✅ **ALL PASSED (100%)**" if suite.failed == 0 else f"❌ **{suite.failed} FAILED**"

        md = []
        md.append("# 🧪 AI Agent IDE Test Suite Report\n")
        md.append(f"**Execution Timestamp:** `{suite.timestamp}`  ")
        md.append(f"**Execution Mode:** `{suite.mode.upper()}`  ")
        md.append(f"**Target Model:** `{suite.model}`  ")
        md.append(f"**Overall Result:** {status_badge}  ")
        md.append(f"**Total Scenarios Tested:** `{suite.total}`  ")
        md.append(f"**Passed:** `{suite.passed}` | **Failed:** `{suite.failed}`  \n")
        md.append("---\n")
        md.append("## 📊 Domain Scoreboard\n")
        md.append("| Domain ID | Domain Name | Total Tests | Passed | Failed | Avg Latency | Status |")
        md.append("|:---:|---|:---:|:---:|:---:|:---:|:---:|")

        for d in suite.domains:
            status_icon = "✅ PASS" if d.failed == 0 else "❌ FAIL"
            md.append(f"| {d.domain_id} | {d.name} | {d.total} | {d.passed} | {d.failed} | {d.avg_latency:.2f}s | {status_icon} |")

        md.append(f"| **—** | **TOTAL** | **{suite.total}** | **{suite.passed}** | **{suite.failed}** | **—** | **{status_badge}** |\n")
        md.append("---\n")
        md.append("## 📋 Comprehensive 40-Scenario Verification Matrix\n")
        md.append("| Test ID | Scenario Name | Domain | Latency | Status | Verified Details |")
        md.append("|---|---|---|:---:|:---:|---|")

        for d in suite.domains:
            for t in d.tests:
                status_icon = "✅ PASS" if t.status == "PASS" else "❌ FAIL"
                escaped_details = t.details.replace("\n", "<br>").replace("|", "\\|")
                md.append(f"| `{t.id}` | **{t.name}** | {t.domain} | {t.latency_seconds:.2f}s | {status_icon} | {escaped_details} |")

        md.append("\n---\n")
        if suite.failed > 0:
            md.append("## ❌ Failure Diagnostics & Tracebacks\n")
            for d in suite.domains:
                for t in d.tests:
                    if t.status == "FAIL":
                        md.append(f"### `{t.id}`: {t.name}\n")
                        md.append(f"- **Domain:** {t.domain}\n")
                        md.append(f"- **Error:** {t.error or 'Unknown error'}\n")
                        if t.stderr:
                            md.append(f"```text\n{t.stderr}\n```\n")

        content = "\n".join(md)
        self.output_path.write_text(content, encoding="utf-8")
        return content
