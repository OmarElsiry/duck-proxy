import os, time, base64, yaml
from fastapi import FastAPI, Request
from fastapi.responses import StreamingResponse, JSONResponse
import litellm
import duck_provider  # registers duck custom provider
from duck_ai import DuckChat, image_generation

with open("config.yaml") as f:
    MODELS = [m["model_name"] for m in yaml.safe_load(f)["model_list"]]

app = FastAPI()


def _model(name: str) -> str:
    return name if name.startswith("duck/") else f"duck/{name}"


@app.get("/v1/models")
async def models():
    return {"object": "list",
            "data": [{"id": m, "object": "model", "owned_by": "duck"} for m in MODELS]}


@app.post("/v1/chat/completions")
async def chat(request: Request):
    body = await request.json()
    model = _model(body["model"])
    stream = body.get("stream", False)

    if stream:
        def gen():
            for chunk in litellm.completion(model=model, messages=body["messages"],
                                           stream=True, **_opts(body)):
                delta = ""
                if chunk.choices and chunk.choices[0].delta:
                    delta = chunk.choices[0].delta.content or ""
                yield f"data: {_chunk_json(chunk.id or 'x', model, delta)}\n\n"
            yield "data: [DONE]\n\n"
        return StreamingResponse(gen(), media_type="text/event-stream")

    try:
        resp = litellm.completion(model=model, messages=body["messages"], **_opts(body))
        return JSONResponse(resp.model_dump())
    except Exception as e:
        return JSONResponse({"error": str(e)}, status_code=500)


@app.post("/v1/images/generations")
async def images(request: Request):
    body = await request.json()
    try:
        data = duck_provider._get_chat_client(image_generation).generate_image(body.get("prompt", ""))
        return {"created": int(time.time()),
                "data": [{"b64_json": base64.b64encode(data).decode()}]}
    except Exception as e:
        return JSONResponse({"error": str(e)}, status_code=500)


def _opts(body: dict) -> dict:
    # ponytail: forward only web_search toggle; rest ignored by provider
    out = {}
    if body.get("web_search"):
        out["web_search"] = True
    return out


def _chunk_json(cid, model, delta):
    import json
    return json.dumps({
        "id": cid, "object": "chat.completion.chunk", "created": int(time.time()),
        "model": model,
        "choices": [{"index": 0, "delta": {"content": delta}, "finish_reason": None}],
    })


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
