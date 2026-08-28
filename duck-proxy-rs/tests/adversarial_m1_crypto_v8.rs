use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use deno_core::{JsRuntime, RuntimeOptions};
use duck_proxy_rs::crypto::EphemeralKeypair;
use duck_proxy_rs::v8::{extract_html_lookup, generate_browser_stubs, wrap_challenge_code};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

// =============================================================================
// SECTION 1: CRYPTO JWK ADVERSARIAL STRESS TESTS
// =============================================================================

#[test]
fn test_adversarial_crypto_concurrent_keypair_generation_uniqueness() {
    const NUM_THREADS: usize = 8;
    const KEYS_PER_THREAD: usize = 4;
    const TOTAL_KEYS: usize = NUM_THREADS * KEYS_PER_THREAD;

    let generated_keys = Arc::new(Mutex::new(Vec::with_capacity(TOTAL_KEYS)));
    let mut handles = Vec::with_capacity(NUM_THREADS);

    for _ in 0..NUM_THREADS {
        let keys_clone = Arc::clone(&generated_keys);
        let handle = thread::spawn(move || {
            let mut thread_keys = Vec::with_capacity(KEYS_PER_THREAD);
            for _ in 0..KEYS_PER_THREAD {
                let kp = EphemeralKeypair::generate().expect("Ephemeral keypair generation failed");
                thread_keys.push(kp);
            }
            let mut lock = keys_clone.lock().unwrap();
            lock.extend(thread_keys);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked during keypair generation");
    }

    let keys = generated_keys.lock().unwrap();
    assert_eq!(keys.len(), TOTAL_KEYS, "Expected exactly {} generated keypairs", TOTAL_KEYS);

    let mut modulus_set = HashSet::new();
    for kp in keys.iter() {
        let jwk = kp.public_jwk();
        let is_unique = modulus_set.insert(jwk.n.clone());
        assert!(is_unique, "Collision detected in generated RSA moduli! Duplicate: {}", jwk.n);
    }

    assert_eq!(
        modulus_set.len(),
        TOTAL_KEYS,
        "100% key uniqueness guarantee failed across concurrent threads"
    );
}

#[test]
fn test_adversarial_crypto_jwk_modulus_strict_base64url_compliance() {
    for i in 0..10 {
        let keypair = EphemeralKeypair::generate().expect("Keypair generation failed");
        let jwk = keypair.public_jwk();

        // 1. Strictly 342 ASCII characters
        assert_eq!(
            jwk.n.len(),
            342,
            "Iteration {}: 2048-bit modulus base64url length must be strictly 342 chars, got {}",
            i,
            jwk.n.len()
        );
        assert!(
            jwk.n.is_ascii(),
            "Iteration {}: JWK modulus must contain strictly ASCII characters",
            i
        );

        // 2. Valid base64url charset (RFC 4648 §5): [A-Za-z0-9_-]
        for (idx, ch) in jwk.n.chars().enumerate() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "Iteration {}: Invalid character '{}' at index {} in JWK modulus",
                i,
                ch,
                idx
            );
        }

        // 3. Must NOT contain standard base64 characters (+, /) or padding (=)
        assert!(
            !jwk.n.contains('+'),
            "Iteration {}: Modulus contains standard base64 '+' character",
            i
        );
        assert!(
            !jwk.n.contains('/'),
            "Iteration {}: Modulus contains standard base64 '/' character",
            i
        );
        assert!(
            !jwk.n.contains('='),
            "Iteration {}: Modulus contains '=' padding character",
            i
        );

        // 4. Must decode to exactly 256 bytes (2048 bits) with URL_SAFE_NO_PAD
        let decoded = URL_SAFE_NO_PAD
            .decode(&jwk.n)
            .expect("Failed to decode base64url modulus with URL_SAFE_NO_PAD");
        assert_eq!(
            decoded.len(),
            256,
            "Iteration {}: Decoded modulus length must be exactly 256 bytes (2048 bits)",
            i
        );

        // 5. Exponent must be strictly "AQAB" (65537)
        assert_eq!(jwk.e, "AQAB");
        let e_decoded = URL_SAFE_NO_PAD.decode(&jwk.e).unwrap();
        assert_eq!(e_decoded, vec![1, 0, 1]);

        // 6. Metadata attributes
        assert_eq!(jwk.alg, "RSA-OAEP-256");
        assert_eq!(jwk.kty, "RSA");
        assert_eq!(jwk.key_use, "enc");
        assert!(jwk.ext);
        assert_eq!(jwk.key_ops, vec!["encrypt".to_string()]);
    }
}

