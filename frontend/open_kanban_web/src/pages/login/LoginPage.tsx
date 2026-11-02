import { Link } from "react-router";

export function LoginPage() {
  return (
    <section className="card">
      <h1>Вход</h1>
      <p className="muted">
        Форма входа появится в срезе S1 (аутентификация: регистрация, вход, обновление сессии).
      </p>
      <Link to="/">← На главную</Link>
    </section>
  );
}
