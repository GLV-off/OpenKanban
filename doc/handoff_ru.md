# Передача работы в следующую сессию

| Поле | Значение |
|---|---|
| Дата | 2026-11-02 |
| Репозиторий | `git@github.com:GLV-off/OpenKanban.git`, ветка `main` |
| Статус | Итерация 0 завершена; S1: бекенд готов, фронтенд — следующий шаг |
| Файл | Временная рабочая записка. Удалить или обновить после завершения S1 |

## 1. Цель следующей сессии

Довести срез **S1 «Аутентификация»** до конца: фронтенд-часть (формы входа и
регистрации, access-токен в памяти, автопродление сессии, guard маршрутов,
выход), тесты, обновление документации. Если останется время — начать **S2**
(рабочие пространства, участники, роли).

## 2. Состояние на момент передачи

- **I0 завершена:** бекенд-скелет, SQLite + миграция `0001`, OpenAPI + Swagger UI
  (вне production), фронтенд-скелет, `tools/gen-api.sh`, `tools/smoke.sh`, CI
  (backend + frontend jobs).
- **S1 бекенд готов:** регистрация, вход, refresh с ротацией и отзывом цепочки,
  выход, `/me`, argon2id, JWT HS256, троттлинг входа, allowlist `Origin`,
  `problem+json` для всех ошибок.
- **Проверки зелёные:** бекенд — 43 теста (21 unit + 8 API + 12 auth + 2 db),
  `cargo fmt`, `clippy -D warnings`; фронтенд — `biome check`, `tsc --noEmit`,
  `vitest` (2 теста), `vite build`.
- **Контракт перегенерирован:** `frontend/open_kanban_web/src/shared/api/schema.d.ts`
  (есть пути `/api/v1/auth/*`, security schemes `bearer_auth`, `refresh_cookie`).
- **Изменения не закоммичены.** Рабочее дерево содержит I0 + S1 бекенд поверх
  коммита `f09802b`.

Предлагаемые коммиты перед новой сессией:

```
feat(backend): add service skeleton with health, sqlite and openapi
feat(frontend): add vite react skeleton with typed api client
chore(ci): add backend and frontend pipelines
feat(auth): add registration, login and rotating refresh sessions
docs: record accepted decisions and auth contract
```

## 3. Принятые решения — не пересматривать

- **Q1–Q7 подтверждены заказчиком:** UUIDv7 для `id`; видимость пользователей
  только через общие пространства; доски/колонки ведут owner/admin, `member` —
  только задачи; перенос задач между пространствами запрещён; email-подтверждение
  отложено до I2; модульный монолит; refresh в httpOnly-cookie, access в памяти.
- **Токены:** access JWT HS256, TTL 15 мин, stateless; refresh 30 дней, в БД только
  SHA-256-хеш, ротация при каждом refresh; повторное использование старого
  refresh отзывает все сессии пользователя.
- **Cookie:** `ok_refresh`, `HttpOnly`, `SameSite=Strict`, `Path=/api/v1/auth`,
  `Secure` при `OK_ENV=production`.
- **Ошибки:** `application/problem+json` с `type/title/status/code/detail/errors`.
- **OpenAPI-first:** типы фронтенда генерируются, руками не правятся.
- **TypeScript 5.9.3**, не 7.x: `openapi-typescript@7.13` требует peer `^5.x`.
- **Локальное состояние форм** — React state; `react-hook-form`/`zod` пока не
  установлены и для двух форм не нужны (YAGNI).
- секрет JWT обязателен в production (`OK_JWT_SECRET`, ≥32 символов); в dev —
  небезопасный секрет по умолчанию.

## 4. Порядок работ (фронтенд S1)

1. **API-слой** — `src/features/auth/api.ts`:
   - обёртки `register`, `login`, `refresh`, `logout`, `me` над `api` из
     `src/shared/api/client.ts`;
   - cookie отправляется только при `credentials: "include"` — добавить в
     `createClient` (или в fetch-обёртку) и проверить, что не ломается
     подмена `fetch` в тестах;
   - разбор `problem+json`: `code`, `detail`, `errors[{field,message}]`.
