import pytest
from taskpulse.queue import TaskQueue

def test_queue_push_pop():
    q = TaskQueue()
    q.push({'id': 1, 'name': 'task-1'})
    assert q.size() == 1
    item = q.pop()
    assert item['id'] == 1
    assert q.size() == 0

def test_dlq_handling():
    q = TaskQueue(max_retries=3)
    failed_task = {'id': 99, 'error': 'timeout'}
    q.send_to_dlq(failed_task)
    assert q.dlq_size() == 1
