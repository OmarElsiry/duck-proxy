#!/usr/bin/env python3
import sys
import argparse
import datetime
from pathlib import Path

# Add project root to sys.path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from tests.harness.models import TestSuiteResult
from tests.harness.reporter import ArtifactReporter
from tests.harness.test_cases.test_file_ops import run_domain_file_ops
from tests.harness.test_cases.test_surgical_edit import run_domain_surgical_edit
from tests.harness.test_cases.test_context_memory import run_domain_context_memory
from tests.harness.test_cases.test_tool_calling import run_domain_tool_calling
from tests.harness.test_cases.test_streaming import run_domain_streaming
from tests.harness.test_cases.test_model_routing import run_domain_model_routing
from tests.harness.test_cases.test_subagents import run_domain_subagents
from tests.harness.test_cases.test_edge_cases import run_domain_edge_cases

def main():
    parser = argparse.ArgumentParser(description="AI Agent IDE Comprehensive Test Suite Runner")
    parser.add_argument("--mode", choices=["mock", "live"], default="mock", help="Execution mode (mock or live)")
    parser.add_argument("--model", default="duckproxy/gpt-5.6-luna", help="Target model identifier")
    parser.add_argument("--domain", choices=[
        "all", "file_ops", "surgical_edit", "context_memory",
        "tool_calling", "streaming", "model_routing", "subagents", "edge_cases"
    ], default="all", help="Target specific test domain")
    parser.add_argument("--output", default="test_results.md", help="Path for markdown report artifact")
    args = parser.parse_args()

    timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    suite = TestSuiteResult(timestamp=timestamp, mode=args.mode, model=args.model)

    domain_runners = {
        "file_ops": run_domain_file_ops,
        "surgical_edit": run_domain_surgical_edit,
        "context_memory": run_domain_context_memory,
        "tool_calling": run_domain_tool_calling,
        "streaming": run_domain_streaming,
        "model_routing": run_domain_model_routing,
        "subagents": run_domain_subagents,
        "edge_cases": run_domain_edge_cases,
    }

    if args.domain == "all":
        for name, runner in domain_runners.items():
            suite.domains.append(runner(mode=args.mode, model=args.model))
    else:
        runner = domain_runners.get(args.domain)
        if runner:
            suite.domains.append(runner(mode=args.mode, model=args.model))

    reporter = ArtifactReporter(output_path=args.output)
    reporter.print_terminal_summary(suite)
    reporter.generate_markdown_report(suite)

    # Return non-zero exit code if any test failed
    sys.exit(0 if suite.failed == 0 else 1)

if __name__ == "__main__":
    main()
