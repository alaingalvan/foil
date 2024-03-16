// ====================================================================================================================
// 🏃‍♂️ Make SystemJS global in node:
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
global.require = require;
import "systemjs";
import "systemjs/dist/extras/named-register.js";

System.addImportMap(importMap);

const { routes, client } = await System.import(frontendMain);
const { jsx } = await System.import("react/jsx-runtime");
const { createStaticHandler, createStaticRouter, StaticRouterProvider } = await System.import("react-router-dom/server");
const { renderToPipeableStream } = await System.import("react-dom/server");
const { getDataFromTree } = await System.import("@apollo/client/react/ssr");

// ====================================================================================================================
import { createServer } from "http";

// ====================================================================================================================
// 🐶 Create fetch Request based on node HTTP request.
function createFetchRequest(req) {
    let origin = `http://${req.headers["host"]}`;
    let url = new URL("", origin);
    try {
        url = new URL(req.url, origin)
    }
    catch (e) {
        console.error("Failed to construct URL: %s", req.url)
    }

    let controller = new AbortController();
    req.on("close", () => controller.abort());
    let headers = new Headers();
    for (let [key, values] of Object.entries(req.headers)) {
        if (values) {
            if (Array.isArray(values)) {
                for (let value of values) {
                    headers.append(key, value);
                }
            }
            else {
                headers.set(key, values);
            }
        }
    }
    let init = {
        method: req.method,
        headers,
        signal: controller.signal,
    };
    if (req.method !== "GET" && req.method !== "HEAD") {
        init.body = "{}";
    }
    return new Request(url.href, init);
}
// ====================================================================================================================
// 🎨 Handle all server-side rendering of this foil application.
const foilRequestHandler = async (request, response) => {
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
  async function main() {
    const jsxRuntime = await System.import('react/jsx-runtime');
    const reactDom = await System.import('react-dom/client');
    const reactRouterDom = await System.import('react-router-dom');
    const app = await System.import('${frontendMain}');
    const node = jsxRuntime.jsx(reactRouterDom.RouterProvider, { router: reactRouterDom.createBrowserRouter(app.routes) });
    reactDom.hydrateRoot(document, node);
  }
  main();
`;
    const { pipe } = renderToPipeableStream(node, {
        bootstrapScriptContent,
        onShellReady() {
            response.setHeader("content-type", "text/html");
            pipe(response);
        },
    });
};

// ====================================================================================================================
const server = createServer(foilRequestHandler);
server.listen(4011, "127.0.0.1", () => { });

