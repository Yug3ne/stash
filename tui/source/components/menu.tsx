import React, { useEffect } from "react";
import { Box, Text, useInput } from "ink";

const items: { label: string; key: string; value: "install" | "list" | "update" | "remove" | "quit" }[] = [
  { label: "Install AppImage", key: "i", value: "install" },
  { label: "List installed apps", key: "l", value: "list" },
  { label: "Update apps", key: "u", value: "update" },
  { label: "Remove an app", key: "r", value: "remove" },
  { label: "Quit", key: "q", value: "quit" },
];

interface Props {
  onSelect: (screen: "install" | "list" | "update" | "remove") => void;
  onMessage: (msg: string | null) => void;
}

export default function Menu({ onSelect, onMessage }: Props) {
  const [selected, setSelected] = React.useState(0);

  useInput((input, key) => {
    if (key.upArrow) {
      setSelected((s) => (s > 0 ? s - 1 : items.length - 1));
      onMessage(null);
    } else if (key.downArrow) {
      setSelected((s) => (s < items.length - 1 ? s + 1 : 0));
      onMessage(null);
    } else if (key.return) {
      const item = items[selected];
      if (item.value === "quit") {
        process.exit(0);
      } else {
        onSelect(item.value);
      }
    } else if (key.escape) {
      process.exit(0);
    } else {
      const item = items.find((i) => i.key === input.toLowerCase());
      if (item) {
        if (item.value === "quit") {
          process.exit(0);
        } else {
          onSelect(item.value);
        }
      }
    }
  });

  useEffect(() => {
    onMessage("Use ↑/↓ or letter keys, Enter to select");
  }, [onMessage]);

  return (
    <Box flexDirection="column">
      {items.map((item, index) => (
        <Box key={item.value}>
          <Text bold={index === selected} color={index === selected ? "cyan" : undefined}>
            {index === selected ? "> " : "  "}
            [{item.key}] {item.label}
          </Text>
        </Box>
      ))}
    </Box>
  );
}
