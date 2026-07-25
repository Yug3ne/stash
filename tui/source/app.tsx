import React, { useState } from "react";
import { Box, Text } from "ink";
import Menu from "./components/menu.js";
import InstallScreen from "./components/install.js";
import ListScreen from "./components/list.js";
import UpdateScreen from "./components/update.js";
import RemoveScreen from "./components/remove.js";

type Screen = "menu" | "install" | "list" | "update" | "remove";

export default function App() {
  const [screen, setScreen] = useState<Screen>("menu");
  const [message, setMessage] = useState<string | null>(null);

  const navigate = (next: Screen) => {
    setMessage(null);
    setScreen(next);
  };

  return (
    <Box flexDirection="column" padding={1}>
      <Box marginBottom={1}>
        <Text bold color="cyan">
          AppImage Installer
        </Text>
      </Box>

      {message && (
        <Box marginBottom={1}>
          <Text>{message}</Text>
        </Box>
      )}

      {screen === "menu" && (
        <Menu
          onSelect={(selected) => navigate(selected)}
          onMessage={setMessage}
        />
      )}
      {screen === "install" && (
        <InstallScreen
          onBack={() => navigate("menu")}
          onMessage={setMessage}
        />
      )}
      {screen === "list" && (
        <ListScreen
          onBack={() => navigate("menu")}
          onMessage={setMessage}
        />
      )}
      {screen === "update" && (
        <UpdateScreen
          onBack={() => navigate("menu")}
          onMessage={setMessage}
        />
      )}
      {screen === "remove" && (
        <RemoveScreen
          onBack={() => navigate("menu")}
          onMessage={setMessage}
        />
      )}
    </Box>
  );
}