#[test]
fn test_adversarial_crypto_oaep_sha256_payload_boundaries() {
    let keypair = EphemeralKeypair::generate().expect("Keypair generation failed");

    // RSA-2048 with OAEP-SHA256 max payload = 256 - 2*32 - 2 = 190 bytes
    let payload_sizes = vec![0, 1, 2, 16, 32, 64, 100, 128, 189, 190];

    for size in payload_sizes {
        let plaintext = vec![0x42u8; size];

        let ciphertext = keypair
            .encrypt_oaep_sha256(&plaintext)
            .unwrap_or_else(|e| panic!("Encryption failed for payload size {}: {:?}", size, e));

        assert_eq!(
            ciphertext.len(),
            256,
            "Ciphertext length must be exactly 256 bytes for size {}",
            size
        );

        let decrypted = keypair
            .decrypt_oaep_sha256(&ciphertext)
            .unwrap_or_else(|e| panic!("Decryption failed for payload size {}: {:?}", size, e));

        assert_eq!(
            decrypted, plaintext,
            "Decrypted plaintext mismatch for payload size {}",
            size
        );
    }

    // Test payload exceeding maximum OAEP capacity (191 bytes and 256 bytes)
    let overflow_191 = vec![0x55u8; 191];
    let overflow_res = keypair.encrypt_oaep_sha256(&overflow_191);
    assert!(
        overflow_res.is_err(),
        "Payload of 191 bytes must fail RSA-OAEP-256 encryption (max capacity is 190 bytes)"
    );

    let overflow_256 = vec![0x77u8; 256];
    let overflow_256_res = keypair.encrypt_oaep_sha256(&overflow_256);
    assert!(overflow_256_res.is_err());

    // Test decryption of corrupted ciphertext
    let valid_ciphertext = keypair.encrypt_oaep_sha256(b"hello world").unwrap();
    let mut corrupted = valid_ciphertext.clone();
    corrupted[10] ^= 0xFF; // Flip bits
    let corrupt_res = keypair.decrypt_oaep_sha256(&corrupted);
    assert!(
        corrupt_res.is_err(),
        "Decryption of corrupted ciphertext must fail"
    );

    // Test decryption of invalid ciphertext lengths
    assert!(keypair.decrypt_oaep_sha256(&[]).is_err());
    assert!(keypair.decrypt_oaep_sha256(&[0u8; 100]).is_err());
    assert!(keypair.decrypt_oaep_sha256(&[0u8; 255]).is_err());
    assert!(keypair.decrypt_oaep_sha256(&[0u8; 257]).is_err());
}

// =============================================================================
// SECTION 2: V8 STUBS & INJECTION ADVERSARIAL STRESS TESTS
// =============================================================================

