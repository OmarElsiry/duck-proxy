import os
import time
import json
import base64
import httpx
from typing import Optional, List, Dict, Any
from fastapi import FastAPI, Request, Response
from fastapi.responses import HTMLResponse, JSONResponse, StreamingResponse
import sys
from pathlib import Path

_here = Path(__file__).resolve().parent
if str(_here) not in sys.path:
    sys.path.insert(0, str(_here))

try:
    from duck_ai import DuckChat, resolve_model, image_generation
    from duck_ai.exceptions import DuckChatError, ChallengeError, RateLimitError, APIError
except ImportError:
    from .duck_ai import DuckChat, resolve_model, image_generation
    from .duck_ai.exceptions import DuckChatError, ChallengeError, RateLimitError, APIError

app = FastAPI(title="Duck Proxy - Vercel Serverless Gateway", version="1.0.0")

# Available Duck models and their mapping
MODELS = [
    {"id": "gpt-5.6-luna", "object": "model", "owned_by": "duck"},
    {"id": "gpt5", "object": "model", "owned_by": "duck"},
    {"id": "gpt5_mini", "object": "model", "owned_by": "duck"},
    {"id": "claude", "object": "model", "owned_by": "duck"},
    {"id": "mistral", "object": "model", "owned_by": "duck"},
    {"id": "gemma", "object": "model", "owned_by": "duck"},
    {"id": "gpt_oss", "object": "model", "owned_by": "duck"},
    {"id": "image", "object": "model", "owned_by": "duck"},
    {"id": "dall-e-3", "object": "model", "owned_by": "duck"},
]

def get_proxy_client() -> Optional[httpx.Client]:
    """Configure optional proxy if running on cloud/datacenter IP."""
    proxy = os.getenv("RESIDENTIAL_PROXY") or os.getenv("HTTPS_PROXY") or os.getenv("HTTP_PROXY")
    if proxy:
        return httpx.Client(proxy=proxy, http2=False, timeout=httpx.Timeout(45.0, connect=10.0))
    return None

def normalize_messages(messages: List[Dict[str, Any]]) -> List[Dict[str, str]]:
    out = []
    for m in messages:
        role = m.get("role", "user")
        content = m.get("content", "")
        if isinstance(content, list):
            text = "".join(part.get("text", "") for part in content if isinstance(part, dict))
        else:
            text = str(content or "")
        out.append({"role": role, "content": text})
    return out

