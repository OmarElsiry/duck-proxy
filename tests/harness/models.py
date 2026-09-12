from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any

@dataclass
class TestCaseResult:
    id: str
    name: str
    domain: str
    description: str
    status: str  # "PASS", "FAIL", "SKIP", "WARN"
    latency_seconds: float
    details: str = ""
    error: Optional[str] = None
    stdout: str = ""
    stderr: str = ""

@dataclass
class DomainResult:
    domain_id: int
    name: str
    tests: List[TestCaseResult] = field(default_factory=list)

    @property
    def total(self) -> int:
        return len(self.tests)

    @property
    def passed(self) -> int:
        return sum(1 for t in self.tests if t.status == "PASS")

    @property
    def failed(self) -> int:
        return sum(1 for t in self.tests if t.status == "FAIL")

    @property
    def avg_latency(self) -> float:
        if not self.tests:
            return 0.0
        return sum(t.latency_seconds for t in self.tests) / len(self.tests)

@dataclass
class TestSuiteResult:
    timestamp: str
    mode: str
    model: str
    domains: List[DomainResult] = field(default_factory=list)

    @property
    def total(self) -> int:
        return sum(d.total for d in self.domains)

    @property
    def passed(self) -> int:
        return sum(d.passed for d in self.domains)

    @property
    def failed(self) -> int:
        return sum(d.failed for d in self.domains)

    @property
    def pass_percentage(self) -> float:
        if self.total == 0:
            return 100.0
        return (self.passed / self.total) * 100.0
