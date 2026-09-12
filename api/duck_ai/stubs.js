var __ua = __DDG_REAL_UA__;
var __HTML_LOOKUP = __DDG_HTML_LOOKUP__;

function __makeHtmlElement(tag) {
  var state = { _innerHTML: '', _qsaCount: 0, _cssText: '' };
  var styleObj = {};
  Object.defineProperty(styleObj, 'cssText', {
    get: function(){ return state._cssText; },
    set: function(v){ state._cssText = String(v||''); },
    enumerable: true, configurable: true
  });
  var el = {
    tagName: String(tag).toUpperCase(),
    nodeName: String(tag).toUpperCase(),
    nodeType: 1,
    children: [], childNodes: [], classList: [],
    style: styleObj, dataset: {},
    offsetWidth: 100, offsetHeight: 20, scrollHeight: 20,
    offsetTop: 100, offsetLeft: 0, offsetParent: null,
    clientWidth: 100, clientHeight: 20,
    getBoundingClientRect: function(){
      return { width: 100, height: 20, top: 100, left: 0, right: 100, bottom: 120, x: 0, y: 100 };
    },
    getClientRects: function(){
      return [{ width: 100, height: 20, top: 100, left: 0, right: 100, bottom: 120 }];
    },
    setAttribute: function(){}, removeAttribute: function(){},
    getAttribute: function(a){ if(a==='srcdoc') return state._srcdoc||''; return null; },
    hasAttribute: function(){ return false; },
    appendChild: function(c){ return c; },
    removeChild: function(c){ return c; },
    addEventListener: function(){}, removeEventListener: function(){},
    querySelector: function(){ return null; },
    querySelectorAll: function(s){
      if (s === '*') {
        var arr = []; arr.length = state._qsaCount; return arr;
      }
      return [];
    },
    cloneNode: function(){ return __makeHtmlElement(tag); },
    _getState: function(){ return state; }
  };
  Object.defineProperty(el, 'innerHTML', {
    get: function(){ return state._innerHTML; },
    set: function(v){
      var key = String(v);
      var entry = __HTML_LOOKUP && __HTML_LOOKUP[key];
      if (entry) { state._innerHTML = String(entry.html); state._qsaCount = entry.count|0; }
      else { state._innerHTML = key; state._qsaCount = 0; }
    },
    enumerable: true, configurable: true
  });
  Object.defineProperty(el, 'outerHTML', { get: function(){ return '<' + tag + '>' + state._innerHTML + '</' + tag + '>'; }, enumerable: true });
  Object.defineProperty(el, 'srcdoc', { get: function(){ return state._srcdoc||''; }, set: function(v){ state._srcdoc = String(v); }, enumerable: true });
  Object.defineProperty(el, 'contentWindow', { get: function(){
    var w = {};
    w.document = __ifDoc;
    w.Proxy = Proxy;
    w.self = w;
    w.top = w;
    w.parent = w;
    w.window = w;
    return w;
  }, enumerable: true });
  Object.defineProperty(el, 'contentDocument', { get: function(){ return __ifDoc; }, enumerable: true });
  return el;
}

function __mkObj(name, base) {
  base = base || {};
  return new Proxy(base, {
    get: function(t, k) {
      if (k in t) return t[k];
      if (k === Symbol.toPrimitive) return function(){ return ''; };
      if (k === Symbol.iterator) return undefined;
      if (k === 'then' || k === 'catch' || k === 'finally') return undefined;
      if (k === 'constructor') return Object;
      if (k === 'toString' || k === 'valueOf') return function(){ return '[object ' + name + ']'; };
      if (k === 'length') return 0;
      if (k === 'nodeType') return 1;
      if (k === 'tagName' || k === 'nodeName') return 'DIV';
      if (k === 'innerHTML' || k === 'outerHTML' || k === 'textContent' || k === 'innerText' || k === 'value') return '';
      if (k === 'children' || k === 'childNodes' || k === 'classList') return [];
      if (typeof k === 'string' && (k.indexOf('get') === 0 || k.indexOf('query') === 0 || k.indexOf('find') === 0)) {
        return function(arg){
          if (k === 'querySelectorAll' || k === 'getElementsByTagName' || k === 'getElementsByClassName') return [];
          return null;
        };
      }
      return function(){ return __mkObj(name + '.' + String(k)); };
    },
    has: function(t, k){ return k in t; },
    set: function(t, k, v){ t[k] = v; return true; }
  });
}

