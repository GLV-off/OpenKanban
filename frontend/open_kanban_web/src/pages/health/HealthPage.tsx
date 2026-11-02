import type { UseQueryResult } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";

import { api } from "../../shared/api/client";

async function fetchHealth(): Promise<string> {
  const { data, response } = await api.GET("/healthz");
  if (!response.ok || !data) {
    throw new Error(`healthz responded with HTTP ${response.status}`);
  }
  return data.status;
}

async function fetchReadiness(): Promise<string> {
  const { data, response } = await api.GET("/readyz");
  if (!response.ok || !data) {
    throw new Error(`readyz responded with HTTP ${response.status}`);
  }
  return data.status;
}

export function HealthPage() {
  const health = useQuery({
    queryKey: ["system", "healthz"],
    queryFn: fetchHealth,
    refetchInterval: 15_000,
    retry: false,
  });
  const readiness = useQuery({
    queryKey: ["system", "readyz"],
    queryFn: fetchReadiness,
    refetchInterval: 15_000,
    retry: false,
  });

  return (
    <section className="card">
      <h1>Состояние сервиса</h1>
      <p className="muted">
        Каркас клиента: страница проверяет связь с API через клиент, сгенерированный из OpenAPI.
      </p>

      <dl className="status-list">
        <StatusRow id="health" label="Процесс (/healthz)" query={health} />
        <StatusRow id="ready" label="Готовность (/readyz)" query={readiness} />
      </dl>

      <div className="actions">
        <a href="/api/v1/docs" target="_blank" rel="noreferrer">
          Swagger UI
        </a>
        <a href="/api/v1/openapi.json" target="_blank" rel="noreferrer">
          OpenAPI JSON
        </a>
      </div>
    </section>
  );
}

type StatusRowProps = {
  id: string;
  label: string;
  query: UseQueryResult<string, Error>;
};

function StatusRow({ id, label, query }: StatusRowProps) {
  const state = query.isPending ? "idle" : query.isError ? "bad" : "good";
  const value = query.isPending ? "…" : query.isError ? "недоступен" : query.data;

  return (
    <div className="status-row">
      <dt>{label}</dt>
      <dd
        className={`pill pill-${state}`}
        data-testid={`status-${id}`}
        title={query.error?.message}
      >
        {value}
      </dd>
    </div>
  );
}
