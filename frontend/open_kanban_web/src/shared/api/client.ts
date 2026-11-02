import createClient from "openapi-fetch";

import type { paths } from "./schema";

/**
 * Typed client generated from the service OpenAPI contract (see tools/gen-api.sh).
 * Absolute origin keeps `fetch` happy both in the browser and in jsdom tests.
 */
export const api = createClient<paths>({
  baseUrl: window.location.origin,
  // Resolve fetch at request time so tests can stub it after module import.
  fetch: (request) => globalThis.fetch(request),
});
