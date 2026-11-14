import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { PricePage } from "./PricePage";
import { TownMutePage } from "./TownMutePage";
import { HotkeysPage } from "./HotkeysPage";
import { SearchesPage } from "./SearchesPage";
import { UpdateNotice } from "./UpdateNotice";
import { UpdatesPage } from "./UpdatesPage";
import { startUpdateChecks, useUpdates } from "./updates";
import "./App.css";

const NAV = [
  { id: "price", label: "Price check", soon: false },
  { id: "searches", label: "Searches", soon: false },
  { id: "builds", label: "Builds", soon: true },
  { id: "town-mute", label: "Town mute", soon: false },
  { id: "hotkeys", label: "Hotkeys", soon: false },
  { id: "updates", label: "Updates", soon: false },
] as const;

type Page = (typeof NAV)[number]["id"];

function App() {
  const [page, setPage] = useState<Page>("price");
  const [version, setVersion] = useState<string | null>(null);

  const { update } = useUpdates();

  useEffect(() => {
    getVersion().then(setVersion, () => setVersion(null));
    startUpdateChecks();
  }, []);
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
            {item.id === "updates" && update && <span className="dot" title="Update available" />}
          </button>
        ))}
        <div className="nav-foot">
          {page !== "updates" && <UpdateNotice onOpen={() => setPage("updates")} />}
          {version && <div className="version">v{version}</div>}
        </div>
      </nav>
      <main className="main">
        {page === "price" && <PricePage />}
        {page === "searches" && <SearchesPage />}
        {page === "town-mute" && <TownMutePage />}
        {page === "hotkeys" && <HotkeysPage />}
        {page === "updates" && <UpdatesPage />}
      </main>
    </div>
  );
}

export default App;
