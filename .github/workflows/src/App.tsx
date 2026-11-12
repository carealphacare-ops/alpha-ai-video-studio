import { useState } from "react";
import { NAV } from "./nav";
import Dashboard from "./pages/Dashboard";
import Projects from "./pages/Projects";
import Clipping from "./pages/Clipping";
import Providers from "./pages/Providers";
import SettingsPage from "./pages/SettingsPage";
import Planned from "./pages/Planned";

export default function App() {
  const [page, setPage] = useState("dashboard");
  const item = NAV.find((n) => n.id === page)!;

  const body = (() => {
    switch (page) {
      case "dashboard": return <Dashboard go={setPage} />;
      case "projects": return <Projects />;
      case "clipping": return <Clipping />;
      case "providers": return <Providers />;
      case "settings": return <SettingsPage />;
      default: return <Planned item={item} />;
    }
  })();

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">ALPHA<span>AI VIDEO STUDIO</span></div>
        <nav>
          {NAV.map((n) => (
            <button key={n.id} className={n.id === page ? "nav active" : "nav"} onClick={() => setPage(n.id)}>
              <span>{n.label}</span>
              <i className={`dot ${n.status}`} title={n.status} />
            </button>
          ))}
        </nav>
        <div className="legend">
          <i className="dot working" /> works <i className="dot partial" /> partial <i className="dot planned" /> planned
        </div>
      </aside>
      <main className="content">
        <h1>{item.label}</h1>
        {body}
      </main>
    </div>
  );
}
