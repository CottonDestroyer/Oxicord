// Injected into the temporary sign-in webview, top frame only.
//
// Discord's own page does the whole login: password, captcha, MFA, QR. Once
// the user is in, the page starts calling the API with an Authorization
// header. We watch same-origin API calls for it and report, once, the token
// together with the identity the page presents (user agent and the
// X-Super-Properties header), because a token only keeps working when it is
// used with the identity it was issued to. Nothing else leaves the page.
(() => {
  if (window !== window.top || location.origin !== "https://discord.com") return;

  const capability = "__OXICORD_CAPABILITY__";
  const startedAt = Date.now();
  const TEN_MINUTES = 10 * 60 * 1000;
  // How long to wait for a request that carries the identity as well, once
  // we already have a token.
  const IDENTITY_GRACE_MS = 5000;
  const WANTED = ["authorization", "x-super-properties", "x-discord-locale", "x-discord-timezone"];

  let sent = false;
  let pendingTimer = null;

  const isApiCall = (target) => {
    try {
      const url = new URL(target, location.href);
      return url.origin === "https://discord.com" && /^\/api\/v\d+\//.test(url.pathname);
    } catch {
      return false;
    }
  };

  const deliver = (headers) => {
    if (sent) return;
    sent = true;
    clearTimeout(pendingTimer);
    window.ipc.postMessage(
      capability +
        JSON.stringify({
          token: headers["authorization"],
          superProperties: headers["x-super-properties"] ?? null,
          userAgent: navigator.userAgent,
          locale: headers["x-discord-locale"] ?? null,
          timezone: headers["x-discord-timezone"] ?? null,
        })
    );
  };

  const offer = (headers) => {
    if (sent || Date.now() - startedAt > TEN_MINUTES) return;
    const token = headers["authorization"];
    if (typeof token !== "string" || token.length < 16 || token.length > 2048) return;
    if (/\s/.test(token)) return;

    if (headers["x-super-properties"]) {
      deliver(headers);
    } else if (pendingTimer === null) {
      // The first call had no identity. Give a later one a moment to show
      // it, then settle for the bare token.
      pendingTimer = setTimeout(() => deliver(headers), IDENTITY_GRACE_MS);
    }
  };

  const requests = new WeakMap();
  const nativeOpen = XMLHttpRequest.prototype.open;
  const nativeSetHeader = XMLHttpRequest.prototype.setRequestHeader;
  const nativeSend = XMLHttpRequest.prototype.send;

  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    requests.set(this, isApiCall(url) ? {} : null);
    return nativeOpen.call(this, method, url, ...rest);
  };
  XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
    const bag = requests.get(this);
    const key = String(name).toLowerCase();
    if (bag && WANTED.includes(key)) bag[key] = String(value);
    return nativeSetHeader.call(this, name, value);
  };
  // Headers are only complete once the request is sent.
  XMLHttpRequest.prototype.send = function (...args) {
    const bag = requests.get(this);
    if (bag) offer(bag);
    return nativeSend.apply(this, args);
  };

  const nativeFetch = window.fetch;
  window.fetch = function (input, init) {
    const isRequest = input instanceof Request;
    if (isApiCall(isRequest ? input.url : input)) {
      const source = new Headers(init?.headers ?? (isRequest ? input.headers : undefined));
      const bag = {};
      for (const key of WANTED) {
        const value = source.get(key);
        if (value) bag[key] = value;
      }
      offer(bag);
    }
    return nativeFetch.apply(this, arguments);
  };
})();
