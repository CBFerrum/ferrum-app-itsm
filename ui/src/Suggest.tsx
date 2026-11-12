// Known errors that fit what someone is typing (GET /api/app/itsm/suggest?q=, src/discovery.rs): the five best matches, with the
// workaround each one has. Beside an incident's title, the catalog's search and a request's free text, so the answer is
// there before the question is sent. A portal person is shown only those a problem has published.

import { useQuery } from "@tanstack/react-query";
import { api, isolate, refusal, t } from "@ferrum/client";
import { Alert, useDebounced } from "@ferrum/kit";

export interface Suggestion {
  id: string;
  title: string;
  workaround: string | null;
}

/** How little is worth asking about: a word is not enough to match a known error. */
export const MIN_TYPED = 3;

export const askSuggest = (q: string, signal?: AbortSignal) => api.get<{ suggestions: Suggestion[] }>("/api/app/itsm/suggest", { q: q.trim() }, signal).then((r) => r.suggestions);

export function Suggest({ typed }: { typed: string }) {
  const q = useDebounced(typed.trim(), 300);
  const found = useQuery({ queryKey: ["itsm", "suggest", q], queryFn: ({ signal }) => askSuggest(q, signal), enabled: q.length >= MIN_TYPED });
  if (q.length < MIN_TYPED) return null;
  if (found.isError) return <p className="muted">{t("Known problems could not be searched: {why}", { why: isolate(refusal(found.error)) })}</p>;
  const list = found.data ?? [];
  if (!list.length) return null;
  return (
    <Alert tone="info">
      <p><strong>{t("Known problems that may be this")}</strong></p>
      <ul className="stack">
        {list.map((s) => (
          <li key={s.id}>
            <strong>{isolate(s.title)}</strong>
            {s.workaround && <p className="muted">{t("Workaround: {text}", { text: isolate(s.workaround) })}</p>}
          </li>
        ))}
      </ul>
    </Alert>
  );
}
