#!/usr/bin/env python3
"""
Automated Test Suite for Live Vercel Duck-Proxy Deployment
Usage:
    python3 scripts/test_vercel_deployment.py https://<your-deployment>.vercel.app
"""

import sys
import time
import json
import httpx

def run_tests(base_url: str):
    base_url = base_url.rstrip("/")
    print(f"\n==================================================")
    print(f"🧪 Testing Live Vercel Deployment: {base_url}")
    print(f"==================================================\n")

    client = httpx.Client(timeout=35.0)

    # Test 1: Web Dashboard / Root
    print("👉 [Test 1/4] GET / (Root Diagnostic Dashboard)...")
    try:
        t0 = time.time()
        r = client.get(f"{base_url}/")
        elapsed = time.time() - t0
        print(f"   Status: {r.status_code} ({elapsed:.2f}s)")
        if r.status_code == 200:
            print("   ✅ Root dashboard is live and serving from Vercel!")
        else:
            print(f"   ❌ Unexpected status: {r.status_code}")
    except Exception as e:
        print(f"   ❌ Connection failed: {e}")

    # Test 2: Models Endpoint
    print("\n👉 [Test 2/4] GET /v1/models (OpenAI Models List)...")
    try:
        t0 = time.time()
        r = client.get(f"{base_url}/v1/models")
        elapsed = time.time() - t0
        print(f"   Status: {r.status_code} ({elapsed:.2f}s)")
        if r.status_code == 200:
            data = r.json()
            models = [m["id"] for m in data.get("data", [])]
            print(f"   ✅ Models endpoint operational! ({len(models)} models available)")
            print(f"   Supported models: {', '.join(models[:5])}...")
        else:
            print(f"   ❌ Failed with status: {r.status_code}, body: {r.text[:200]}")
    except Exception as e:
        print(f"   ❌ Models request failed: {e}")

    # Test 3: Chat Completions (Non-Streaming)
    print("\n👉 [Test 3/4] POST /v1/chat/completions (Upstream Duck.ai Chat)...")
    try:
        t0 = time.time()
        payload = {
            "model": "gpt5",
            "messages": [{"role": "user", "content": "Say hello in one word."}]
        }
        r = client.post(f"{base_url}/v1/chat/completions", json=payload)
        elapsed = time.time() - t0
        print(f"   Status: {r.status_code} ({elapsed:.2f}s)")
        print(f"   Response Body:")
        print(f"   {r.text}")
        if r.status_code == 200:
            print("   🎉 SUCCESS! Chat completion succeeded through Vercel!")
        elif r.status_code == 502:
            print("   ⚠️ EXPECTED BLOCK: DuckDuckGo Cloudflare WAF blocked Vercel's datacenter IP.")
            print("   💡 Solution: Configure RESIDENTIAL_PROXY in Vercel environment variables.")
        else:
            print(f"   ❌ HTTP {r.status_code}")
    except Exception as e:
        print(f"   ❌ Chat request failed: {e}")

    # Test 4: Streaming SSE Test
    print("\n👉 [Test 4/4] POST /v1/chat/completions (Streaming SSE)...")
    try:
        t0 = time.time()
        payload = {
            "model": "gpt5",
            "stream": True,
            "messages": [{"role": "user", "content": "Count to 3."}]
        }
        with client.stream("POST", f"{base_url}/v1/chat/completions", json=payload) as r:
            elapsed = time.time() - t0
            print(f"   Status: {r.status_code} ({elapsed:.2f}s)")
            print("   Stream Chunks:")
            for line in r.iter_lines():
                if line:
                    print(f"   {line}")
    except Exception as e:
        print(f"   ❌ Streaming request failed: {e}")

    print(f"\n==================================================")
    print(f"🏁 Test Suite Completed for {base_url}")
    print(f"==================================================\n")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/test_vercel_deployment.py <DEPLOYMENT_URL>")
        sys.exit(1)
    run_tests(sys.argv[1])