@app.get("/", response_class=HTMLResponse)
async def root_dashboard():
    """Diagnostic Dashboard for Vercel deployment status."""
    is_vercel = "VERCEL" in os.environ
    has_proxy = bool(os.getenv("RESIDENTIAL_PROXY") or os.getenv("HTTPS_PROXY"))
    
    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Duck Proxy — Serverless Gateway</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0a0c10; color: #e1e4e8; margin: 0; padding: 40px 20px; }}
        .card {{ max-width: 720px; margin: 0 auto; background: #161b22; border: 1px solid #30363d; border-radius: 12px; padding: 32px; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }}
        h1 {{ margin-top: 0; color: #58a6ff; font-size: 24px; display: flex; align-items: center; gap: 10px; }}
        .badge {{ font-size: 12px; padding: 4px 10px; border-radius: 20px; font-weight: bold; }}
        .badge-online {{ background: #238636; color: #fff; }}
        .badge-warn {{ background: #d29922; color: #000; }}
        .badge-err {{ background: #da3633; color: #fff; }}
        .info-grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin: 24px 0; }}
        .info-box {{ background: #0d1117; padding: 16px; border-radius: 8px; border: 1px solid #21262d; }}
        .info-label {{ font-size: 12px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; }}
        .info-value {{ font-size: 16px; font-weight: 600; margin-top: 6px; }}
        pre {{ background: #0d1117; border: 1px solid #30363d; border-radius: 8px; padding: 16px; overflow-x: auto; color: #79c0ff; font-size: 13px; }}
        .alert {{ padding: 16px; border-radius: 8px; margin: 20px 0; }}
        .alert-warning {{ background: rgba(210, 153, 34, 0.15); border: 1px solid #d29922; color: #e3b341; }}
        .alert-success {{ background: rgba(35, 134, 54, 0.15); border: 1px solid #238636; color: #3fb950; }}
    </style>
</head>
<body>
    <div class="card">
        <h1>🦆 Duck Proxy <span class="badge badge-online">ONLINE</span></h1>
        <p>OpenAI-compatible serverless proxy for Duck.ai deployed on Vercel.</p>
        
        <div class="info-grid">
            <div class="info-box">
                <div class="info-label">Environment</div>
                <div class="info-value">{'Vercel Serverless' if is_vercel else 'Standalone/Local'}</div>
            </div>
            <div class="info-box">
                <div class="info-label">Egress Proxy Status</div>
                <div class="info-value">{'Configured ✅' if has_proxy else 'Direct (Datacenter IP) ⚠️'}</div>
            </div>
        </div>

        {'<div class="alert alert-warning"><strong>⚠️ Upstream Notice:</strong> Running directly on Vercel AWS Lambda IPs without a residential proxy will trigger DuckDuckGo Cloudflare 403 blocks on chat requests. To bypass, configure the <code>RESIDENTIAL_PROXY</code> environment variable in your Vercel project settings.</div>' if not has_proxy else '<div class="alert alert-success"><strong>✅ Configured:</strong> Outbound requests are routed through a residential proxy.</div>'}

        <h3>Endpoints</h3>
        <ul>
            <li><code>GET /v1/models</code> — List supported models</li>
            <li><code>POST /v1/chat/completions</code> — OpenAI chat completions (streaming & non-streaming)</li>
            <li><code>POST /v1/images/generations</code> — Image generation endpoint</li>
        </ul>

        <h3>Quick Test</h3>
        <pre>curl -X POST https://YOUR-VERCEL-URL.vercel.app/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -d '{{"model": "gpt-5.6-luna", "messages": [{{"role": "user", "content": "Hello"}}]}}'</pre>
    </div>
</body>
</html>"""
    return html

@app.get("/v1/models")
async def get_models():
    """Return OpenAI-compatible models list."""
    return {"object": "list", "data": MODELS}

@app.post("/v1/chat/completions")
async def chat_completions(request: Request):
    """Handle OpenAI-compatible chat completion requests."""
    try:
        body = await request.json()
    except Exception:
        return JSONResponse(
            {"error": {"message": "Invalid JSON body", "type": "invalid_request_error"}},
            status_code=400
        )

    model_name = body.get("model", "gpt-5.6-luna")
    if model_name.startswith("duck/"):
        model_name = model_name[5:]
    
    messages = normalize_messages(body.get("messages", []))
    stream = body.get("stream", False)
    web_search = body.get("web_search", False)

    try:
        http_client = get_proxy_client()
        chat = DuckChat(model=model_name, client=http_client, timeout=25.0, effort="reasoning")
    except Exception as e:
        return JSONResponse(
            {"error": {"message": f"Failed to initialize DuckChat client: {str(e)}", "type": "internal_error"}},
            status_code=500
        )

    if stream:
        def stream_generator():
            request_id = f"chatcmpl-{int(time.time()*1000)}"
            created_ts = int(time.time())
            try:
                # First chunk with role
                first_chunk = {
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": created_ts,
                    "model": model_name,
                    "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": None}],
                }
                yield f"data: {json.dumps(first_chunk)}\n\n"

                for text_chunk in chat.stream(messages, web_search=web_search):
                    chunk = {
                        "id": request_id,
                        "object": "chat.completion.chunk",
                        "created": created_ts,
                        "model": model_name,
                        "choices": [{"index": 0, "delta": {"content": text_chunk}, "finish_reason": None}],
                    }
                    yield f"data: {json.dumps(chunk)}\n\n"

                # Final chunk
                final_chunk = {
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": created_ts,
                    "model": model_name,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                }
                yield f"data: {json.dumps(final_chunk)}\n\n"
                yield "data: [DONE]\n\n"
            except Exception as stream_err:
                err_chunk = {
                    "error": {
                        "message": str(stream_err),
                        "type": "upstream_error",
                        "upstream_blocked": "blocked" in str(stream_err).lower() or "403" in str(stream_err)
                    }
                }
                yield f"data: {json.dumps(err_chunk)}\n\n"
                yield "data: [DONE]\n\n"

        return StreamingResponse(stream_generator(), media_type="text/event-stream")

    # Non-streaming
    try:
        response_text = chat.ask(messages, web_search=web_search)
        created_ts = int(time.time())
        return JSONResponse({
            "id": f"chatcmpl-{created_ts}",
            "object": "chat.completion",
            "created": created_ts,
            "model": model_name,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": response_text},
                    "finish_reason": "stop"
                }
            ],
            "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
        })
    except Exception as e:
        err_msg = str(e)
        is_blocked = "403" in err_msg or "challenge" in err_msg.lower() or "blocked" in err_msg.lower()
        return JSONResponse(
            {
                "error": {
                    "message": err_msg,
                    "type": "upstream_blocked" if is_blocked else "upstream_error",
                    "advice": "Duck.ai blocks AWS/Vercel datacenter IPs. Set RESIDENTIAL_PROXY in Vercel to bypass." if is_blocked else ""
                }
            },
            status_code=502 if is_blocked else 500
        )

@app.post("/v1/images/generations")
async def generate_images(request: Request):
    """Handle OpenAI-compatible image generations."""
    try:
        body = await request.json()
        prompt = body.get("prompt", "")
        http_client = get_proxy_client()
        chat = DuckChat(model=image_generation, client=http_client, timeout=30.0)
        img_bytes = chat.generate_image(prompt)
        b64_data = base64.b64encode(img_bytes).decode("ascii")
        return {
            "created": int(time.time()),
            "data": [{"b64_json": b64_data}]
        }
    except Exception as e:
        return JSONResponse(
            {"error": {"message": str(e), "type": "image_generation_error"}},
            status_code=500
        )
