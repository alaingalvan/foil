// ====================================================================================================================
// 🏃‍♂️ Make SystemJS global in node:
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
global.require = require;
import 'systemjs';
import 'systemjs/dist/extras/named-register.js';

System.addImportMap(importMap);

const { routes, client } = await System.import(frontendMain);
const { jsx } = await System.import('react/jsx-runtime');
const { createStaticHandler, createStaticRouter, StaticRouterProvider } =
  await System.import('react-router');
const { renderToPipeableStream } = await System.import('react-dom/server');
const { getDataFromTree } = await System.import('@apollo/client/react/ssr');

// ====================================================================================================================
import { createServer } from 'http';

// ====================================================================================================================
// 🐶 Create fetch Request based on node HTTP request.
function createFetchRequest(req) {
  let origin = `http://${req.headers['host']}`;
  let url = new URL('', origin);
  try {
    url = new URL(req.url, origin);
  } catch (e) {
    console.error('Failed to construct URL: %s', req.url);
  }

  let controller = new AbortController();
  req.on('close', () => controller.abort());
  let headers = new Headers();
  for (let [key, values] of Object.entries(req.headers)) {
    if (values) {
      if (Array.isArray(values)) {
        for (let value of values) {
          headers.append(key, value);
        }
      } else {
        headers.set(key, values);
      }
    }
  }
  let init = {
    method: req.method,
    headers,
    signal: controller.signal
  };
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    init.body = '{}';
  }
  return new Request(url.href, init);
}
// ====================================================================================================================
// 🔥 Hot Reloading
const CLIENT_RELOAD_PATH = '/__foil/reload';
const clients = new Set();

function liveReload(request, response) {
  response.setHeader('Content-Type', 'text/event-stream');
  response.setHeader('Cache-Control', 'no-cache');
  response.setHeader('Connection', 'keep-alive');
  response.flushHeaders();

  clients.add(response);
  request.on('close', () => clients.delete(response));
}

// ====================================================================================================================
// 🐛 Browser error reports never reach node, so the page reports them here with a plain GET the backend already proxies.
const CLIENT_ERROR_PATH = '/__foil/client-error';

function logClientError(request, response) {
  const params = new URL(request.url, 'http://localhost').searchParams;
  // Caps size and strips control characters so a stranger can't flood or forge log lines.
  const clean = (value, max) =>
    String(value ?? '')
      .slice(0, max)
      .replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, '');
  const kind = clean(params.get('kind'), 20);
  console.error(
    'Foil Renderer Error: %s in %s\n%s',
    kind.charAt(0).toUpperCase() + kind.slice(1),
    clean(params.get('url'), 200),
    clean(params.get('message'), 4000)
  );
  response.writeHead(204);
  response.end();
}

// Without this, a failed render leaves the request hanging and logs nothing useful.
function reportError(request, error, response) {
  console.error(
    'Foil Renderer Error: Server render in %s\n%s',
    request.url,
    error?.stack ?? error
  );
  if (!response.headersSent) {
    response.writeHead(500, { 'content-type': 'text/plain' });
  }
  response.end('Internal Server Error');
}

// ====================================================================================================================
// 🎨 Handle all server-side rendering of this foil application.
const foilRequestHandler = async (request, response) => {
  try {
    if (request.url?.startsWith(CLIENT_RELOAD_PATH)) {
      return liveReload(request, response);
    }
    if (request.url?.startsWith(CLIENT_ERROR_PATH)) {
      return logClientError(request, response);
    }
    let { query, dataRoutes } = createStaticHandler(routes);
    let fetchRequest = createFetchRequest(request);
    let context = await query(fetchRequest);
    if (context instanceof Response) {
      for (let [headerKey, headerValue] of context.headers) {
        response.setHeader(headerKey, headerValue);
      }
      response.writeHead(context.status, context.statusText);
      return response.end(context.body);
    }
    let router = createStaticRouter(dataRoutes, context);
    const node = jsx(StaticRouterProvider, { router, context });
    await getDataFromTree(node);
    const bootstrapScriptContent = `
  window.__APOLLO_STATE__ = ${JSON.stringify(client.extract())};
  // Runtime Error Tracking
  function foilReport(kind, error, info) {
    try {
      var text = (error && error.stack) || String(error);
      if (info && info.componentStack)
        text += '\\nComponent stack:' + info.componentStack;
      fetch(
        '${CLIENT_ERROR_PATH}?kind=' +
          encodeURIComponent(kind) +
          '&url=' +
          encodeURIComponent(location.pathname) +
          '&message=' +
          encodeURIComponent(text.slice(0, 4000)),
        { keepalive: true },
      ).catch(function () {});
    } catch (e) {}
  }
  window.addEventListener('error', function (e) {
    foilReport('window', e.error || e.message);
  });
  window.addEventListener('unhandledrejection', function (e) {
    foilReport('promise', e.reason);
  });

  // Hot Reloading
  const connect = () => {
    const source = new EventSource('${CLIENT_RELOAD_PATH}')

    source.onerror = () => {
      source.close()
      const poll = setInterval(async () => {
        try {
          if ((await fetch('${CLIENT_RELOAD_PATH}')).ok) {
            clearInterval(poll)
            window.location.reload()
          }
        } catch {}
      }, 500)
    }
  }
  connect();

  async function main() {
    const jsxRuntime = await System.import('react/jsx-runtime');
    const reactDom = await System.import('react-dom/client');
    const reactRouter = await System.import('react-router');
    const app = await System.import('${frontendMain}');
    const node = jsxRuntime.jsx(reactRouter.RouterProvider, {
      router: reactRouter.createBrowserRouter(app.routes),
    });
    reactDom.hydrateRoot(document, node, {
      onCaughtError: function (error, info) {
        console.error(error);
        foilReport('caught', error, info);
      },
      onUncaughtError: function (error, info) {
        console.error(error);
        foilReport('uncaught', error, info);
      },
      onRecoverableError: function (error, info) {
        var message = String((error && error.message) || error);
        if (message.indexOf('synchronously rendering the entire root') !== -1) return;
        console.error(error);
        foilReport('recoverable', error, info);
      },
    });
  }
  main();
`;
    const { pipe } = renderToPipeableStream(node, {
      bootstrapScriptContent,
      onShellReady() {
        response.setHeader('content-type', 'text/html');
        pipe(response);
      },
      onShellError(error) {
        reportError(request, error, response);
      },
      onError(error) {
        console.error(
          'Foil Renderer Error: Server render in %s\n%s',
          request.url,
          error?.stack ?? error
        );
      }
    });
  } catch (error) {
    reportError(request, error, response);
  }
};

// ====================================================================================================================
const server = createServer(foilRequestHandler);
server.listen(4011, '127.0.0.1', () => {
  console.log('🌠 Foil Renderer running in http://127.0.0.1:4011');
});
