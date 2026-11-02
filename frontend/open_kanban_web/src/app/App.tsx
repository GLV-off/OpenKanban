import { Link, Route, Routes } from "react-router";

import { HealthPage } from "../pages/health/HealthPage";
import { LoginPage } from "../pages/login/LoginPage";

export function App() {
  return (
    <div className="app">
      <header className="app-header">
        <span className="brand">Open Kanban</span>
        <nav className="app-nav">
          <Link to="/">Состояние</Link>
          <Link to="/login">Вход</Link>
        </nav>
      </header>

      <main className="app-main">
        <Routes>
          <Route path="/" element={<HealthPage />} />
          <Route path="/login" element={<LoginPage />} />
        </Routes>
      </main>
    </div>
  );
}
