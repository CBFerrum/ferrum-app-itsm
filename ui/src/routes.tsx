// The routes the app adds under the signed-in frame: its own screens, then the records frame's pages. Every records page takes
// its entity (and id) from the address, so a new or an existing record reaches the generic form with its entity (routes.test.tsx).

import type { RouteObject } from "react-router";

const page = (name: "FormView" | "ListView" | "ReportView" | "AppSettingsPage" | "ScreenPage") => () => import("./records").then((m) => ({ Component: m[name] }));

export const routes: RouteObject[] = [
  { index: true, lazy: () => import("./Home").then((m) => ({ Component: m.Home })) },
  { path: "change/:id", lazy: () => import("./Change").then((m) => ({ Component: m.ChangeRoute })) },
  { path: "calendar", lazy: () => import("./Calendar").then((m) => ({ Component: m.CalendarRoute })) },
  { path: "problems", lazy: () => import("./Problem").then((m) => ({ Component: m.ProblemsRoute })) },
  { path: "problem/:id", lazy: () => import("./Problem").then((m) => ({ Component: m.ProblemRoute })) },
  { path: "major/:id", lazy: () => import("./Major").then((m) => ({ Component: m.MajorRoute })) },
  { path: "catalog", lazy: () => import("./Catalog").then((m) => ({ Component: m.CatalogRoute })) },
  { path: "catalog/:id", lazy: () => import("./Catalog").then((m) => ({ Component: m.OfferingRoute })) },
  { path: "request/:id", lazy: () => import("./Catalog").then((m) => ({ Component: m.RequestRoute })) },
  { path: "items", lazy: () => import("./Items").then((m) => ({ Component: m.ItemsRoute })) },
  { path: "ci/:id", lazy: () => import("./Items").then((m) => ({ Component: m.ItemRoute })) },
  { path: "e/:entity", lazy: page("ListView") },
  { path: "e/:entity/:id", lazy: page("FormView") },
  { path: "r/:name", lazy: page("ReportView") },
  { path: "apps/:app/settings", lazy: page("AppSettingsPage") },
  { path: "x/:app/:page", lazy: page("ScreenPage") },
];
