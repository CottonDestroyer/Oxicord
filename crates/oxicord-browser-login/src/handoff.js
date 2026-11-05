// Injected into the temporary sign-in webview, top frame only.
//
// Discord's own page does the whole login: password, captcha, MFA, QR. Once
// the user is in, the page starts calling the API with an Authorization
// header. We watch for that header on same-origin API calls and pass its
// value to the app, once. Nothing else leaves the page.
(() => {
  if (window !== window.top || location.origin !== "https://discord.com") return;

  const capability = "__OXICORD_CAPABILITY__";
  const startedAt = Date.now();
  const TEN_MINUTES = 10 * 60 * 1000;
  let sent = false;

  const isApiCall = (target) => {
    try {
      const url = new URL(target, location.href);
      return url.origin === "https://discord.com" && /^\/api\/v\d+\//.test(url.pathname);
    } catch {
      return false;
    }
  };

  const offer = (value) => {
    if (sent || Date.now() - startedAt > TEN_MINUTES) return;
    if (typeof value !== "string" || value.length < 16 || value.length > 2048) return;
    if (/\s/.test(value)) return;
    sent = true;
    window.ipc.postMessage(capability + value);
  };

  const trackedRequests = new WeakMap();
  const nativeOpen = XMLHttpRequest.prototype.open;
  const nativeSetHeader = XMLHttpRequest.prototype.setRequestHeader;

  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    trackedRequests.set(this, isApiCall(url));
    return nativeOpen.call(this, method, url, ...rest);
  };
  XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
    if (trackedRequests.get(this) && String(name).toLowerCase() === "authorization") offer(value);
    return nativeSetHeader.call(this, name, value);
  };

  const nativeFetch = window.fetch;
  window.fetch = function (input, init) {
    const isRequest = input instanceof Request;
    if (isApiCall(isRequest ? input.url : input)) {
      const headers = new Headers(init?.headers ?? (isRequest ? input.headers : undefined));
      offer(headers.get("authorization"));
    }
    return nativeFetch.apply(this, arguments);
  };
})();
