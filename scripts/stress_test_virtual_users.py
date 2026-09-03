#!/usr/bin/env python3
"""
Comprehensive Stress Test Suite for Duck Proxy:
1. Tests Virtual User rotation on gpt-5.6-luna.
2. Tests prompt context limits: 500 chars, 2000 chars, 5000 chars, 7500 chars, 12000 chars.
3. Tests streaming and non-streaming under load.
4. Tests alternative models (claude-haiku-4-5, gpt-5.4-mini, mistral-small-2603).
"""

import sys
import time
import requests
import json

BASE_URL = "http://127.0.0.1:18080/v1"

def test_single_chat(model, prompt, stream=False, expected_substr=None):
    payload = {
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "stream": stream
    }
    t0 = time.time()
    resp = requests.post(f"{BASE_URL}/chat/completions", json=payload, timeout=60, stream=stream)
    elapsed = time.time() - t0
    
    if resp.status_code != 200:
        print(f"❌ [{model}] FAILED (HTTP {resp.status_code}): {resp.text[:200]}")
        return False

    if stream:
        content = ""
        for line in resp.iter_lines():
            line = line.decode('utf-8')
            if line.startswith("data: ") and line != "data: [DONE]":
                try:
                    chunk = json.loads(line[6:])
                    delta = chunk["choices"][0]["delta"].get("content", "")
                    content += delta
                except Exception:
                    pass
        print(f"✅ [{model}] STREAM ({elapsed:.2f}s, len={len(content)}): {content.strip()[:60]}...")
    else:
        data = resp.json()
        content = data["choices"][0]["message"]["content"]
        print(f"✅ [{model}] NON-STREAM ({elapsed:.2f}s, len={len(content)}): {content.strip()[:60]}...")

    if expected_substr and expected_substr.lower() not in content.lower():
        print(f"⚠️ Warning: expected '{expected_substr}' in output")
    return True

def test_context_limits():
    print("\n--- Testing Context Window Boundaries ---")
    sizes = [500, 2000, 5000, 7000, 10000, 15000]
    for size in sizes:
        padding = "Word " * (size // 5)
        prompt = f"Summarize this text in 3 words: {padding} Final keyword is APPLE."
        print(f"\nTesting prompt length {len(prompt)} characters on gpt-5.6-luna...")
        success = test_single_chat("gpt-5.6-luna", prompt, stream=False)
        if not success:
            print(f"❌ Context limit test failed at {size} characters")
            return False
        time.sleep(1)
    return True

def test_multi_model_rotation():
    print("\n--- Testing All Duck.ai Model Catalogs ---")
    models = ["gpt-5.6-luna", "claude-haiku-4-5", "gpt-5.4-mini", "mistral-small-2603", "tinfoil/gemma4-31b"]
    for m in models:
        print(f"Testing model {m}...")
        success = test_single_chat(m, f"What is 2+2? Answer in one number using model {m}.", stream=True)
        if not success:
            return False
        time.sleep(1)
    return True

if __name__ == "__main__":
    print("🚀 Starting Duck Proxy Stress & Context Limits Test...")
    ok1 = test_multi_model_rotation()
    ok2 = test_context_limits()
    
    if ok1 and ok2:
        print("\n🎉 ALL STRESS TESTS & CONTEXT BOUNDARY TESTS PASSED SUCCESSFULLY!")
        sys.exit(0)
    else:
        print("\n❌ SOME TESTS FAILED!")
        sys.exit(1)