2. **Auth-состояние** — `src/features/auth/AuthProvider.tsx`:
   - статус `loading | anonymous | authenticated`, `user`, `accessToken`
     (только в памяти, не в localStorage);
   - bootstrap при монтировании: `POST /api/v1/auth/refresh`; `200` — сохранить
     access+user, `401` — anonymous; защита от двойного вызова в StrictMode;
   - `login`, `register`, `logout`, `refreshSession`.
3. **Авто-refresh** — хелпер `requestWithAuth`:
   - на `401` один раз вызвать refresh, повторить исходный запрос; single-flight,
     чтобы параллельные запросы не устраивали гонку refresh;
   - сам refresh не должен уходить в бесконечный цикл.
4. **Формы** — `/login` и `/register`:
   - валидация на клиенте (email, пароль ≥10, display_name 1–80);
   - показ серверных `errors[]` у соответствующих полей;
   - после успеха — редирект на `from` или `/`, ошибки — в общем блоке.
5. **Guard и шапка**:
   - `RequireAuth`: `loading` → заглушка, `anonymous` → `<Navigate to="/login" />`
     с сохранением `from`;
   - публичные `/login`, `/register` редиректят аутентифицированного на `/`;
   - в шапке — имя пользователя и кнопка выхода (вызывает `/logout`, чистит
     состояние).
6. **OpenAPI-клиент**: `schema.d.ts` не править; при изменении бекенда —
   `tools/gen-api.sh` при запущенном сервисе и коммит вместе с бекендом.
7. **Тесты (vitest + Testing Library)**:
   - успешный вход: форма → контекст authenticated, показан пользователь;
   - неверный пароль: сообщение из `problem+json`, состояние anonymous;
   - guard: анонимный пользователь редиректится на `/login`;
   - bootstrap: `refresh 200` восстанавливает сессию, `401` — нет;
   - `401` на защищённом запросе → один refresh → повтор запроса;
   - выход очищает состояние и вызывает `/logout`.
8. **Документация**: отметить S1 завершённым в `doc/roadmap_ru.md`, обновить
   README (если появились экраны/поведение), удалить этот файл.

## 5. Контракт API (для фронтенда)

| Метод | Путь | Тело | Успех |
|---|---|---|---|
| POST | `/api/v1/auth/register` | `{email,password,display_name}` | `201` + `AuthResponse`, Set-Cookie |
| POST | `/api/v1/auth/login` | `{email,password}` | `200` + `AuthResponse`, Set-Cookie |
| POST | `/api/v1/auth/refresh` | — (cookie) | `200` + `AuthResponse`, новая cookie |
| POST | `/api/v1/auth/logout` | — (cookie) | `204`, cookie очищается |
| GET | `/api/v1/auth/me` | — (`Bearer`) | `200` + `PublicUser` |

`AuthResponse`: `{access_token, token_type:"Bearer", expires_in, user}`.

`PublicUser`: `{id, email, display_name, email_verified, created_at}`.

Коды ошибок для UI: `INVALID_CREDENTIALS` (401), `EMAIL_TAKEN` (409),
`VALIDATION_ERROR` (422, поле `errors[]`), `TOO_MANY_REQUESTS` (429, заголовок
`Retry-After`), `MISSING_TOKEN`/`INVALID_TOKEN` (401), `ORIGIN_NOT_ALLOWED` (403).

## 6. Ключевые файлы

Бекенд (`backend/open_kanban_service/`):

- `src/main.rs`, `src/app.rs`, `src/config.rs`, `src/error.rs`, `src/state.rs`;
- `src/auth/{routes,service,token,password,throttle,extract}.rs`;
- `src/users/mod.rs`, `src/clock.rs`, `src/db/mod.rs`;
- `migrations/0001_users_sessions.sql`;
- `tests/{api,auth,db}.rs`, `tests/support/mod.rs`.

Фронтенд (`frontend/open_kanban_web/`):

