from typing import List, Dict, Any, Optional

class TaskQueue:
    def __init__(self, max_retries: int = 3):
        self.tasks: List[Dict[str, Any]] = []
        self.dlq: List[Dict[str, Any]] = []
        self.max_retries = max_retries

    def push(self, task: Dict[str, Any]) -> None:
        self.tasks.append(task)

    def pop(self) -> Optional[Dict[str, Any]]:
        if not self.tasks:
            return None
        return self.tasks.pop(0)

    def send_to_dlq(self, task: Dict[str, Any]) -> None:
        self.dlq.append(task)

    def size(self) -> int:
        return len(self.tasks)

    def dlq_size(self) -> int:
        return len(self.dlq)