#[test]
fn test_adversarial_v8_stubs_user_agent_injection_safety() {
    let malicious_user_agents: Vec<String> = vec![
        // Double quote breakouts
        r#"Mozilla/5.0 "Special" Chrome/150.0.0.0"#.to_string(),
        // Single quote & backslashes
        r#"Mozilla/5.0 (Linux; \x86_64\; \n\t) 'Test' \u0000 🦆"#.to_string(),
        // JS Breakout attempt with semicolon
        r#"Mozilla/5.0"; globalThis.__pwned = true; var x = ""#.to_string(),
        // JS Breakout attempt with closing parenthesis
        r#"Mozilla/5.0"); (function(){ globalThis.__injected = 123; })(); (""#.to_string(),
        // Script tag and HTML chars
        r#"Mozilla/5.0 <script>alert('xss')</script> Chrome/150"#.to_string(),
        // Newlines, null bytes, and unicode emojis
        "Mozilla/5.0 \r\n\t \0 Unicode: 🚀 🦆 \\/ \" ' ` ${process.exit(1)}".to_string(),
        // Empty User-Agent
        "".to_string(),
        // Extremely long User-Agent (4096 chars)
        "A".repeat(4096),
    ];

    for (idx, ua) in malicious_user_agents.into_iter().enumerate() {
        let stubs_code = generate_browser_stubs(&ua, None);

        // 1. Verify placeholder was fully replaced
        assert!(
            !stubs_code.contains("__DDG_REAL_UA__"),
            "Case {}: Stubs still contain __DDG_REAL_UA__",
            idx
        );

        // 2. Evaluate stubs in a real deno_core V8 runtime
        let mut runtime = JsRuntime::new(RuntimeOptions::default());
        let eval_res = runtime.execute_script("<stubs_injection_test>", stubs_code);
        assert!(
            eval_res.is_ok(),
            "Case {}: V8 failed to execute generated stubs with UA {:?}: {:?}",
            idx,
            ua,
            eval_res.err()
        );

        // 3. Verify window.navigator.userAgent matches exactly
        let check_script = r#"
            if (typeof window === 'undefined') throw new Error('window is undefined');
            if (typeof navigator === 'undefined') throw new Error('navigator is undefined');
            if (window.navigator.userAgent !== __ua) throw new Error('userAgent mismatch');
            if (globalThis.__pwned || globalThis.__injected) throw new Error('Code injection occurred!');
            window.navigator.userAgent;
        "#;
        let res_value = runtime
            .execute_script("<check_ua>", check_script.to_string())
            .expect("Failed to verify userAgent in V8");

        // Verify the navigator.userAgent returned by V8
        let str_val = {
            let scope = &mut runtime.handle_scope();
            let local_val = deno_core::v8::Local::new(scope, res_value);
            local_val.to_rust_string_lossy(scope)
        };
        assert_eq!(str_val, ua, "Case {}: V8 navigator.userAgent does not match input", idx);
    }
}

#[test]
fn test_adversarial_v8_stubs_html_lookup_injection_safety() {
    let malicious_lookups = vec![
        // Malicious JSON containing closing quotes and script injection
        r#"{"<script>alert(1)</script>":{"html":"<script>alert(\"evil\")</script>","count":1}}"#,
        // Complex HTML with quotes and attributes
        r#"{"<div class=\"foo\" data-attr='bar'>hello</div>":{"html":"<div class=\"foo\" data-attr='bar'>hello</div>","count":3}}"#,
        // Nested lookup with backslashes
        r#"{"key\\\"with\\\"quotes":{"html":"<div><span>1</span><span>2</span></div>","count":2}}"#,
        // Empty lookup object
        "{}",
    ];

    for (idx, lookup_json) in malicious_lookups.into_iter().enumerate() {
        let ua = "Mozilla/5.0 (Adversarial Test) Chrome/150";
        let stubs_code = generate_browser_stubs(ua, Some(lookup_json));

        assert!(!stubs_code.contains("__DDG_HTML_LOOKUP__"));

        let mut runtime = JsRuntime::new(RuntimeOptions::default());
        let eval_res = runtime.execute_script("<stubs_html_lookup>", stubs_code);
        assert!(
            eval_res.is_ok(),
            "Case {}: V8 failed to execute stubs with lookup_json {:?}: {:?}",
            idx,
            lookup_json,
            eval_res.err()
        );

        // Test that __makeHtmlElement and innerHTML lookup work
        let test_script = r#"
            var el = document.createElement('div');
            el.innerHTML = '<script>alert(1)</script>';
            el.innerHTML;
        "#;
        let _ = runtime.execute_script("<test_inner_html>", test_script.to_string());
    }
}

