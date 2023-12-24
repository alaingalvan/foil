import { createServer } from "http";
import { renderToPipeableStream } from "react-dom/server";
import { StaticRouter } from "react-router-dom/server";
type RouterAppComponent = React.FunctionComponent<{
  router: React.FunctionComponent;
}>;

const foilRequestHandler =
  (ReactApp: RouterAppComponent) => (request, response) => {
    const ServerRouter = ({ children }) => (
      <StaticRouter location={request.url}>{children}</StaticRouter>
    );
    const { pipe } = renderToPipeableStream(
      <ReactApp router={ServerRouter} />,
      {
        bootstrapScriptContent: "System.import('/assets/build/main.js');",
        onShellReady() {
          response.setHeader("content-type", "text/html");
          pipe(response);
        },
      }
    );
  };

const server = createServer(foilRequestHandler(Default));
server.listen(4011, "127.0.0.1", () => {});
