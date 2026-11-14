import { TownMutePage } from "./TownMutePage";
import "./App.css";

const NAV = [
  { id: "price", label: "Price check", soon: true },
  { id: "builds", label: "Builds", soon: true },
  { id: "town-mute", label: "Town mute", soon: false },
];

function App() {
  return (
    <div className="shell">
      <nav className="nav">
        <div className="brand">PoE Tool</div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${item.id === "town-mute" ? "active" : ""}`}
            disabled={item.soon}
          >
            {item.label}
            {item.soon && <span className="soon">Soon</span>}
          </button>
        ))}
      </nav>
      <main className="main">
        <TownMutePage />
      </main>
    </div>
  );
}

export default App;