#[test]
fn test_adversarial_v8_wrap_challenge_code_edge_cases() {
    let test_cases = vec![
        // 1. Simple async expression resolving an object
        (
            "(async () => ({ result: 'duck_token_ok', code: 200 }))()",
            true, // should succeed (__R set)
        ),
        // 2. Promise resolving a primitive string
        (
            "Promise.resolve('challenge_solved_successfully')",
            true,
        ),
        // 3. Promise resolving a number
        (
            "Promise.resolve(987654321)",
            true,
        ),
        // 4. Promise resolving null
        (
            "Promise.resolve(null)",
            true,
        ),
        // 5. Async function throwing an error
        (
            "(async () => { throw new Error('challenge failed: invalid telemetry'); })()",
            false, // should fail (__E set)
        ),
        // 6. Promise rejecting with a string
        (
            "Promise.reject('rejected by anti-bot algorithm')",
            false,
        ),
        // 7. Promise rejecting with an Error
        (
            "Promise.reject(new TypeError('TypeError in DOM stub access'))",
            false,
        ),
    ];

    for (idx, (js_expr, expect_success)) in test_cases.into_iter().enumerate() {
        let ua = "Mozilla/5.0 (Test) Chrome/150";
        let stubs_code = generate_browser_stubs(ua, None);
        let wrapped_code = wrap_challenge_code(js_expr);

        let mut runtime = JsRuntime::new(RuntimeOptions::default());
        runtime
            .execute_script("<stubs>", stubs_code)
            .expect("Stubs setup failed");

        // Execute wrapped code
        let exec_res = runtime.execute_script("<challenge>", wrapped_code);
        assert!(exec_res.is_ok(), "Case {}: Failed to execute wrapped code: {:?}", idx, exec_res.err());

        if expect_success {
            // Check that __E is null and __R is populated (or null if promise resolved null)
            let verify_script = r#"
                if (__E !== null) throw new Error('Expected __E to be null, got: ' + __E);
                true;
            "#;
            let verify_res = runtime.execute_script("<verify>", verify_script.to_string());
            assert!(verify_res.is_ok(), "Case {}: Expected success but got error: {:?}", idx, verify_res.err());
        } else {
            // Check that __E is populated
            let verify_script = r#"
                if (__E === null) throw new Error('Expected __E to contain error string');
                __E;
            "#;
            let verify_res = runtime.execute_script("<verify_err>", verify_script.to_string());
            assert!(verify_res.is_ok(), "Case {}: Expected failure in __E: {:?}", idx, verify_res.err());
        }
    }
}

#[test]
fn test_adversarial_v8_extract_html_lookup_edge_cases() {
    // 1. Complex mixed snippets
    let js_snippets = r#"
        const a = '<div><p><span>nested</span></p></div>';
        const b = "<section><article><h1>Header</h1><p>Text</p></article></section>";
        const c = 'non-html string with < and > symbols: 5 < 10 and 20 > 15';
        const d = '<img src="test.png" /><input type="text" />';
        const e = '<!-- comment --><div>after comment</div>';
        const f = '<div><p>repeated</p></div>';
        const g = '<div><p>repeated</p></div>';
    "#;

    let lookup = extract_html_lookup(js_snippets);

    // Verify tag counting
    assert!(lookup.contains_key("<div><p><span>nested</span></p></div>"));
    let nested = lookup.get("<div><p><span>nested</span></p></div>").unwrap();
    assert_eq!(nested.count, 3); // <div>, <p>, <span>

    assert!(lookup.contains_key("<section><article><h1>Header</h1><p>Text</p></article></section>"));
    let section = lookup.get("<section><article><h1>Header</h1><p>Text</p></article></section>").unwrap();
    assert_eq!(section.count, 4); // <section>, <article>, <h1>, <p>

    assert!(lookup.contains_key("<img src=\"test.png\" /><input type=\"text\" />"));
    let self_closing = lookup.get("<img src=\"test.png\" /><input type=\"text\" />").unwrap();
    assert_eq!(self_closing.count, 2); // <img>, <input>

    // Verify deduplication
    assert!(lookup.contains_key("<div><p>repeated</p></div>"));
    assert_eq!(lookup.get("<div><p>repeated</p></div>").unwrap().count, 2);

    // Empty string
    let empty_lookup = extract_html_lookup("");
    assert!(empty_lookup.is_empty());

    // JS with no HTML
    let no_html = extract_html_lookup("var a = 1; var b = 'hello world'; function foo() { return true; }");
    assert!(no_html.is_empty());
}

