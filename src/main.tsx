import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { PriceOverlay } from "./PriceOverlay";
import "./App.css";

// The overlay window loads the same page with #price.
const isOverlay = window.location.hash === "#price";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isOverlay ? <PriceOverlay /> : <App />}</React.StrictMode>,
);
