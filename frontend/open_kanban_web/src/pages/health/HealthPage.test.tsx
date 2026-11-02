import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { HealthPage } from "./HealthPage";

function jsonResponse(body: unknown, status = 200): Response {
  const text = JSON.stringify(body);

  return {
    ok: status >= 200 && status < 300,
    status,
    headers: {
      get: (name: string) => (name.toLowerCase() === "content-type" ? "application/json" : null),
    },
    json: async () => body,
    text: async () => text,
  } as unknown as Response;
}

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });

  return render(
    <QueryClientProvider client={client}>
      <HealthPage />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("HealthPage", () => {
  it("renders service and readiness states", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) => {
        const url =
          typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
        if (url.includes("/healthz")) {
          return jsonResponse({ status: "ok" });
        }
        if (url.includes("/readyz")) {
          return jsonResponse({ status: "ready" });
        }
        return jsonResponse({ title: "Not found" }, 404);
      }),
    );

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("status-health").textContent).toBe("ok");
    });
    await waitFor(() => {
      expect(screen.getByTestId("status-ready").textContent).toBe("ready");
    });
  });

  it("reports an unavailable service", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => jsonResponse({ title: "Service unavailable" }, 503)),
    );

    renderPage();

    await waitFor(() => {
      expect(screen.getByTestId("status-health").textContent).toBe("недоступен");
    });
  });
});
