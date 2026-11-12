// @vitest-environment jsdom
// The app's real route table, with a new and an existing record for each entity that has its own screen and for ones that do not
// (the records frame's generic form): the entity must reach the page from the address, and no page may be left on "Loading…" once
// its data has come (the walk found CRM's form waiting forever because its route carried no entity).

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { routes } from "./routes";
import { fakeServer, until } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const meta = (name: string) => ({ name, title_field: "title", form: [], children: {}, fields: [{ name: "title", type: "Data", label: "Title" }], permissions: { read: "all", write: "all", create: "all" } });
const every = ["Incident", "ItChange", "Problem", "ConfigurationItem", "CatalogRequest", "ServiceOffering", "ChangeType"];
const server = () =>
  fakeServer([
    ["GET", "/api/auth/me", { tenant: "t", user: { id: "u1", email: "ana@x.test", full_name: "Ana", is_admin: true, roles: [] } }],
    ...every.map((e): [string, string, unknown] => ["GET", `/api/meta/${e}`, meta(e)]),
    ["GET", "/api/meta", { entities: every.map((e) => ({ name: e, permissions: { read: "all", write: "all", create: "all" } })) }],
    ["GET", "/api/v1/", (u: URL) => (u.pathname.endsWith("/approval") ? { id: "ap", status: "approved", requester: "u9", requested_at: "2026-12-01T09:00:00Z", steps: [], decisions: [] } : u.pathname.split("/").length > 4 ? { id: u.pathname.split("/").pop(), version: 1, title: "x", status: "Draft" } : { rows: [] })],
  ]);

async function open(path: string) {
  const host = document.createElement("div");
  document.body.append(host);
  const router = createMemoryRouter([{ path: "/", element: <Outlet />, children: routes }], { initialEntries: [path] });
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  await act(async () => {
    createRoot(host).render(
      <QueryClientProvider client={qc}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
  });
  // Waits for the page to leave its loading state; a page that never does fails here.
  await until(() => !host.querySelector('[aria-busy="true"]') && !!host.textContent);
  return host;
}

describe("the route table hands every record page its entity", () => {
  const generic = ["Incident", "ItChange", "Problem", "ConfigurationItem", "CatalogRequest", "ServiceOffering", "ChangeType"];
  for (const entity of generic) {
    for (const id of ["new", "r1"]) {
      it(`/e/${entity}/${id} reaches the generic form with its entity and leaves loading`, async () => {
        const fake = server();
        const host = await open(`/e/${entity}/${id}`);
        expect(fake.seen.some((s) => s.line.startsWith(`GET /api/meta/${entity}`))).toBe(true);
        expect(host.querySelector('[aria-busy="true"]')).toBeNull();
        expect(host.textContent).not.toContain("Something went wrong");
      });
    }
  }

  for (const path of ["/change/new", "/change/r1", "/problem/r1", "/ci/r1", "/request/r1", "/catalog/r1", "/major/r1"]) {
    it(`${path}, its own screen, leaves loading`, async () => {
      server();
      const host = await open(path);
      expect(host.querySelector('[aria-busy="true"]')).toBeNull();
      expect(host.textContent).not.toContain("Something went wrong");
    });
  }
});
