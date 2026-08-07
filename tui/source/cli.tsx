#!/usr/bin/env bun
import { render } from "ink";
import App from "./app.js";

if (!process.stdin.isTTY) {
  console.error("appimage-install-tui requires an interactive terminal.");
  console.error("Run it directly in a terminal (not piped or redirected).");
  process.exit(1);
}

render(<App />);