- `src/shared/api/{client.ts,schema.d.ts}`;
- `src/app/{App.tsx,query-client.ts,styles.css}`;
- `src/pages/{health/HealthPage.tsx,login/LoginPage.tsx}`;
- `src/main.tsx`, `vite.config.ts`, `vitest.config.ts`, `biome.json`.

Инструменты и процесс: `tools/gen-api.sh`, `tools/smoke.sh`,
`.github/workflows/ci.yml`, `doc/*.md`, `AGENTS.md`.

## 7. Грабли (проверено на практике)

- **`sessions.replaced_by` — FK на `sessions(id)`.** При refresh новый ряд
  вставляется до `UPDATE ... replaced_by`, иначе SQLite даёт нарушение FK и
  `500`. Не менять порядок.
- **openapi-fetch захватывает `fetch` при создании клиента.** В `client.ts`
  поэтому используется обёртка `fetch: (request) => globalThis.fetch(request)` —
  на ней держатся тесты. Не заменять на прямой `fetch`.
- **Относительные URL падают в jsdom/Node** («Failed to parse URL»), поэтому
  `baseUrl: window.location.origin`. Не переходить на `"/"`.
- **RTL auto-cleanup не работает при `globals: false`** — в `afterEach` явно
  вызывать `cleanup()` (уже сделано в `HealthPage.test.tsx`).
- **Refresh-cookie `HttpOnly`** — из JS не читается; восстановление сессии только
  сетевым вызовом, не по наличию cookie.
- **`Origin` обязателен и проверяется, если присутствует.** Dev-прокси Vite
  отдаёт `Origin: http://localhost:5173` — он в дефолтном allowlist. При другом
  порту/хосте добавить в `OK_ALLOWED_ORIGINS`.
- **Vite 8 слушает `localhost` (IPv6 `::1`)** — проверять `curl localhost:5173`,
  а не `127.0.0.1`.
- **utoipa 6:** security schemes добавляются программно в `openapi_document()`
  (`app.rs`); derive-атрибута `security_schemes` в этой версии нет.
- **`sqlx::migrate!`** вшивает `migrations/` на этапе компиляции: новый SQL-файл
  требует пересборки.
- **argon2:** API `password-hash` 0.6 — `hash_password(&[u8])` сам генерирует
  соль, `verify_password(&[u8], &str)`; старые примеры с `SaltString` не подходят.
- **`rand` 0.10:** трейт `Rng` (не `RngCore`) даёт `fill_bytes`.
- **jsonwebtoken 11:** для HS256 нужен feature `rust_crypto`
  (`default-features = false`).

## 8. Команды запуска и проверки

```bash
# из корня репозитория
cp .env.example .env              # при необходимости
cargo run -p open_kanban_service  # API на 127.0.0.1:8080

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -p open_kanban_service

# фронтенд
cd frontend/open_kanban_web
npm ci
npm run dev        # http://localhost:5173, прокси /api и /healthz на :8080
npm run lint
npm run typecheck
npm run test
npm run build

# контракт (нужен запущенный бекенд)
cd ../..
tools/gen-api.sh
tools/smoke.sh
```

## 9. Definition of Done для S1

- Пользователь регистрируется, входит, перезагружает страницу и остаётся
  аутентифицированным (через refresh), выходит.
- Формы показывают серверные ошибки (включая `VALIDATION_ERROR` по полям).
- Guard не пускает анонимного пользователя к защищённым страницам.
- Один неудачный `401` приводит к одному refresh и повтору запроса, без циклов.
- Новые тесты фронтенда зелёные; CI зелёный; README/roadmap обновлены;
  `schema.d.ts` синхронизирован с бекендом.

## 10. После S1

**S2 — пространства и роли:** миграция `workspaces` + `memberships`
(частичный уникальный индекс на одного owner), CRUD пространства, добавление
участника по email, назначение администратора владельцем, матрица прав из
`doc/domain_ru.md` §5 с негативными тестами. Затем фронтенд: список
пространств, настройки, участники.