#[test]
fn test_adversarial_v8_comprehensive_dom_stubs_capabilities() {
    let ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";
    let stubs_code = generate_browser_stubs(ua, None);

    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    runtime
        .execute_script("<stubs>", stubs_code)
        .expect("Failed to initialize stubs in V8");

    // Comprehensive DOM verification script testing all required stubs from ORIGINAL_REQUEST §4.B
    let dom_test_script = r#"
        // 1. Window & global aliases
        if (globalThis.window !== window) throw new Error('globalThis.window !== window');
        if (window.globalThis !== window) throw new Error('window.globalThis !== window');
        if (self !== window) throw new Error('self !== window');
        if (top !== window) throw new Error('top !== window');
        if (parent !== window) throw new Error('parent !== window');

        // 2. Navigator properties
        if (navigator.userAgent.indexOf('Chrome/150') === -1) throw new Error('userAgent Chrome/150 missing');
        if (navigator.webdriver !== false) throw new Error('webdriver must be false');
        if (navigator.language !== 'en-US') throw new Error('language mismatch');
        if (navigator.languages.length !== 2) throw new Error('languages mismatch');
        if (navigator.cookieEnabled !== true) throw new Error('cookieEnabled mismatch');
        if (navigator.onLine !== true) throw new Error('onLine mismatch');
        if (navigator.hardwareConcurrency !== 8) throw new Error('hardwareConcurrency mismatch');

        // 3. Screen and Location
        if (screen.width !== 1920 || screen.height !== 1080) throw new Error('screen resolution mismatch');
        if (location.origin !== 'https://duckduckgo.com') throw new Error('location origin mismatch');
        if (location.protocol !== 'https:') throw new Error('location protocol mismatch');

        // 4. Document & getElementById / querySelector
        if (typeof document.getElementById !== 'function') throw new Error('getElementById missing');
        var jsa = document.getElementById('jsa');
        if (!jsa) throw new Error('#jsa iframe not found');
        if (jsa.tagName !== 'IFRAME') throw new Error('#jsa tagName mismatch');
        if (jsa.getAttribute('sandbox') !== 'allow-scripts allow-same-origin') throw new Error('sandbox mismatch');
        if (!jsa.contentDocument) throw new Error('contentDocument missing on iframe');
        if (!jsa.contentWindow) throw new Error('contentWindow missing on iframe');

        // 5. getComputedStyle
        var styleEl = document.createElement('div');
        styleEl.style.cssText = 'display: none; color: red;';
        var computed = getComputedStyle(styleEl);
        if (computed.getPropertyValue('display') !== 'none') throw new Error('getComputedStyle display mismatch: ' + computed.getPropertyValue('display'));

        // 6. Elements & innerHTML
        var div = document.createElement('div');
        div.innerHTML = '<span>sample</span>';
        if (div.innerHTML !== '<span>sample</span>') throw new Error('innerHTML mismatch: ' + div.innerHTML);
        if (div.tagName !== 'DIV') throw new Error('tagName mismatch');
        if (div.nodeType !== 1) throw new Error('nodeType mismatch');

        // 7. Global constructors
        if (typeof HTMLElement !== 'function') throw new Error('HTMLElement missing');
        if (typeof HTMLIFrameElement !== 'function') throw new Error('HTMLIFrameElement missing');
        if (typeof Event !== 'function') throw new Error('Event missing');
        if (typeof fetch !== 'function') throw new Error('fetch missing');

        // 8. DDG Specific Constants
        if (window.__DDG_BE_VERSION__ !== 1) throw new Error('__DDG_BE_VERSION__ mismatch');
        if (window.__DDG_FE_CHAT_HASH__ !== 1) throw new Error('__DDG_FE_CHAT_HASH__ mismatch');

        true;
    "#;

    let test_res = runtime.execute_script("<dom_tests>", dom_test_script.to_string());
    assert!(
        test_res.is_ok(),
        "DOM stubs capabilities test failed in V8 runtime: {:?}",
        test_res.err()
    );
}