var __ifMeta = __mkObj('meta', {
  getAttribute: function(a){ return a==='content' ? "default-src 'none'; script-src 'unsafe-inline';" : null; },
  hasAttribute: function(a){ return a==='content'; },
  tagName: 'META', nodeName: 'META'
});
var __ifDoc;
__ifDoc = __mkObj('iframeDoc', {
  querySelector: function(s){
    if (s && s.indexOf('Content-Security-Policy') !== -1) return __ifMeta;
    if (s === 'meta') return __ifMeta;
    return null;
  },
  querySelectorAll: function(s){
    if (s && s.indexOf('Content-Security-Policy') !== -1) return [__ifMeta];
    if (s === 'meta') return [__ifMeta];
    return [];
  },
  getElementsByTagName: function(t){ return t && t.toLowerCase()==='meta' ? [__ifMeta] : []; },
  body: __mkObj('iframeBody', {
    querySelector: function(s){ return s && s.indexOf('Content-Security-Policy')!==-1 ? __ifMeta : null; },
    querySelectorAll: function(s){ return s && s.indexOf('Content-Security-Policy')!==-1 ? [__ifMeta] : []; },
    appendChild: function(){}, removeChild: function(){}
  }),
  head: __mkObj('iframeHead', {
    querySelector: function(s){ return s && s.indexOf('Content-Security-Policy')!==-1 ? __ifMeta : null; },
    querySelectorAll: function(s){ return s && s.indexOf('Content-Security-Policy')!==-1 ? [__ifMeta] : []; },
    appendChild: function(){}, removeChild: function(){}
  }),
  documentElement: __mkObj('iframeRoot'),
  createElement: function(){ return __mkObj('elem', {setAttribute:function(){}, appendChild:function(){}, removeChild:function(){}, getAttribute:function(){return null;}, hasAttribute:function(){return false;}}); },
  cookie: '', readyState: 'complete'
});

var __iframeEl = __mkObj('iframe', {
  contentDocument: __ifDoc,
  contentWindow: __mkObj('iframeWin', { document: __ifDoc, top: undefined, parent: undefined }),
  document: __ifDoc,
  getAttribute: function(a){
    if (a==='sandbox') return 'allow-scripts allow-same-origin';
    if (a==='srcdoc') return '';
    if (a==='id') return 'jsa';
    return null;
  },
  hasAttribute: function(a){ return a==='sandbox'||a==='id'; },
  tagName: 'IFRAME', nodeName: 'IFRAME', id: 'jsa'
});

var document = __mkObj('document', {
  querySelector: function(s){
    if (s === '#jsa') return __iframeEl;
    if (s && s.indexOf('Content-Security-Policy') !== -1) return __ifMeta;
    return null;
  },
  querySelectorAll: function(s){
    if (s === '#jsa') return [__iframeEl];
    if (s && s.indexOf('Content-Security-Policy') !== -1) return [__ifMeta];
    return [];
  },
  getElementById: function(id){ return id==='jsa' ? __iframeEl : null; },
  getElementsByTagName: function(t){ if(t&&t.toLowerCase()==='iframe') return [__iframeEl]; return []; },
  getElementsByClassName: function(){ return []; },
  body: __mkObj('body', {appendChild:function(){}, removeChild:function(){}, querySelector:function(s){return s==='#jsa'?__iframeEl:null;}, querySelectorAll:function(s){return s==='#jsa'?[__iframeEl]:[];}}),
  head: __mkObj('head', {appendChild:function(){}, removeChild:function(){}, querySelector:function(){return null;}, querySelectorAll:function(){return [];}}),
  documentElement: __mkObj('root'),
  createElement: function(tag){ return __makeHtmlElement(tag||'div'); },
  createTextNode: function(t){ return {nodeType:3, nodeValue:String(t||''), textContent:String(t||'')}; },
  cookie: '', readyState: 'complete', title: '',
  addEventListener: function(){}, removeEventListener: function(){}
});

