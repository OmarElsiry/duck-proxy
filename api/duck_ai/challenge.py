from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import threading
import time
from typing import Dict, List, Optional, Tuple

from .exceptions import ChallengeError

STUBS_PATH = os.path.join(os.path.dirname(__file__), "stubs.js")
STUBS_TEMPLATE: Optional[str] = None
STUBS_LOCK = threading.Lock()
RUNTIME_LOCK = threading.Lock()


def load_stubs() -> str:
    global STUBS_TEMPLATE
    if STUBS_TEMPLATE is None:
        with STUBS_LOCK:
            if STUBS_TEMPLATE is None:
                with open(STUBS_PATH, "r", encoding="utf-8") as f:
                    STUBS_TEMPLATE = f.read()
    return STUBS_TEMPLATE


def b64_sha256(s: str) -> str:
    return base64.b64encode(hashlib.sha256(s.encode("utf-8")).digest()).decode("ascii")


def has_miniracer() -> bool:
    try:
        import py_mini_racer
        return True
    except Exception:
        return False


def extract_html_inputs(js: str) -> List[str]:
    found: List[str] = []
    seen = set()
    for m in re.finditer(r"""(['"])(<[^'"]{1,400}?)\1""", js):
        s = m.group(2)
        if s in seen:
            continue
        seen.add(s)
        found.append(s)
    return found


def serialize_etree(node) -> str:
    out = ""
    for child in node:
        tag = child.tag
        if not isinstance(tag, str):
            continue
        if "}" in tag:
            tag = tag.split("}", 1)[1]
        attrs = "".join(f' {k}="{v}"' for k, v in child.attrib.items())
        out += f"<{tag}{attrs}>"
        if child.text:
            out += child.text
        out += serialize_etree(child)
        out += f"</{tag}>"
        if child.tail:
            out += child.tail
    return out


def normalize_html(s: str) -> Tuple[str, int]:
    try:
        import html5lib
        frag = html5lib.parseFragment(s, treebuilder="etree", namespaceHTMLElements=False)
        inner = serialize_etree(frag)
        count = sum(1 for e in frag.iter() if isinstance(e.tag, str)) - 1
        if count < 0:
            count = 0
        return inner, count
    except Exception:
        return s, 0


def build_html_lookup(js: str) -> Dict[str, Dict[str, object]]:
    lookup: Dict[str, Dict[str, object]] = {}
    for s in extract_html_inputs(js):
        norm, count = normalize_html(s)
        lookup[s] = {"html": norm, "count": count}
    return lookup


def solve_challenge(challenge_b64: str, user_agent: str) -> str:
    if not has_miniracer():
        raise ChallengeError(
            "JavaScript engine not available. Install with: pip install mini-racer"
        )
    try:
        from py_mini_racer import MiniRacer
    except Exception as e:
        raise ChallengeError(f"failed to import mini-racer: {e}") from e

    try:
        js = base64.b64decode(challenge_b64).decode("utf-8", errors="replace")
    except Exception as e:
        raise ChallengeError(f"challenge is not valid base64: {e}") from e

    html_lookup = build_html_lookup(js)
    stubs = load_stubs().replace("__DDG_REAL_UA__", json.dumps(user_agent))
    stubs = stubs.replace("__DDG_HTML_LOOKUP__", json.dumps(html_lookup))

    with RUNTIME_LOCK:
        ctx = MiniRacer()
        try:
            ctx.eval(stubs)
            ctx.eval(
                "(%s).then(function(v){__R=v;}).catch(function(e){__E=String((e&&e.stack)||e);});"
                % js
            )
            for _ in range(100):
                if ctx.execute("__R !== null || __E !== null"):
                    break
                time.sleep(0.02)
            err = ctx.execute("__E")
            if err:
                raise ChallengeError(f"challenge JS error: {err}")
            res = ctx.execute("__R")
        finally:
            try:
                del ctx
            except Exception:
                pass

    if not isinstance(res, dict):
        raise ChallengeError("challenge returned no result")
    client_hashes = list(res.get("client_hashes") or [])
    if not client_hashes:
        raise ChallengeError("challenge returned empty client_hashes")
    client_hashes[0] = user_agent
    res["client_hashes"] = [b64_sha256(t) for t in client_hashes]
    if "meta" in res and isinstance(res["meta"], dict):
        res["meta"]["origin"] = "https://duck.ai"
        res["meta"]["stack"] = (
            "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)\n    at async https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1620812"
        )
        res["meta"]["duration"] = "25"
    payload = json.dumps(res, separators=(",", ":")).encode("utf-8")
    return base64.b64encode(payload).decode("ascii")


def make_fe_signals(*, duration_ms: int = 30000) -> str:
    import random as r

    now_ms = int(time.time() * 1000)
    start_ms = now_ms - duration_ms
    events = [
        {"name": "onboarding_impression", "delta": r.randint(200, 260)},
        {"name": "action", "delta": r.randint(10000, 15000), "trusted": True},
        {"name": "onboarding_impression", "delta": r.randint(15100, 16000)},
        {"name": "onboarding_finish", "delta": r.randint(24000, 27000)},
        {"name": "startNewChat_free", "delta": r.randint(27100, 28500)},
    ]
    payload = {
        "start": start_ms,
        "events": events,
        "end": r.randint(28510, 29000),
    }
    return base64.b64encode(
        json.dumps(payload, separators=(",", ":")).encode("utf-8")
    ).decode("ascii")
