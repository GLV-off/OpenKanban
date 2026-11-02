# GLV Open Kanban

Открытая self-hosted реализация сервиса и веб-клиента для управления задачами
в стиле Kanban: рабочее пространство, доски, колонки, задачи и базовый набор
пользователей с ролями.

Статус: **итерация 0 завершена**, идёт итерация 1. Бекенд-часть S1
(аутентификация) готова: регистрация, вход, refresh с ротацией, выход,
`/me`, троттлинг попыток входа. Следующий шаг — фронтенд-часть S1.

## Что планируется

- Регистрация по email и паролю, сессии с обновлением токена.
- Рабочее пространство с одним владельцем, администраторами и участниками.
- Доски и колонки с настраиваемым порядком.
- Задачи с заголовком и описанием; перетаскивание между колонками и перенос
  между досками с сохранением идентификатора задачи.
- Видимость пользователей только в пределах общих пространств.
- Первая итерация — SQLite на бекенде, Rust + axum + sqlx; клиент —
  TypeScript + React + Vite.
- Открытый контракт API (OpenAPI 3.1), по которому можно написать любой клиент.

## Документация

| Документ | О чём |
|---|---|
| [doc/goals_ru.md](doc/goals_ru.md) | Цели, границы, требования, критерии приёмки |
| [doc/domain_ru.md](doc/domain_ru.md) | Модель данных, инварианты, роли, перенос задач |
| [doc/stack_ru.md](doc/stack_ru.md) | Стек, версии, обоснование, риски |
| [doc/roadmap_ru.md](doc/roadmap_ru.md) | Итерации и Definition of Done |
| [AGENTS.md](AGENTS.md) | Правила разработки в репозитории |

## Структура

```
backend/open_kanban_service/   # HTTP API (Rust + axum + sqlx + SQLite)
frontend/open_kanban_web/      # SPA (React + TypeScript + Vite)
doc/                           # документация
tools/                         # gen-api.sh, smoke.sh
```

## Бекенд

Требования: Rust 1.85+ (проверено на 1.96).

```bash
cargo run -p open_kanban_service    # запуск API на 127.0.0.1:8080
cargo test                          # unit + интеграционные тесты
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Проверки запущенного сервиса:

```bash
curl -s localhost:8080/healthz                  # {"status":"ok"}
curl -s localhost:8080/readyz                   # {"status":"ready"} (проверяет БД)
curl -s localhost:8080/api/v1/openapi.json      # OpenAPI 3.1
open http://localhost:8080/api/v1/docs          # Swagger UI (кроме production)
```

### Аутентификация

| Метод | Endpoint | Назначение |
|---|---|---|
| POST | `/api/v1/auth/register` | регистрация; сразу создаёт сессию (201) |
| POST | `/api/v1/auth/login` | вход по email и паролю |
| POST | `/api/v1/auth/refresh` | ротация refresh-токена, новый access |
| POST | `/api/v1/auth/logout` | отзыв текущей сессии (204) |
| GET | `/api/v1/auth/me` | текущий пользователь (Bearer) |

- Access-токен — JWT (HS256), передаётся как `Authorization: Bearer`,
  живёт `OK_ACCESS_TTL_SECONDS` (по умолчанию 15 минут).
- Refresh-токен — только в cookie `ok_refresh` (`HttpOnly`, `SameSite=Strict`,
  `Path=/api/v1/auth`, `Secure` в production), живёт
  `OK_REFRESH_TTL_SECONDS` (по умолчанию 30 дней).
- Refresh ротируется: повторное использование старого токена отзывает все
  активные сессии пользователя.
- Вход ограничен `OK_LOGIN_MAX_ATTEMPTS` неудачами на email за 15 минут.
- Cookie-эндпоинты принимают только `Origin` из `OK_ALLOWED_ORIGINS`
  (если заголовок присутствует).

```bash
curl -s -X POST localhost:8080/api/v1/auth/register \
  -H 'content-type: application/json' \
  -H 'origin: http://localhost:5173' \
  -d '{"email":"demo@example.com","password":"correct horse battery","display_name":"Demo"}'
```

### Конфигурация

Переменные окружения (можно положить в `.env`, шаблон — `.env.example`;
переменные процесса имеют приоритет):

| Переменная | По умолчанию | Назначение |
|---|---|---|
| `OK_BIND` | `127.0.0.1:8080` | адрес и порт HTTP-сервера |
| `OK_DATABASE_URL` | `sqlite://open_kanban.db` | URL SQLite; файл создаётся при старте |
| `OK_LOG` | `info,open_kanban_service=debug` | фильтр `tracing` (EnvFilter) |
| `OK_ENV` | `development` | `development` / `production`; production включает JSON-логи и отключает Swagger UI |
| `OK_JWT_SECRET` | dev-секрет (только для разработки) | секрет подписи JWT, ≥ 32 символов; обязателен в production |
| `OK_ACCESS_TTL_SECONDS` | `900` | время жизни access-токена |
| `OK_REFRESH_TTL_SECONDS` | `2592000` | время жизни refresh-сессии (30 дней) |
| `OK_ALLOWED_ORIGINS` | `http://localhost:5173,http://127.0.0.1:5173` | разрешённые `Origin` для cookie-эндпоинтов |
| `OK_COOKIE_SECURE` | `true` в production, иначе `false` | флаг `Secure` у refresh-cookie |
| `OK_LOGIN_MAX_ATTEMPTS` | `10` | неудачных попыток входа на email за 15 минут |

БД создаётся автоматически, миграции применяются при старте. Файлы `*.db`,
`.env`, `target/`, `node_modules/`, `dist/` исключены из git.

## Фронтенд

Требования: Node.js 24+ (проверено на 24.18). TypeScript зафиксирован на 5.9:
`openapi-typescript` 7.13 пока требует peer `^5.x`.

```bash
cd frontend/open_kanban_web
npm ci
npm run dev          # http://localhost:5173
```

Dev-сервер проксирует `/healthz`, `/readyz` и `/api` на `127.0.0.1:8080`,
поэтому страница состояния работает при запущенном бекенде.

```bash
npm run lint         # biome check .
npm run typecheck
npm run test         # vitest run
npm run build        # typecheck + production build
```

## Инструменты

```bash
tools/smoke.sh                 # проверка /healthz, /readyz, /api/v1/openapi.json
tools/gen-api.sh               # OpenAPI → src/shared/api/schema.d.ts (нужен запущенный API)
```

Сгенерированные типы коммитятся: сборка фронтенда не требует запущенного
сервиса. После изменения API-контракта: `tools/gen-api.sh` и коммит
`schema.d.ts` вместе с изменением бекенда.
