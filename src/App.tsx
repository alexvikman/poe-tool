import { useState } from "react";
import { PricePage } from "./PricePage";
import { TownMutePage } from "./TownMutePage";
import { HotkeysPage } from "./HotkeysPage";
import "./App.css";

const NAV = [
  { id: "price", label: "Price check", soon: false },
  { id: "builds", label: "Builds", soon: true },
  { id: "town-mute", label: "Town mute", soon: false },
  { id: "hotkeys", label: "Hotkeys", soon: false },
] as const;

type Page = (typeof NAV)[number]["id"];

function App() {
  const [page, setPage] = useState<Page>("price");
  return (
    <div className="shell">
      <nav className="nav">
        <div className="brand">PoE Tool</div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${item.id === page ? "active" : ""}`}
            disabled={item.soon}
            onClick={() => setPage(item.id)}
          >
            {item.label}
            {item.soon && <span className="soon">Soon</span>}
          </button>
        ))}
      </nav>
      <main className="main">
        {page === "price" && <PricePage />}
        {page === "town-mute" && <TownMutePage />}
        {page === "hotkeys" && <HotkeysPage />}
      </main>
    </div>
  );
}

export default App;