var window;
window = __mkObj('window', {
  document: document,
  __DDG_BE_VERSION__: 1, __DDG_FE_CHAT_HASH__: 1,
  navigator: __mkObj('navigator', { userAgent: __ua, webdriver: false, language: 'en-US', languages: ['en-US','en'], platform: 'MacIntel', vendor: 'Apple Computer, Inc.', appVersion: '5.0', cookieEnabled: true, onLine: true, hardwareConcurrency: 8, deviceMemory: 8 }),
  innerWidth: 1280, innerHeight: 800, outerWidth: 1280, outerHeight: 800, devicePixelRatio: 1,
  screen: __mkObj('screen', { width:1920, height:1080, availWidth:1920, availHeight:1080, colorDepth:24, pixelDepth:24 }),
  location: __mkObj('location', { href:'https://duckduckgo.com/', origin:'https://duckduckgo.com', host:'duckduckgo.com', hostname:'duckduckgo.com', protocol:'https:', pathname:'/', search:'', hash:'', port:'' }),
  performance: __mkObj('perf', { now: function(){ return 0; }, timeOrigin: 0 }),
  history: __mkObj('history', { length: 1, state: null }),
  localStorage: __mkObj('ls', { getItem:function(){return null;}, setItem:function(){}, removeItem:function(){}, clear:function(){}, length:0, key:function(){return null;} }),
  sessionStorage: __mkObj('ss', { getItem:function(){return null;}, setItem:function(){}, removeItem:function(){}, clear:function(){}, length:0, key:function(){return null;} }),
  addEventListener: function(){}, removeEventListener: function(){}, dispatchEvent: function(){return true;},
  getComputedStyle: function(el){
    var css = (el && el.style && el.style.cssText) || '';
    return {
      getPropertyValue: function(p){
        var m = css.match(new RegExp(p + '\\s*:\\s*([^;]+)', 'i'));
        if (m) return m[1].trim();
        if (p === 'display') return 'block';
        return '';
      },
      display: (css.match(/display\s*:\s*([^;]+)/i)||[])[1]||'block'
    };
  },
  setTimeout: function(fn){ try{fn();}catch(e){} return 0; }, clearTimeout: function(){},
  setInterval: function(){ return 0; }, clearInterval: function(){},
  requestAnimationFrame: function(fn){ try{fn();}catch(e){} return 0; }, cancelAnimationFrame: function(){},
  matchMedia: function(){ return __mkObj('mq', {matches:false, media:'', addListener:function(){}, removeListener:function(){}, addEventListener:function(){}, removeEventListener:function(){}}); },
  hasOwnProperty: function(k){
    if (k==='__DDG_BE_VERSION__'||k==='__DDG_FE_CHAT_HASH__') return true;
    return Object.prototype.hasOwnProperty.call(this,k);
  },
  alert: function(){}, confirm: function(){return true;}, prompt: function(){return '';},
  open: function(){return null;}, close: function(){}, focus: function(){}, blur: function(){}
});
window.top = window; window.self = window; window.window = window; window.parent = window; window.globalThis = window;
var top = window, self = window, parent = window;
var navigator = window.navigator;
var location = window.location;
var screen = window.screen;
var performance = window.performance;
var history = window.history;
var localStorage = window.localStorage;
var sessionStorage = window.sessionStorage;
var getComputedStyle = function(el){ return window.getComputedStyle(el); };
var __R = null, __E = null;
function __HTMLClass(name){ var c = function(){}; c.prototype = __mkObj(name+'.proto'); return c; }
var HTMLElement = __HTMLClass('HTMLElement');
var HTMLDivElement = __HTMLClass('HTMLDivElement');
var HTMLIFrameElement = __HTMLClass('HTMLIFrameElement');
var HTMLDocument = __HTMLClass('HTMLDocument');
var Document = __HTMLClass('Document');
var Element = __HTMLClass('Element');
var Node = __HTMLClass('Node');
var Window = __HTMLClass('Window');
var Event = __HTMLClass('Event');
var MouseEvent = __HTMLClass('MouseEvent');
var KeyboardEvent = __HTMLClass('KeyboardEvent');
var TouchEvent = __HTMLClass('TouchEvent');
var XMLHttpRequest = __HTMLClass('XMLHttpRequest');
var WebSocket = __HTMLClass('WebSocket');
var Image = __HTMLClass('Image');
var FormData = __HTMLClass('FormData');
var Blob = __HTMLClass('Blob');
var File = __HTMLClass('File');
var FileReader = __HTMLClass('FileReader');
var URL = __HTMLClass('URL');
var URLSearchParams = __HTMLClass('URLSearchParams');
var Headers = __HTMLClass('Headers');
var Request = __HTMLClass('Request');
var Response = __HTMLClass('Response');
var fetch = function(){ return Promise.resolve(__mkObj('resp', {ok:true, status:200, json:function(){return Promise.resolve({});}, text:function(){return Promise.resolve('');}})); };
